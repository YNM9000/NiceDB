use std::collections::HashMap;

use mongodb::bson::{Bson, Document};
use nd_driver::ColumnInfo;

const MAX_DEPTH: usize = 4;
const MAX_FIELDS: usize = 300;

#[derive(Default)]
struct Stats {
    present: usize,
    types: Vec<(&'static str, usize)>,
}

impl Stats {
    fn saw(&mut self, kind: &'static str) {
        match self.types.iter_mut().find(|(k, _)| *k == kind) {
            Some((_, n)) => *n += 1,
            None => self.types.push((kind, 1)),
        }
    }
}

pub fn type_name(v: &Bson) -> &'static str {
    match v {
        Bson::Double(_) => "double",
        Bson::String(_) => "string",
        Bson::Array(_) => "array",
        Bson::Document(_) => "document",
        Bson::Boolean(_) => "bool",
        Bson::Null => "null",
        Bson::RegularExpression(_) => "regex",
        Bson::JavaScriptCode(_) | Bson::JavaScriptCodeWithScope(_) => "javascript",
        Bson::Int32(_) => "int",
        Bson::Int64(_) => "long",
        Bson::Timestamp(_) => "timestamp",
        Bson::Binary(_) => "binData",
        Bson::ObjectId(_) => "objectId",
        Bson::DateTime(_) => "date",
        Bson::Symbol(_) => "symbol",
        Bson::Decimal128(_) => "decimal",
        Bson::Undefined => "undefined",
        Bson::MaxKey => "maxKey",
        Bson::MinKey => "minKey",
        Bson::DbPointer(_) => "dbPointer",
    }
}

pub fn columns(sample: &[Document]) -> Vec<ColumnInfo> {
    let mut order: Vec<String> = Vec::new();
    let mut stats: HashMap<String, Stats> = HashMap::new();
    for doc in sample {
        let mut seen: Vec<String> = Vec::new();
        walk(doc, "", 0, &mut |path, v| {
            if !stats.contains_key(path) {
                if order.len() >= MAX_FIELDS {
                    return;
                }
                order.push(path.to_owned());
            }
            let s = stats.entry(path.to_owned()).or_default();
            if !seen.iter().any(|p| p == path) {
                s.present += 1;
                seen.push(path.to_owned());
            }
            s.saw(type_name(v));
        });
    }
    order
        .into_iter()
        .filter_map(|path| {
            let s = stats.remove(&path)?;
            Some(ColumnInfo {
                primary_key: path == "_id",
                nullable: s.present < sample.len() || s.types.iter().any(|(k, _)| *k == "null"),
                type_name: describe(&s),
                name: path,
            })
        })
        .collect()
}

fn describe(s: &Stats) -> String {
    let mut types = s.types.clone();
    types.sort_by_key(|t| std::cmp::Reverse(t.1));
    if let [(only, _)] = types.as_slice() {
        return (*only).to_owned();
    }
    let total: usize = types.iter().map(|(_, n)| n).sum();
    types
        .iter()
        .map(|(k, n)| format!("{k} {}%", (n * 100 + total / 2) / total.max(1)))
        .collect::<Vec<_>>()
        .join(" · ")
}

fn walk(doc: &Document, prefix: &str, depth: usize, f: &mut impl FnMut(&str, &Bson)) {
    for (k, v) in doc {
        let path = if prefix.is_empty() {
            k.clone()
        } else {
            format!("{prefix}.{k}")
        };
        f(&path, v);
        if depth + 1 >= MAX_DEPTH {
            continue;
        }
        match v {
            Bson::Document(d) => walk(d, &path, depth + 1, f),
            Bson::Array(items) => {
                for item in items {
                    if let Bson::Document(d) = item {
                        walk(d, &path, depth + 1, f);
                    }
                }
            }
            _ => {}
        }
    }
}
