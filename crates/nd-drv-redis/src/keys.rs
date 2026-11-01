use std::time::{Duration, Instant};

use nd_driver::{
    DriverError, KeyData, KeyEntry, KeyInfo, KeyPage, KeyRead, KeyScan, KeyValue, KeyWrite,
    Outcome, Result, command_line,
};
use redis::Value;

use crate::{RedisConnection, reply};

const SCAN_BUDGET: Duration = Duration::from_millis(250);
const MAX_TEXT: u64 = 1 << 20;

const LIST_REMOVE: &str = "if redis.call('LINDEX', KEYS[1], ARGV[1]) == ARGV[2] then \
redis.call('LSET', KEYS[1], ARGV[1], ARGV[3]) return redis.call('LREM', KEYS[1], 1, ARGV[3]) \
else return -1 end";
const MARKER: &str = "\u{1}nicedb-removed\u{1}";

fn number(v: &Value) -> Option<i64> {
    match v {
        Value::Int(n) => Some(*n),
        other => reply::scalar(other)?.parse().ok(),
    }
}

fn ttl(v: &Value) -> Option<u64> {
    number(v).and_then(|n| u64::try_from(n).ok())
}

pub async fn scan(c: &RedisConnection, s: &KeyScan) -> Result<KeyPage> {
    let want = usize::try_from(s.count.clamp(1, 5000)).unwrap_or(500);
    let pattern = if s.pattern.trim().is_empty() {
        "*"
    } else {
        s.pattern.as_str()
    };
    let mut cursor = if s.cursor.trim().is_empty() {
        "0".to_owned()
    } else {
        s.cursor.clone()
    };
    let started = Instant::now();
    let mut names: Vec<String> = Vec::new();
    let mut args = vec!["MATCH", pattern, "COUNT", "1000"];
    if let Some(kind) = s.kind.as_deref().filter(|k| !k.is_empty()) {
        args.extend(["TYPE", kind]);
    }
    loop {
        let (next, found) = c.scan_page(&cursor, &args).await?;
        cursor = next;
        names.extend(found);
        if cursor == "0" || names.len() >= want || started.elapsed() > SCAN_BUDGET {
            break;
        }
    }
    let keys = describe_keys(c, names).await;
    let total = c.dbsize().await;
    Ok(KeyPage {
        keys,
        cursor,
        total,
    })
}

async fn describe_keys(c: &RedisConnection, names: Vec<String>) -> Vec<KeyInfo> {
    if names.is_empty() {
        return Vec::new();
    }
    let replies = c
        .pipe_each(&names, 3, |pipe, n| {
            pipe.cmd("TYPE").arg(n);
            pipe.cmd("PTTL").arg(n);
            pipe.cmd("MEMORY").arg("USAGE").arg(n);
        })
        .await;
    names
        .into_iter()
        .enumerate()
        .map(|(i, key)| {
            let at = |j: usize| replies.get(i * 3 + j);
            KeyInfo {
                key,
                kind: at(0).and_then(reply::scalar).unwrap_or_default(),
                ttl_ms: at(1).and_then(ttl),
                bytes: at(2).and_then(number).and_then(|n| u64::try_from(n).ok()),
            }
        })
        .collect()
}

#[allow(clippy::many_single_char_names, clippy::too_many_lines)]
pub async fn read(c: &RedisConnection, r: &KeyRead) -> Result<KeyValue> {
    let key = r.key.as_str();
    let count = r.count.clamp(1, 1000);
    let mut pipe = redis::pipe();
    pipe.cmd("TYPE").arg(key).cmd("PTTL").arg(key);
    let head: Vec<Value> = c.pipe_key(key, &pipe).await?;
    let kind = head.first().and_then(reply::scalar).unwrap_or_default();
    let ttl_ms = head.get(1).and_then(ttl);
    let start: i64 = r.cursor.parse().unwrap_or(0);
    let cursor = if r.cursor.is_empty() {
        "0"
    } else {
        r.cursor.as_str()
    };
    let len = |cmd: &str| {
        let mut q = redis::cmd(cmd);
        q.arg(key);
        q
    };
    let size = async |cmd: &str| -> Option<u64> {
        c.query(&len(cmd))
            .await
            .ok()
            .as_ref()
            .and_then(number)
            .and_then(|n| u64::try_from(n).ok())
    };
    let (length, data, next) = match kind.as_str() {
        "string" => {
            let n = size("STRLEN").await.unwrap_or(0);
            let truncated = n > MAX_TEXT;
            let q = if truncated {
                let mut q = redis::cmd("GETRANGE");
                q.arg(key).arg(0).arg(MAX_TEXT - 1);
                q
            } else {
                len("GET")
            };
            let v = c.query(&q).await?;
            let bytes = match &v {
                Value::BulkString(b) => b.clone(),
                other => reply::scalar(other).unwrap_or_default().into_bytes(),
            };
            let binary = std::str::from_utf8(&bytes).is_err();
            (
                Some(n),
                KeyData::Text {
                    text: reply::text(&bytes),
                    binary,
                    truncated,
                },
                None,
            )
        }
        "ReJSON-RL" => {
            let v = c.query(&len("JSON.GET")).await?;
            let text = reply::scalar(&v).unwrap_or_default();
            (
                Some(text.len() as u64),
                KeyData::Text {
                    text,
                    binary: false,
                    truncated: false,
                },
                None,
            )
        }
        "hash" | "set" => {
            let (scan, count_cmd) = if kind == "hash" {
                ("HSCAN", "HLEN")
            } else {
                ("SSCAN", "SCARD")
            };
            let mut q = redis::cmd(scan);
            q.arg(key).arg(cursor).arg("COUNT").arg(count);
            let v = c.query(&q).await?;
            let parts = reply::list(&v);
            let next = parts.first().and_then(reply::scalar).filter(|n| n != "0");
            let body = parts.get(1).cloned().unwrap_or(Value::Nil);
            let entries = if kind == "hash" {
                reply::pairs(&body)
                    .into_iter()
                    .map(|(f, v)| KeyEntry {
                        name: reply::scalar(f).unwrap_or_default(),
                        value: reply::scalar(v).unwrap_or_default(),
                    })
                    .collect()
            } else {
                reply::list(&body)
                    .iter()
                    .map(|m| KeyEntry {
                        name: reply::scalar(m).unwrap_or_default(),
                        value: String::new(),
                    })
                    .collect()
            };
            (size(count_cmd).await, KeyData::Entries { entries }, next)
        }
        "list" | "zset" => {
            let total = size(if kind == "list" { "LLEN" } else { "ZCARD" }).await;
            let stop = start + i64::from(count) - 1;
            let mut q = redis::cmd(if kind == "list" {
                "LRANGE"
            } else {
                "ZREVRANGE"
            });
            q.arg(key).arg(start).arg(stop);
            if kind == "zset" {
                q.arg("WITHSCORES");
            }
            let v = c.query(&q).await?;
            let entries: Vec<KeyEntry> = if kind == "list" {
                reply::list(&v)
                    .iter()
                    .zip(start..)
                    .map(|(item, i)| KeyEntry {
                        name: i.to_string(),
                        value: reply::scalar(item).unwrap_or_default(),
                    })
                    .collect()
            } else {
                reply::pairs(&v)
                    .into_iter()
                    .map(|(m, s)| KeyEntry {
                        name: reply::scalar(m).unwrap_or_default(),
                        value: reply::scalar(s).unwrap_or_default(),
                    })
                    .collect()
            };
            let more = total.is_some_and(|t| u64::try_from(stop + 1).unwrap_or(u64::MAX) < t);
            (
                total,
                KeyData::Entries { entries },
                more.then(|| (stop + 1).to_string()),
            )
        }
        "stream" => {
            let from = if r.cursor.is_empty() {
                "-"
            } else {
                r.cursor.as_str()
            };
            let mut q = redis::cmd("XRANGE");
            q.arg(key).arg(from).arg("+").arg("COUNT").arg(count);
            let v = c.query(&q).await?;
            let entries: Vec<KeyEntry> = reply::list(&v)
                .iter()
                .map(|e| {
                    let parts = reply::list(e);
                    KeyEntry {
                        name: parts.first().and_then(reply::scalar).unwrap_or_default(),
                        value: parts.get(1).map(reply::fields_json).unwrap_or_default(),
                    }
                })
                .collect();
            let next = (entries.len() == count as usize)
                .then(|| entries.last().map(|e| format!("({}", e.name)))
                .flatten();
            (size("XLEN").await, KeyData::Entries { entries }, next)
        }
        _ => (None, KeyData::Missing, None),
    };
    Ok(KeyValue {
        key: r.key.clone(),
        kind,
        ttl_ms,
        length,
        data,
        next,
    })
}

fn args(parts: &[&str]) -> Vec<String> {
    parts.iter().map(|s| (*s).to_owned()).collect()
}

pub fn commands(w: &KeyWrite) -> Vec<Vec<String>> {
    let with = |head: &[&str], rest: &[String]| {
        let mut a = args(head);
        a.extend(rest.iter().cloned());
        vec![a]
    };
    match w {
        KeyWrite::SetString { key, value } => vec![args(&["SET", key, value, "KEEPTTL"])],
        KeyWrite::HashSet { key, field, value } => vec![args(&["HSET", key, field, value])],
        KeyWrite::HashDelete { key, fields } => with(&["HDEL", key], fields),
        KeyWrite::ListSet { key, index, value } => {
            vec![args(&["LSET", key, &index.to_string(), value])]
        }
        KeyWrite::ListPush { key, values, head } => {
            with(&[if *head { "LPUSH" } else { "RPUSH" }, key], values)
        }
        KeyWrite::ListRemove { key, index, value } => vec![args(&[
            "EVAL",
            LIST_REMOVE,
            "1",
            key,
            &index.to_string(),
            value,
            MARKER,
        ])],
        KeyWrite::SetAdd { key, members } => with(&["SADD", key], members),
        KeyWrite::SetRemove { key, members } => with(&["SREM", key], members),
        KeyWrite::ZsetAdd { key, member, score } => {
            vec![args(&["ZADD", key, &score.to_string(), member])]
        }
        KeyWrite::ZsetRemove { key, members } => with(&["ZREM", key], members),
        KeyWrite::StreamAdd { key, fields } => {
            let flat: Vec<String> = fields
                .iter()
                .flat_map(|f| [f.name.clone(), f.value.clone()])
                .collect();
            with(&["XADD", key, "*"], &flat)
        }
        KeyWrite::StreamDelete { key, ids } => with(&["XDEL", key], ids),
        KeyWrite::JsonSet { key, value } => vec![args(&["JSON.SET", key, "$", value])],
        KeyWrite::Expire {
            key,
            ttl_ms: Some(ms),
        } => {
            vec![args(&["PEXPIRE", key, &ms.to_string()])]
        }
        KeyWrite::Expire { key, ttl_ms: None } => vec![args(&["PERSIST", key])],
        KeyWrite::Rename { key, to } => vec![args(&["RENAMENX", key, to])],
        KeyWrite::Delete { keys } => with(&["UNLINK"], keys),
        KeyWrite::Create {
            key,
            kind,
            first,
            ttl_ms,
        } => {
            let (n, v) = (first.name.as_str(), first.value.as_str());
            let mut out = vec![match kind.as_str() {
                "hash" => args(&["HSET", key, n, v]),
                "list" => args(&["RPUSH", key, v]),
                "set" => args(&["SADD", key, n]),
                "zset" => args(&["ZADD", key, v, n]),
                "stream" => args(&["XADD", key, "*", n, v]),
                "ReJSON-RL" => args(&["JSON.SET", key, "$", v, "NX"]),
                _ => args(&["SET", key, v, "NX"]),
            }];
            if let Some(ms) = ttl_ms {
                out.push(args(&["PEXPIRE", key, &ms.to_string()]));
            }
            out
        }
    }
}

pub fn describe(w: &KeyWrite) -> String {
    commands(w)
        .iter()
        .map(|c| command_line(c))
        .collect::<Vec<_>>()
        .join("\n")
}

pub async fn write(c: &RedisConnection, w: &KeyWrite) -> Result<Outcome> {
    let key = w.key();
    if let KeyWrite::Create { .. } = w {
        let exists = c.query(redis::cmd("EXISTS").arg(key)).await?;
        if number(&exists).unwrap_or(0) > 0 {
            return Err(DriverError::query(format!(
                "a key named {key} already exists"
            )));
        }
    }
    let list = commands(w);
    let mut pipe = redis::pipe();
    if list.len() > 1 {
        pipe.atomic();
    }
    for cmd in &list {
        let mut q = redis::Cmd::new();
        for a in cmd {
            q.arg(a);
        }
        pipe.add_command(q);
    }
    let replies: Vec<Value> = c.pipe_key(key, &pipe).await?;
    let first = replies.first().cloned().unwrap_or(Value::Nil);
    let n = number(&first);
    match w {
        KeyWrite::Rename { to, .. } if n == Some(0) => {
            return Err(DriverError::query(format!(
                "a key named {to} already exists"
            )));
        }
        KeyWrite::ListRemove { .. } if n == Some(-1) => {
            return Err(DriverError::query(
                "the list changed since it was read: reload it and try again",
            ));
        }
        KeyWrite::Create { .. } if first == Value::Nil => {
            return Err(DriverError::query(format!(
                "a key named {key} already exists"
            )));
        }
        KeyWrite::Expire {
            ttl_ms: Some(_), ..
        } if n == Some(0) => {
            return Err(DriverError::query(format!("the key {key} is gone")));
        }
        _ => {}
    }
    let command = list
        .first()
        .and_then(|c| c.first())
        .cloned()
        .unwrap_or_default();
    Ok(Outcome {
        command: Some(command),
        rows_affected: n.and_then(|n| u64::try_from(n).ok()),
    })
}
