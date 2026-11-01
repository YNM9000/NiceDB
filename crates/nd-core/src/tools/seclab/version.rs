use std::collections::BTreeMap;
use std::sync::{Arc, RwLock};
use std::time::{SystemTime, UNIX_EPOCH};

use nd_driver::{Fact, Flavor};
use serde::Deserialize;

use super::{Ctx, Outcome, SecSeverity, found, subject};
use crate::tools::Engine;

#[derive(Debug, Deserialize)]
pub(super) struct Advisory {
    cve: String,
    fixed: String,
    severity: String,
    summary: String,
}

#[derive(Debug, Deserialize)]
pub(super) struct Series {
    eol: Option<String>,
    advisories: Vec<Advisory>,
}

#[derive(Debug, Deserialize)]
pub(super) struct Product {
    pub sources: Vec<String>,
    series: BTreeMap<String, Series>,
}

#[derive(Debug, Deserialize)]
pub(super) struct Bundled {
    pub snapshot: String,
    pub products: BTreeMap<String, Product>,
}

static CURRENT: RwLock<Option<Arc<Bundled>>> = RwLock::new(None);

fn shipped() -> Bundled {
    serde_json::from_str(include_str!("versions.json")).unwrap_or(Bundled {
        snapshot: String::new(),
        products: BTreeMap::new(),
    })
}

pub(super) fn bundled() -> Arc<Bundled> {
    if let Ok(guard) = CURRENT.read()
        && let Some(list) = guard.as_ref()
    {
        return list.clone();
    }
    let list = Arc::new(shipped());
    if let Ok(mut guard) = CURRENT.write() {
        guard.get_or_insert_with(|| list.clone());
    }
    list
}

pub(crate) fn install(json: &str) -> Result<String, String> {
    let list: Bundled = serde_json::from_str(json).map_err(|e| e.to_string())?;
    if list.products.is_empty() || list.snapshot.len() != 10 {
        return Err("not a versions list".into());
    }
    let now = bundled();
    if list.snapshot < now.snapshot {
        return Ok(now.snapshot.clone());
    }
    let snapshot = list.snapshot.clone();
    if let Ok(mut guard) = CURRENT.write() {
        *guard = Some(Arc::new(list));
    }
    Ok(snapshot)
}

fn numbers(text: &str) -> Vec<u32> {
    let text = text
        .strip_prefix("5.5.5-")
        .filter(|_| text.contains("MariaDB"))
        .unwrap_or(text);
    let start = text
        .find(|c: char| c.is_ascii_digit())
        .unwrap_or(text.len());
    let mut out = Vec::new();
    for part in text[start..].split('.') {
        let digits: String = part.chars().take_while(char::is_ascii_digit).collect();
        let Ok(n) = digits.parse() else { break };
        out.push(n);
        if digits.len() != part.len() {
            break;
        }
    }
    out
}

fn at_least(have: &[u32], want: &[u32]) -> bool {
    let len = have.len().max(want.len());
    let pad = |v: &[u32]| -> Vec<u32> {
        v.iter()
            .copied()
            .chain(std::iter::repeat(0))
            .take(len)
            .collect()
    };
    pad(have) >= pad(want)
}

fn today() -> String {
    let days = i64::try_from(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |d| d.as_secs())
            / 86_400,
    )
    .unwrap_or(0);
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!("{year:04}-{month:02}-{day:02}")
}

fn product_key(engine: Engine) -> Option<&'static str> {
    match engine {
        Engine::Postgres => Some("postgresql"),
        Engine::Mysql => Some("mysql"),
        Engine::Mariadb => Some("mariadb"),
        Engine::Mssql => Some("sqlserver"),
        Engine::Sqlite => Some("sqlite"),
        _ => None,
    }
}

fn severity(s: &str) -> SecSeverity {
    match s {
        "high" => SecSeverity::High,
        "medium" => SecSeverity::Medium,
        _ => SecSeverity::Low,
    }
}

fn sql_server_major(n: &[u32]) -> Vec<u32> {
    let mut v = n.to_vec();
    if let Some(first) = v.first_mut() {
        *first = match *first {
            2012 => 11,
            2014 => 12,
            2016 => 13,
            2017 => 14,
            2019 => 15,
            2022 => 16,
            2025 => 17,
            other => other,
        };
    }
    v
}

pub(super) fn check(cx: &Ctx<'_>) -> Outcome {
    let Some(key) = product_key(cx.engine) else {
        return Outcome::Skip(super::SecSkip::Engine);
    };
    let list = bundled();
    let Some(product) = list.products.get(key) else {
        return Outcome::Skip(super::SecSkip::Engine);
    };
    let base = match cx.conn.edition() {
        Some(e)
            if matches!(
                e.flavor,
                Flavor::Yugabyte | Flavor::Tidb | Flavor::Planetscale
            ) =>
        {
            return Outcome::Skip(super::SecSkip::Engine);
        }
        Some(e) => e.facts.iter().find_map(|f| match f {
            Fact::Base(b) => Some(b.clone()),
            _ => None,
        }),
        None => None,
    };
    let text = base.as_ref().unwrap_or(&cx.conn.server().version);
    let mut have = numbers(text);
    if cx.engine == Engine::Mssql {
        have = sql_server_major(&have);
    }
    if have.is_empty() {
        return Outcome::Skip(super::SecSkip::NotDetectable);
    }
    let series_key = match cx.engine {
        Engine::Mysql | Engine::Mariadb => {
            format!("{}.{}", have[0], have.get(1).copied().unwrap_or(0))
        }
        Engine::Sqlite => "3".to_owned(),
        _ => have[0].to_string(),
    };
    let Some(series) = product.series.get(&series_key) else {
        let oldest = product
            .series
            .keys()
            .next()
            .map(|k| numbers(k))
            .unwrap_or_default();
        if !oldest.is_empty() && !at_least(&have, &oldest) {
            return found(
                SecSeverity::High,
                vec![subject(text.clone(), Some("eol".into()))],
                vec!["-- upgrade to a version that is still supported".into()],
            );
        }
        return Outcome::Passed;
    };
    let mut subjects = Vec::new();
    let mut top: Option<SecSeverity> = None;
    let mut newest: Vec<u32> = Vec::new();
    for a in &series.advisories {
        let fixed = numbers(&a.fixed);
        if at_least(&have, &fixed) {
            continue;
        }
        let s = severity(&a.severity);
        top = Some(top.map_or(s, |t| t.min(s)));
        if at_least(&fixed, &newest) {
            newest = fixed;
        }
        subjects.push(subject(
            a.cve.clone(),
            Some(format!("{} | {} | {}", a.fixed, a.severity, a.summary)),
        ));
    }
    let ended = series
        .eol
        .as_ref()
        .is_some_and(|d| d.as_str() < today().as_str());
    if ended {
        subjects.insert(
            0,
            subject(
                text.clone(),
                Some(format!("eol {}", series.eol.clone().unwrap_or_default())),
            ),
        );
        top = Some(top.map_or(SecSeverity::Medium, |t| t.min(SecSeverity::Medium)));
    }
    let Some(severity) = top else {
        return Outcome::Passed;
    };
    let target = newest
        .iter()
        .map(u32::to_string)
        .collect::<Vec<_>>()
        .join(".");
    let fix = if target.is_empty() {
        "-- upgrade to a version the vendor still supports".to_owned()
    } else {
        format!("-- upgrade to at least version {target} (same series)")
    };
    found(severity, subjects, vec![fix])
}
