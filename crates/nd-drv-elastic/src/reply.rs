use std::fmt;
use std::fmt::Write as _;

use nd_driver::{Column, DriverError, Row};
use serde::Deserialize;
use serde::de::{Deserializer, MapAccess, Visitor};
use serde_json::Value;
use serde_json::value::RawValue;

#[derive(Debug, Default)]
pub(crate) struct Ordered(pub Vec<(String, Value)>);

impl<'de> Deserialize<'de> for Ordered {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct V;
        impl<'de> Visitor<'de> for V {
            type Value = Ordered;
            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str("an object")
            }
            fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Ordered, A::Error> {
                let mut out = Vec::new();
                while let Some((k, v)) = map.next_entry::<String, Value>()? {
                    out.push((k, v));
                }
                Ok(Ordered(out))
            }
        }
        d.deserialize_map(V)
    }
}

#[derive(Deserialize)]
struct Hit<'a> {
    #[serde(rename = "_index", default)]
    index: Option<String>,
    #[serde(rename = "_id", default)]
    id: Option<String>,
    #[serde(rename = "_source", borrow, default)]
    source: Option<&'a RawValue>,
    #[serde(borrow, default)]
    fields: Option<&'a RawValue>,
}

#[derive(Deserialize)]
struct Hits<'a> {
    #[serde(borrow, default)]
    hits: Vec<Hit<'a>>,
}

#[derive(Deserialize)]
struct SearchReply<'a> {
    #[serde(borrow)]
    hits: Option<Hits<'a>>,
    #[serde(default)]
    aggregations: Option<serde_json::Map<String, Value>>,
}

#[derive(Debug, PartialEq)]
pub(crate) enum Shaped {
    Documents(Vec<String>),
    Grid(Vec<Column>, Vec<Row>),
}

fn bad(e: impl fmt::Display) -> DriverError {
    DriverError::query(format!("the server's reply is not valid JSON: {e}"))
}

pub(crate) fn hit_document(index: Option<&str>, id: Option<&str>, source: Option<&str>) -> String {
    let mut out = String::from("{");
    let mut first = true;
    for (k, v) in [("_id", id), ("_index", index)] {
        if let Some(v) = v {
            if !first {
                out.push(',');
            }
            first = false;
            let _ = write!(out, "{}:{}", Value::from(k), Value::from(v));
        }
    }
    let inner = source
        .map(str::trim)
        .and_then(|s| s.strip_prefix('{'))
        .and_then(|s| s.strip_suffix('}'))
        .map(str::trim)
        .filter(|s| !s.is_empty());
    if let Some(inner) = inner {
        if !first {
            out.push(',');
        }
        out.push_str(inner);
    }
    out.push('}');
    out
}

pub(crate) fn shape(body: &str) -> Result<Shaped, DriverError> {
    let text = body.trim();
    if text.is_empty() {
        return Ok(Shaped::Documents(Vec::new()));
    }
    if text.starts_with('[') {
        let items: Vec<Ordered> = serde_json::from_str(text).map_err(bad)?;
        let (columns, rows) = table(&items);
        return Ok(Shaped::Grid(columns, rows));
    }
    if text.starts_with('{')
        && let Ok(reply) = serde_json::from_str::<SearchReply>(text)
        && let Some(hits) = reply.hits
    {
        if hits.hits.is_empty()
            && let Some(aggs) = reply.aggregations.filter(|a| !a.is_empty())
        {
            let (columns, rows) = aggregations(&aggs);
            return Ok(Shaped::Grid(columns, rows));
        }
        let docs = hits
            .hits
            .iter()
            .map(|h| {
                hit_document(
                    h.index.as_deref(),
                    h.id.as_deref(),
                    h.source.or(h.fields).map(RawValue::get),
                )
            })
            .collect();
        return Ok(Shaped::Documents(docs));
    }
    if text.starts_with('{') {
        serde_json::from_str::<&RawValue>(text).map_err(bad)?;
        return Ok(Shaped::Documents(vec![text.to_owned()]));
    }
    Ok(Shaped::Grid(
        vec![Column {
            name: "text".into(),
            type_name: None,
        }],
        text.lines().map(|l| vec![Some(l.to_owned())]).collect(),
    ))
}

fn numeric(rows: &[Row], i: usize) -> Option<String> {
    let mut any = false;
    for r in rows {
        if let Some(Some(v)) = r.get(i) {
            if v.parse::<f64>().is_err() {
                return None;
            }
            any = true;
        }
    }
    any.then(|| "double".to_owned())
}

fn typed(names: Vec<String>, rows: &[Row]) -> Vec<Column> {
    names
        .into_iter()
        .enumerate()
        .map(|(i, name)| Column {
            name,
            type_name: numeric(rows, i),
        })
        .collect()
}

pub(crate) fn cell(v: &Value) -> Option<String> {
    match v {
        Value::Null => None,
        Value::String(s) => Some(s.clone()),
        other => Some(other.to_string()),
    }
}

pub(crate) fn table(items: &[Ordered]) -> (Vec<Column>, Vec<Row>) {
    let mut names: Vec<String> = Vec::new();
    for item in items {
        for (k, _) in &item.0 {
            if !names.contains(k) {
                names.push(k.clone());
            }
        }
    }
    let rows: Vec<Row> = items
        .iter()
        .map(|item| {
            names
                .iter()
                .map(|n| {
                    item.0
                        .iter()
                        .find(|(k, _)| k == n)
                        .and_then(|(_, v)| cell(v))
                })
                .collect()
        })
        .collect();
    (typed(names, &rows), rows)
}

type Cells = Vec<(String, Option<String>)>;

const BUCKET_META: [&str; 6] = [
    "key",
    "key_as_string",
    "doc_count",
    "from",
    "to",
    "doc_count_error_upper_bound",
];

fn metrics(name: &str, v: &Value, out: &mut Cells) {
    let Some(o) = v.as_object() else {
        return;
    };
    if let Some(value) = o.get("value") {
        out.push((
            name.to_owned(),
            o.get("value_as_string")
                .and_then(cell)
                .or_else(|| cell(value)),
        ));
    } else if let Some(values) = o.get("values").and_then(Value::as_object) {
        for (k, x) in values {
            out.push((format!("{name}.{k}"), cell(x)));
        }
    } else if o.contains_key("count") && o.contains_key("sum") {
        for k in ["count", "min", "max", "avg", "sum"] {
            if let Some(x) = o.get(k) {
                out.push((format!("{name}.{k}"), cell(x)));
            }
        }
    } else if let Some(n) = o.get("doc_count").filter(|_| !o.contains_key("buckets")) {
        out.push((format!("{name}.doc_count"), cell(n)));
    }
}

fn buckets(v: &Value) -> Vec<(Option<String>, &serde_json::Map<String, Value>)> {
    match v.get("buckets") {
        Some(Value::Array(list)) => list
            .iter()
            .filter_map(Value::as_object)
            .map(|b| {
                let key = b
                    .get("key_as_string")
                    .or_else(|| b.get("key"))
                    .and_then(cell);
                (key, b)
            })
            .collect(),
        Some(Value::Object(keyed)) => keyed
            .iter()
            .filter_map(|(k, b)| b.as_object().map(|b| (Some(k.clone()), b)))
            .collect(),
        _ => Vec::new(),
    }
}

fn flatten(prefix: &Cells, level: &serde_json::Map<String, Value>, rows: &mut Vec<Cells>) {
    let mut own = prefix.clone();
    let mut nested = Vec::new();
    for (name, v) in level {
        if BUCKET_META.contains(&name.as_str()) || !v.is_object() {
            continue;
        }
        if v.get("buckets").is_some() {
            nested.push((name, v));
        } else if let Some(sub) = v
            .as_object()
            .filter(|o| o.contains_key("doc_count") && o.values().any(Value::is_object))
        {
            own.push((
                format!("{name}.doc_count"),
                sub.get("doc_count").and_then(cell),
            ));
            nested.push((name, v));
        } else {
            metrics(name, v, &mut own);
        }
    }
    if nested.is_empty() {
        rows.push(own);
        return;
    }
    for (name, v) in nested {
        let list = buckets(v);
        if list.is_empty() {
            if let Some(single) = v.as_object().filter(|o| !o.contains_key("buckets")) {
                flatten(&own, single, rows);
            }
            continue;
        }
        for (key, bucket) in list {
            let mut next = own.clone();
            next.push((name.clone(), key));
            next.push((
                format!("{name}.doc_count"),
                bucket.get("doc_count").and_then(cell),
            ));
            flatten(&next, bucket, rows);
        }
    }
}

pub(crate) fn aggregations(aggs: &serde_json::Map<String, Value>) -> (Vec<Column>, Vec<Row>) {
    let mut rows = Vec::new();
    flatten(&Vec::new(), aggs, &mut rows);
    let mut names: Vec<String> = Vec::new();
    for row in &rows {
        for (k, _) in row {
            if !names.contains(k) {
                names.push(k.clone());
            }
        }
    }
    let grid: Vec<Row> = rows
        .iter()
        .map(|r| {
            names
                .iter()
                .map(|n| r.iter().find(|(k, _)| k == n).and_then(|(_, v)| v.clone()))
                .collect()
        })
        .collect();
    (typed(names, &grid), grid)
}

pub(crate) fn error(status: u16, body: &str) -> DriverError {
    let v: Value = serde_json::from_str(body).unwrap_or(Value::Null);
    let e = v.get("error");
    let reason = e
        .and_then(|e| e.get("reason"))
        .and_then(Value::as_str)
        .or_else(|| e.and_then(Value::as_str))
        .map(str::to_owned);
    let kind = e.and_then(|e| e.get("type")).and_then(Value::as_str);
    let cause = e
        .and_then(|e| e.get("root_cause"))
        .and_then(|c| c.get(0))
        .and_then(|c| c.get("reason"))
        .and_then(Value::as_str)
        .filter(|c| Some(*c) != reason.as_deref());
    let message = reason.unwrap_or_else(|| {
        let t = body.trim();
        if t.is_empty() {
            format!("HTTP {status}")
        } else {
            t.chars().take(2000).collect()
        }
    });
    if status == 401 || status == 403 {
        return DriverError::Auth { message };
    }
    DriverError::Query {
        message,
        code: kind.map(str::to_owned),
        detail: cause.map(str::to_owned),
        hint: None,
        offset: None,
    }
}

pub(crate) fn affected(body: &str) -> Option<u64> {
    let v: Value = serde_json::from_str(body).ok()?;
    if let Some(r) = v.get("result").and_then(Value::as_str) {
        return Some(u64::from(matches!(r, "created" | "updated" | "deleted")));
    }
    if let Some(items) = v.get("items").and_then(Value::as_array) {
        return Some(items.len() as u64);
    }
    ["deleted", "updated", "created"]
        .iter()
        .filter_map(|k| v.get(*k).and_then(Value::as_u64))
        .reduce(|a, b| a + b)
}
