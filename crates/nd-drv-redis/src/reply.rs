use std::fmt::Write as _;

use nd_driver::{Column, Row};
use redis::Value;

pub fn text(bytes: &[u8]) -> String {
    match std::str::from_utf8(bytes) {
        Ok(s) => s.to_owned(),
        Err(_) => escaped(bytes),
    }
}

pub fn escaped(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for chunk in bytes.utf8_chunks() {
        out.push_str(chunk.valid());
        for b in chunk.invalid() {
            let _ = write!(out, "\\x{b:02x}");
        }
    }
    out
}

pub fn scalar(v: &Value) -> Option<String> {
    match v {
        Value::BulkString(b) => Some(text(b)),
        Value::SimpleString(s) => Some(s.clone()),
        Value::VerbatimString { text, .. } => Some(text.clone()),
        Value::Okay => Some("OK".into()),
        Value::Int(n) => Some(n.to_string()),
        Value::Double(d) => Some(d.to_string()),
        Value::Boolean(b) => Some(b.to_string()),
        Value::BigNumber(n) => Some(text(n)),
        Value::ServerError(e) => Some(format!("{e:?}")),
        Value::Attribute { data, .. } => scalar(data),
        _ => None,
    }
}

const fn is_nested(v: &Value) -> bool {
    matches!(
        v,
        Value::Array(_) | Value::Set(_) | Value::Map(_) | Value::Push { .. }
    )
}

fn to_json(v: &Value) -> serde_json::Value {
    use serde_json::Value as J;
    match v {
        Value::Nil => J::Null,
        Value::Int(n) => J::from(*n),
        Value::Double(d) => J::from(*d),
        Value::Boolean(b) => J::Bool(*b),
        Value::Array(items) | Value::Set(items) | Value::Push { data: items, .. } => {
            J::Array(items.iter().map(to_json).collect())
        }
        Value::Map(pairs) => {
            let all_text = pairs.iter().all(|(k, _)| scalar(k).is_some());
            if all_text {
                J::Object(
                    pairs
                        .iter()
                        .map(|(k, v)| (scalar(k).unwrap_or_default(), to_json(v)))
                        .collect(),
                )
            } else {
                J::Array(
                    pairs
                        .iter()
                        .map(|(k, v)| J::Array(vec![to_json(k), to_json(v)]))
                        .collect(),
                )
            }
        }
        Value::Attribute { data, .. } => to_json(data),
        other => scalar(other).map_or(J::Null, J::String),
    }
}

pub fn cell(v: &Value) -> Option<String> {
    if is_nested(v) {
        Some(to_json(v).to_string())
    } else {
        scalar(v)
    }
}

fn items(v: &Value) -> Option<&[Value]> {
    match v {
        Value::Array(items) | Value::Set(items) | Value::Push { data: items, .. } => Some(items),
        Value::Attribute { data, .. } => items(data),
        _ => None,
    }
}

fn col(name: &str) -> Column {
    Column {
        name: name.into(),
        type_name: None,
    }
}

pub fn table(v: &Value) -> (Vec<Column>, Vec<Row>) {
    if let Value::Map(pairs) = v {
        let rows = pairs.iter().map(|(k, v)| vec![cell(k), cell(v)]).collect();
        return (vec![col("field"), col("value")], rows);
    }
    let Some(list) = items(v) else {
        return (vec![col("value")], vec![vec![cell(v)]]);
    };
    let width = list
        .iter()
        .map(|i| items(i).map_or(0, <[Value]>::len))
        .max()
        .unwrap_or(0);
    let all_lists = !list.is_empty() && list.iter().all(|i| items(i).is_some());
    if all_lists && width > 1 {
        let mut columns = vec![col("#")];
        columns.extend((1..=width).map(|i| col(&i.to_string())));
        let rows = list
            .iter()
            .enumerate()
            .map(|(n, i)| {
                let inner = items(i).unwrap_or_default();
                let mut row = vec![Some((n + 1).to_string())];
                row.extend((0..width).map(|c| inner.get(c).and_then(cell)));
                row
            })
            .collect();
        return (columns, rows);
    }
    let rows = list
        .iter()
        .enumerate()
        .map(|(n, i)| vec![Some((n + 1).to_string()), cell(i)])
        .collect();
    (vec![col("#"), col("value")], rows)
}

pub fn pairs(v: &Value) -> Vec<(&Value, &Value)> {
    match v {
        Value::Map(p) => p.iter().map(|(k, v)| (k, v)).collect(),
        Value::Attribute { data, .. } => pairs(data),
        other => items(other)
            .map(|list| {
                if list.iter().all(|i| items(i).is_some_and(|p| p.len() == 2)) {
                    list.iter()
                        .filter_map(|i| items(i).and_then(|p| Some((p.first()?, p.get(1)?))))
                        .collect()
                } else {
                    list.as_chunks::<2>()
                        .0
                        .iter()
                        .map(|[a, b]| (a, b))
                        .collect()
                }
            })
            .unwrap_or_default(),
    }
}

pub fn list(v: &Value) -> &[Value] {
    items(v).unwrap_or_default()
}

pub fn fields_json(v: &Value) -> String {
    let mut out = String::from("{");
    for (i, (k, v)) in pairs(v).into_iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        let k = serde_json::Value::String(scalar(k).unwrap_or_default());
        let v = serde_json::Value::String(scalar(v).unwrap_or_default());
        let _ = write!(out, "{k}:{v}");
    }
    out.push('}');
    out
}
