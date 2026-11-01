use std::collections::HashMap;
use std::sync::RwLock;

use nd_driver::{DriverError, Result};
use redis::aio::MultiplexedConnection;
use redis::{AsyncConnectionConfig, Client, Value};
use tokio::sync::Mutex;

use crate::reply;

pub(crate) const SLOTS: u16 = 16384;
const HOPS: usize = 5;

pub(crate) type Addr = (String, u16);

fn crc16(bytes: &[u8]) -> u16 {
    let mut crc: u16 = 0;
    for b in bytes {
        crc ^= u16::from(*b) << 8;
        for _ in 0..8 {
            crc = if crc & 0x8000 == 0 {
                crc << 1
            } else {
                (crc << 1) ^ 0x1021
            };
        }
    }
    crc
}

pub(crate) fn slot(key: &[u8]) -> u16 {
    let tagged = key.iter().position(|b| *b == b'{').and_then(|open| {
        let rest = &key[open + 1..];
        rest.iter()
            .position(|b| *b == b'}')
            .filter(|close| *close > 0)
            .map(|close| &rest[..close])
    });
    crc16(tagged.unwrap_or(key)) % SLOTS
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Shard {
    pub from: u16,
    pub to: u16,
    pub master: Addr,
    pub replicas: Vec<Addr>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct SlotMap {
    pub shards: Vec<Shard>,
}

fn addr(v: &Value, seed: &str) -> Option<Addr> {
    let parts = reply::list(v);
    let host = parts.first().and_then(reply::scalar).unwrap_or_default();
    let port = match parts.get(1)? {
        Value::Int(n) => u16::try_from(*n).ok()?,
        other => reply::scalar(other)?.parse().ok()?,
    };
    let host = if host.is_empty() || host == "?" {
        seed.to_owned()
    } else {
        host
    };
    Some((host, port))
}

impl SlotMap {
    pub(crate) fn from_slots(v: &Value, seed: &str) -> Option<Self> {
        let mut shards: Vec<Shard> = reply::list(v)
            .iter()
            .filter_map(|range| {
                let parts = reply::list(range);
                let bound = |i: usize| -> Option<u16> {
                    match parts.get(i)? {
                        Value::Int(n) => u16::try_from(*n).ok(),
                        other => reply::scalar(other)?.parse().ok(),
                    }
                };
                Some(Shard {
                    from: bound(0)?,
                    to: bound(1)?,
                    master: addr(parts.get(2)?, seed)?,
                    replicas: parts.iter().skip(3).filter_map(|r| addr(r, seed)).collect(),
                })
            })
            .collect();
        if shards.is_empty() {
            return None;
        }
        shards.sort_by_key(|s| s.from);
        Some(Self { shards })
    }

    pub(crate) fn master(&self, slot: u16) -> Option<&Addr> {
        self.shards
            .iter()
            .find(|s| s.from <= slot && slot <= s.to)
            .map(|s| &s.master)
    }

    pub(crate) fn masters(&self) -> Vec<Addr> {
        let mut out: Vec<Addr> = Vec::new();
        for s in &self.shards {
            if !out.contains(&s.master) {
                out.push(s.master.clone());
            }
        }
        out
    }

    pub(crate) fn moved(&mut self, slot: u16, to: Addr) {
        let Some(i) = self
            .shards
            .iter()
            .position(|s| s.from <= slot && slot <= s.to)
        else {
            self.shards.push(Shard {
                from: slot,
                to: slot,
                master: to,
                replicas: Vec::new(),
            });
            self.shards.sort_by_key(|s| s.from);
            return;
        };
        let old = self.shards.remove(i);
        let mut parts = Vec::new();
        if old.from < slot {
            parts.push(Shard {
                to: slot - 1,
                ..old.clone()
            });
        }
        parts.push(Shard {
            from: slot,
            to: slot,
            master: to,
            replicas: Vec::new(),
        });
        if slot < old.to {
            parts.push(Shard {
                from: slot + 1,
                ..old
            });
        }
        for (k, p) in parts.into_iter().enumerate() {
            self.shards.insert(i + k, p);
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Redirect {
    Moved(u16, Addr),
    Ask(u16, Addr),
}

fn parse_addr(text: &str) -> Option<Addr> {
    let (host, port) = text.rsplit_once(':')?;
    Some((host.trim_matches(['[', ']']).to_owned(), port.parse().ok()?))
}

pub(crate) fn redirect(e: &redis::RedisError) -> Option<Redirect> {
    let code = e.code()?;
    let detail = e.detail()?;
    let mut parts = detail.split_whitespace();
    let slot: u16 = parts.next()?.parse().ok()?;
    let to = parse_addr(parts.next()?)?;
    match code {
        "MOVED" => Some(Redirect::Moved(slot, to)),
        "ASK" => Some(Redirect::Ask(slot, to)),
        _ => None,
    }
}

const KEYLESS: &[&str] = &[
    "PING",
    "INFO",
    "DBSIZE",
    "SCAN",
    "KEYS",
    "FLUSHDB",
    "FLUSHALL",
    "CONFIG",
    "CLIENT",
    "CLUSTER",
    "COMMAND",
    "SLOWLOG",
    "TIME",
    "LASTSAVE",
    "MODULE",
    "ACL",
    "LATENCY",
    "RANDOMKEY",
    "DEBUG",
    "ECHO",
    "FUNCTION",
    "SCRIPT",
    "WAIT",
    "SAVE",
    "BGSAVE",
    "BGREWRITEAOF",
    "ROLE",
    "READONLY",
    "READWRITE",
    "PUBLISH",
    "PUBSUB",
    "LOLWUT",
    "SWAPDB",
];

pub(crate) fn key_index(args: &[&[u8]]) -> Option<usize> {
    let name = String::from_utf8_lossy(args.first()?).to_ascii_uppercase();
    let numkeys = |at: usize| -> Option<usize> {
        let n: usize = std::str::from_utf8(args.get(at)?).ok()?.parse().ok()?;
        (n > 0 && args.len() > at + 1).then_some(at + 1)
    };
    let after = |word: &str| -> Option<usize> {
        args.iter()
            .position(|a| a.eq_ignore_ascii_case(word.as_bytes()))
            .map(|p| p + 1)
            .filter(|p| *p < args.len())
    };
    match name.as_str() {
        n if KEYLESS.contains(&n) => None,
        "EVAL" | "EVALSHA" | "EVAL_RO" | "EVALSHA_RO" | "FCALL" | "FCALL_RO" | "ZUNION"
        | "ZINTER" | "ZDIFF" | "SINTERCARD" | "ZINTERCARD" | "LMPOP" | "ZMPOP" => numkeys(2),
        "BLMPOP" | "BZMPOP" => numkeys(3),
        "XREAD" | "XREADGROUP" => after("STREAMS"),
        "OBJECT" | "XINFO" | "XGROUP" => (args.len() > 2).then_some(2),
        "MEMORY"
            if args
                .get(1)
                .is_some_and(|a| a.eq_ignore_ascii_case(b"USAGE")) =>
        {
            (args.len() > 2).then_some(2)
        }
        "MEMORY" | "MIGRATE" => None,
        _ => (args.len() > 1).then_some(1),
    }
}

pub(crate) fn command_slot(cmd: &redis::Cmd) -> Option<u16> {
    let args: Vec<&[u8]> = cmd
        .args_iter()
        .filter_map(|a| match a {
            redis::Arg::Simple(b) => Some(b),
            _ => None,
        })
        .collect();
    key_index(&args).and_then(|i| args.get(i)).map(|k| slot(k))
}

pub(crate) fn parse_cursor(text: &str) -> (usize, String) {
    match text.split_once(':') {
        Some((node, cursor)) => (node.parse().unwrap_or(0), cursor.to_owned()),
        None => (0, "0".to_owned()),
    }
}

pub(crate) fn next_cursor(node: usize, cursor: &str, masters: usize) -> String {
    if cursor != "0" {
        format!("{node}:{cursor}")
    } else if node + 1 < masters {
        format!("{}:0", node + 1)
    } else {
        "0".to_owned()
    }
}

pub(crate) trait Opener: Send + Sync {
    fn client(&self, host: &str, port: u16) -> Result<Client>;
    fn config(&self) -> &AsyncConnectionConfig;
}

pub(crate) struct Cluster {
    pub map: RwLock<SlotMap>,
    seed: Addr,
    conns: Mutex<HashMap<Addr, MultiplexedConnection>>,
    opener: Box<dyn Opener>,
}

impl Cluster {
    pub(crate) fn new(seed: Addr, map: SlotMap, opener: Box<dyn Opener>) -> Self {
        Self {
            map: RwLock::new(map),
            seed,
            conns: Mutex::new(HashMap::new()),
            opener,
        }
    }

    pub(crate) fn masters(&self) -> Vec<Addr> {
        self.map
            .read()
            .map_or_else(|_| vec![self.seed.clone()], |m| m.masters())
    }

    pub(crate) fn snapshot(&self) -> SlotMap {
        self.map.read().map(|m| m.clone()).unwrap_or_default()
    }

    fn target(&self, slot: Option<u16>) -> Addr {
        let map = self.map.read().ok();
        slot.and_then(|s| map.as_ref().and_then(|m| m.master(s).cloned()))
            .or_else(|| map.as_ref().and_then(|m| m.masters().into_iter().next()))
            .unwrap_or_else(|| self.seed.clone())
    }

    pub(crate) async fn node(&self, at: &Addr) -> Result<MultiplexedConnection> {
        let mut conns = self.conns.lock().await;
        if let Some(c) = conns.get(at) {
            return Ok(c.clone());
        }
        let conn = self
            .opener
            .client(&at.0, at.1)?
            .get_multiplexed_async_connection_with_config(self.opener.config())
            .await
            .map_err(crate::rerr)?;
        conns.insert(at.clone(), conn.clone());
        Ok(conn)
    }

    pub(crate) async fn query(&self, cmd: &redis::Cmd) -> Result<Value> {
        let slot = command_slot(cmd);
        let mut at = self.target(slot);
        let mut asking = false;
        for _ in 0..HOPS {
            let mut conn = self.node(&at).await?;
            let out = if asking {
                let mut pipe = redis::pipe();
                pipe.cmd("ASKING").ignore().add_command(cmd.clone());
                pipe.query_async::<Vec<Value>>(&mut conn)
                    .await
                    .map(|mut v| v.pop().unwrap_or(Value::Nil))
            } else {
                cmd.query_async::<Value>(&mut conn).await
            };
            match out {
                Ok(v) => return Ok(v),
                Err(e) => match redirect(&e) {
                    Some(Redirect::Moved(s, to)) => {
                        if let Ok(mut map) = self.map.write() {
                            map.moved(s, to.clone());
                        }
                        at = to;
                        asking = false;
                    }
                    Some(Redirect::Ask(_, to)) => {
                        at = to;
                        asking = true;
                    }
                    None => {
                        if e.is_io_error() || e.is_connection_dropped() {
                            self.conns.lock().await.remove(&at);
                        }
                        return Err(crate::err(&e));
                    }
                },
            }
        }
        Err(DriverError::query(
            "the cluster kept redirecting the command",
        ))
    }

    pub(crate) async fn pipe(&self, key: &str, pipe: &redis::Pipeline) -> Result<Vec<Value>> {
        let mut at = self.target(Some(slot(key.as_bytes())));
        for _ in 0..HOPS {
            let mut conn = self.node(&at).await?;
            match pipe.query_async::<Vec<Value>>(&mut conn).await {
                Ok(v) => return Ok(v),
                Err(e) => match redirect(&e) {
                    Some(Redirect::Moved(s, to)) => {
                        if let Ok(mut map) = self.map.write() {
                            map.moved(s, to.clone());
                        }
                        at = to;
                    }
                    _ => return Err(crate::err(&e)),
                },
            }
        }
        Err(DriverError::query(
            "the cluster kept redirecting the command",
        ))
    }

    pub(crate) async fn on(&self, at: &Addr, cmd: &redis::Cmd) -> Result<Value> {
        let mut conn = self.node(at).await?;
        cmd.query_async(&mut conn).await.map_err(crate::rerr)
    }
}
