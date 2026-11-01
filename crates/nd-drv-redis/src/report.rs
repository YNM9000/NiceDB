use nd_driver::{Report, ReportGroup, ReportSection, Result};
use redis::Value;

use crate::{RedisConnection, reply};

const CLIENT_FIELDS: [&str; 9] = [
    "id", "addr", "name", "user", "db", "age", "idle", "cmd", "flags",
];

fn section(
    id: &str,
    group: ReportGroup,
    columns: &[&str],
    rows: Vec<Vec<String>>,
) -> ReportSection {
    ReportSection {
        id: id.to_owned(),
        group,
        columns: columns.iter().map(|c| (*c).to_owned()).collect(),
        rows,
    }
}

pub fn info_sections(text: &str) -> Vec<ReportSection> {
    let mut out: Vec<ReportSection> = Vec::new();
    for line in text.lines().map(str::trim) {
        if let Some(head) = line.strip_prefix('#') {
            out.push(section(
                &head.trim().to_ascii_lowercase(),
                ReportGroup::Info,
                &["name", "value"],
                Vec::new(),
            ));
        } else if let Some((k, v)) = line.split_once(':')
            && let Some(last) = out.last_mut()
        {
            last.rows.push(vec![k.to_owned(), v.to_owned()]);
        }
    }
    out.retain(|s| !s.rows.is_empty());
    out
}

fn command_stats(text: &str) -> Vec<Vec<String>> {
    let mut rows: Vec<(u64, Vec<String>)> = text
        .lines()
        .filter_map(|l| l.trim().strip_prefix("cmdstat_"))
        .filter_map(|l| l.split_once(':'))
        .map(|(name, rest)| {
            let field = |f: &str| {
                rest.split(',')
                    .find_map(|kv| kv.strip_prefix(f).and_then(|v| v.strip_prefix('=')))
                    .unwrap_or("0")
                    .to_owned()
            };
            let calls = field("calls");
            (
                calls.parse().unwrap_or(0),
                vec![
                    name.replace('|', " ").to_ascii_uppercase(),
                    calls,
                    field("usec"),
                    field("usec_per_call"),
                    field("failed_calls"),
                ],
            )
        })
        .collect();
    rows.sort_by_key(|r| std::cmp::Reverse(r.0));
    rows.into_iter().map(|(_, r)| r).collect()
}

pub fn clients(text: &str) -> Vec<Vec<String>> {
    text.lines()
        .filter(|l| !l.trim().is_empty())
        .map(|line| {
            let fields: Vec<(&str, &str)> = line
                .split(' ')
                .filter_map(|kv| kv.split_once('='))
                .collect();
            CLIENT_FIELDS
                .iter()
                .map(|f| {
                    fields
                        .iter()
                        .find(|(k, _)| k == f)
                        .map(|(_, v)| (*v).to_owned())
                        .unwrap_or_default()
                })
                .collect()
        })
        .collect()
}

fn slowlog(v: &Value) -> Vec<Vec<String>> {
    reply::list(v)
        .iter()
        .map(|e| {
            let parts = reply::list(e);
            let at = |i: usize| parts.get(i).and_then(reply::scalar).unwrap_or_default();
            let command = parts
                .get(3)
                .map(|a| {
                    let args: Vec<String> =
                        reply::list(a).iter().filter_map(reply::scalar).collect();
                    nd_driver::command_line(&args)
                })
                .unwrap_or_default();
            vec![at(0), at(1), at(2), command, at(4), at(5)]
        })
        .collect()
}

fn flatten(prefix: &str, v: &Value, rows: &mut Vec<Vec<String>>) {
    let pairs = reply::pairs(v);
    if matches!(v, Value::Map(_)) || !pairs.is_empty() {
        for (k, inner) in pairs {
            let name = reply::scalar(k).unwrap_or_default();
            let name = if prefix.is_empty() {
                name
            } else {
                format!("{prefix}.{name}")
            };
            match reply::scalar(inner) {
                Some(s) => rows.push(vec![name, s]),
                None => flatten(&name, inner, rows),
            }
        }
    }
}

pub async fn report(c: &RedisConnection) -> Result<Report> {
    let text = |v: &Value| reply::scalar(v).unwrap_or_default();
    let info = c.query(&redis::cmd("INFO")).await?;
    let mut sections = info_sections(&text(&info));
    if let Ok(v) = c.query(redis::cmd("INFO").arg("commandstats")).await {
        let rows = command_stats(&text(&v));
        if !rows.is_empty() {
            sections.push(section(
                "commandstats",
                ReportGroup::Info,
                &["command", "calls", "usec", "usec_per_call", "failed_calls"],
                rows,
            ));
        }
    }
    if let Ok(v) = c.query(redis::cmd("CLIENT").arg("LIST")).await {
        sections.push(section(
            "clients_list",
            ReportGroup::Clients,
            &CLIENT_FIELDS,
            clients(&text(&v)),
        ));
    }
    if let Ok(v) = c.query(redis::cmd("SLOWLOG").arg("GET").arg(128)).await {
        sections.push(section(
            "slowlog",
            ReportGroup::Slowlog,
            &["id", "time", "usec", "command", "client", "name"],
            slowlog(&v),
        ));
    }
    if let Ok(v) = c.query(redis::cmd("MEMORY").arg("STATS")).await {
        let mut rows = Vec::new();
        flatten("", &v, &mut rows);
        sections.push(section(
            "memory_stats",
            ReportGroup::Memory,
            &["name", "value"],
            rows,
        ));
    }
    if let Some(map) = c.slot_map() {
        sections.push(section(
            "cluster_slots",
            ReportGroup::Info,
            &["slots", "master", "replicas"],
            map.shards
                .iter()
                .map(|s| {
                    vec![
                        format!("{}–{}", s.from, s.to),
                        format!("{}:{}", s.master.0, s.master.1),
                        s.replicas
                            .iter()
                            .map(|(h, p)| format!("{h}:{p}"))
                            .collect::<Vec<_>>()
                            .join(", "),
                    ]
                })
                .collect(),
        ));
    }
    if let Some(link) = c.sentinel() {
        sections.push(section(
            "sentinel",
            ReportGroup::Info,
            &["name", "value"],
            sentinel_rows(link).await,
        ));
    }
    if let Ok(v) = c.query(redis::cmd("MEMORY").arg("DOCTOR")).await {
        let rows = text(&v).lines().map(|l| vec![l.to_owned()]).collect();
        sections.push(section(
            "memory_doctor",
            ReportGroup::Memory,
            &["text"],
            rows,
        ));
    }
    Ok(Report { sections })
}

fn field(map: &[(&Value, &Value)], name: &str) -> Option<String> {
    map.iter()
        .find(|(k, _)| reply::scalar(k).as_deref() == Some(name))
        .and_then(|(_, v)| reply::scalar(v))
}

async fn sentinel_rows(link: &crate::SentinelLink) -> Vec<Vec<String>> {
    let connected = format!("{}:{}", link.at.0, link.at.1);
    let mut rows = vec![vec!["master_name".to_owned(), link.name.clone()]];
    let mut master = redis::cmd("SENTINEL");
    master.arg("MASTER").arg(&link.name);
    match link.ask(&master).await {
        Ok((v, (host, port))) => {
            let map = reply::pairs(&v);
            let now = format!(
                "{}:{}",
                field(&map, "ip").unwrap_or_default(),
                field(&map, "port").unwrap_or_default()
            );
            let others = field(&map, "num-other-sentinels")
                .and_then(|n| n.parse::<u32>().ok())
                .unwrap_or(0);
            rows.push(vec!["master_now".into(), now.clone()]);
            rows.push(vec!["connected_to".into(), connected.clone()]);
            if now != connected {
                rows.push(vec!["failover".into(), format!("{connected} → {now}")]);
            }
            rows.push(vec![
                "flags".into(),
                field(&map, "flags").unwrap_or_default(),
            ]);
            rows.push(vec![
                "quorum".into(),
                field(&map, "quorum").unwrap_or_default(),
            ]);
            rows.push(vec!["sentinels".into(), (others + 1).to_string()]);
            rows.push(vec!["asked".into(), format!("{host}:{port}")]);
        }
        Err(e) => rows.push(vec!["error".into(), e.to_string()]),
    }
    let mut replicas = redis::cmd("SENTINEL");
    replicas.arg("REPLICAS").arg(&link.name);
    if let Ok((v, _)) = link.ask(&replicas).await {
        for r in reply::list(&v) {
            let map = reply::pairs(r);
            rows.push(vec![
                "replica".into(),
                format!(
                    "{}:{} · {}",
                    field(&map, "ip").unwrap_or_default(),
                    field(&map, "port").unwrap_or_default(),
                    field(&map, "flags").unwrap_or_default()
                ),
            ]);
        }
    }
    rows
}
