use std::collections::BTreeMap;

use serde::Serialize;
use serde_json::Value;

use crate::ConnectionProfile;

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error, Serialize, specta::Type)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum VarError {
    #[error("{field}: ${{{name}}} is not defined")]
    Undefined { name: String, field: String },
    #[error("{field}: ${{{name}}} looks like a secret; secrets stay in the keychain")]
    Secret { name: String, field: String },
    #[error("{field}: unclosed ${{")]
    Syntax { field: String },
    #[error("{field}: {value} is not a valid number")]
    BadNumber { field: String, value: String },
}

pub fn secret_like(name: &str) -> bool {
    let upper = name.to_ascii_uppercase();
    [
        "PASSWORD",
        "PASSWD",
        "PWD",
        "SECRET",
        "TOKEN",
        "PASSPHRASE",
        "API_KEY",
        "PRIVATE_KEY",
    ]
    .iter()
    .any(|w| upper.contains(w))
}

pub fn valid_name(name: &str) -> bool {
    let mut chars = name.chars();
    chars
        .next()
        .is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
        && chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

pub fn expand(
    text: &str,
    field: &str,
    lookup: &dyn Fn(&str) -> Option<String>,
) -> Result<String, VarError> {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(at) = rest.find('$') {
        out.push_str(&rest[..at]);
        let tail = &rest[at..];
        if let Some(after) = tail.strip_prefix("$${") {
            out.push_str("${");
            rest = after;
        } else if let Some(after) = tail.strip_prefix("${") {
            let end = after.find('}').ok_or_else(|| VarError::Syntax {
                field: field.to_owned(),
            })?;
            let inner = &after[..end];
            let (name, fallback) = match inner.split_once(":-") {
                Some((n, f)) => (n.trim(), Some(f)),
                None => (inner.trim(), None),
            };
            if !valid_name(name) {
                return Err(VarError::Syntax {
                    field: field.to_owned(),
                });
            }
            if secret_like(name) {
                return Err(VarError::Secret {
                    name: name.to_owned(),
                    field: field.to_owned(),
                });
            }
            match lookup(name).or_else(|| fallback.map(str::to_owned)) {
                Some(v) => out.push_str(&v),
                None => {
                    return Err(VarError::Undefined {
                        name: name.to_owned(),
                        field: field.to_owned(),
                    });
                }
            }
            rest = &after[end + 1..];
        } else {
            out.push('$');
            rest = &tail[1..];
        }
    }
    out.push_str(rest);
    Ok(out)
}

pub fn names(text: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let mut rest = text;
    while let Some(at) = rest.find("${") {
        if at > 0 && rest[..at].ends_with('$') {
            rest = &rest[at + 2..];
            continue;
        }
        let after = &rest[at + 2..];
        let Some(end) = after.find('}') else { break };
        let name = after[..end].split(":-").next().unwrap_or_default().trim();
        if valid_name(name) && !out.iter().any(|n| n == name) {
            out.push(name.to_owned());
        }
        rest = &after[end + 1..];
    }
    out
}

fn walk(
    value: &mut Value,
    path: &str,
    lookup: &dyn Fn(&str) -> Option<String>,
) -> Result<(), VarError> {
    match value {
        Value::String(s) if s.contains('$') => *s = expand(s, path, lookup)?,
        Value::Object(map) => {
            for (k, v) in map.iter_mut() {
                let p = if path.is_empty() {
                    k.clone()
                } else {
                    format!("{path}.{k}")
                };
                walk(v, &p, lookup)?;
            }
        }
        Value::Array(items) => {
            for (i, v) in items.iter_mut().enumerate() {
                walk(v, &format!("{path}.{i}"), lookup)?;
            }
        }
        _ => {}
    }
    Ok(())
}

fn set_path(root: &mut Value, path: &str, v: Value) {
    let mut at = root;
    for key in path.split('.') {
        at = match at {
            Value::Object(map) => match map.get_mut(key) {
                Some(next) => next,
                None => return,
            },
            Value::Array(items) => match key.parse::<usize>().ok().and_then(|i| items.get_mut(i)) {
                Some(next) => next,
                None => return,
            },
            _ => return,
        };
    }
    *at = v;
}

pub fn resolve(
    p: &ConnectionProfile,
    lookup: &dyn Fn(&str) -> Option<String>,
) -> Result<ConnectionProfile, VarError> {
    let mut target = serde_json::to_value(&p.target).unwrap_or(Value::Null);
    walk(&mut target, "", lookup)?;
    for (path, raw) in &p.raw_fields {
        let text = expand(raw, path, lookup)?;
        let n = text
            .trim()
            .parse::<u16>()
            .ok()
            .filter(|n| *n > 0 || path == "database")
            .ok_or_else(|| VarError::BadNumber {
                field: path.clone(),
                value: text.clone(),
            })?;
        set_path(&mut target, path, Value::from(n));
    }
    let mut out = p.clone();
    out.target = serde_json::from_value(target).map_err(|_| VarError::Syntax {
        field: "target".into(),
    })?;
    if out.session.init.contains('$') {
        out.session.init = expand(&p.session.init, "session.init", lookup)?;
    }
    out.raw_fields = BTreeMap::new();
    Ok(out)
}

pub fn lookup_in(vars: &BTreeMap<String, String>) -> impl Fn(&str) -> Option<String> + '_ {
    |name| vars.get(name).cloned().or_else(|| std::env::var(name).ok())
}
