use std::collections::BTreeMap;

use nd_driver::{ColumnInfo, Namespace, Relation, Schema};
use serde_json::Value;

pub(crate) fn fields(mapping: &Value) -> Vec<ColumnInfo> {
    let mut out = vec![ColumnInfo {
        name: "_id".into(),
        type_name: "keyword".into(),
        nullable: false,
        primary_key: true,
    }];
    if let Some(props) = mapping.get("properties") {
        walk("", props, &mut out);
    }
    out
}

fn walk(prefix: &str, props: &Value, out: &mut Vec<ColumnInfo>) {
    let Some(props) = props.as_object() else {
        return;
    };
    for (name, def) in props {
        let path = if prefix.is_empty() {
            name.clone()
        } else {
            format!("{prefix}.{name}")
        };
        let kind =
            def.get("type")
                .and_then(Value::as_str)
                .unwrap_or(if def.get("properties").is_some() {
                    "object"
                } else {
                    "?"
                });
        out.push(ColumnInfo {
            name: path.clone(),
            type_name: kind.to_owned(),
            nullable: true,
            primary_key: false,
        });
        if let Some(sub) = def.get("properties") {
            walk(&path, sub, out);
        }
        if let Some(multi) = def.get("fields").and_then(Value::as_object) {
            for (m, d) in multi {
                out.push(ColumnInfo {
                    name: format!("{path}.{m}"),
                    type_name: d
                        .get("type")
                        .and_then(Value::as_str)
                        .unwrap_or("?")
                        .to_owned(),
                    nullable: true,
                    primary_key: false,
                });
            }
        }
    }
}

fn relation(
    name: &str,
    columns: Vec<ColumnInfo>,
    rows: Option<f64>,
    detail: Option<String>,
) -> Relation {
    Relation {
        name: name.to_owned(),
        columns,
        indexes: Vec::new(),
        foreign_keys: Vec::new(),
        estimated_rows: rows,
        detail,
    }
}

pub(crate) struct Sources {
    pub cluster: String,
    pub patterns: String,
    pub indices: Value,
    pub mappings: Value,
    pub aliases: Value,
    pub streams: Value,
}

pub(crate) fn wanted(name: &str, patterns: &str) -> bool {
    let mut any = false;
    for p in patterns.split(',').map(str::trim).filter(|p| !p.is_empty()) {
        any = true;
        if glob(name.as_bytes(), p.as_bytes()) {
            return true;
        }
    }
    !any
}

fn glob(name: &[u8], pattern: &[u8]) -> bool {
    match pattern.split_first() {
        None => name.is_empty(),
        Some((b'*', rest)) => (0..=name.len()).any(|i| glob(&name[i..], rest)),
        Some((c, rest)) => name.first() == Some(c) && glob(&name[1..], rest),
    }
}

#[allow(clippy::too_many_lines)]
pub(crate) fn build(s: &Sources) -> Schema {
    let mut ns = Namespace::new(s.cluster.clone());
    let mapping_of = |index: &str| {
        s.mappings
            .get(index)
            .and_then(|m| m.get("mappings"))
            .map(fields)
            .unwrap_or_default()
    };
    let mut docs: BTreeMap<String, f64> = BTreeMap::new();
    for row in s.indices.as_array().into_iter().flatten() {
        let Some(name) = row.get("index").and_then(Value::as_str) else {
            continue;
        };
        let count = row
            .get("docs.count")
            .and_then(Value::as_str)
            .and_then(|c| c.parse::<f64>().ok());
        if let Some(c) = count {
            docs.insert(name.to_owned(), c);
        }
        if name.starts_with('.') || !wanted(name, &s.patterns) {
            continue;
        }
        let detail = [
            row.get("health").and_then(Value::as_str),
            row.get("store.size").and_then(Value::as_str),
        ]
        .into_iter()
        .flatten()
        .collect::<Vec<_>>()
        .join(" · ");
        ns.tables.push(relation(
            name,
            mapping_of(name),
            count,
            Some(detail).filter(|d| !d.is_empty()),
        ));
    }
    ns.tables.sort_by(|a, b| a.name.cmp(&b.name));
    let mut aliases: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for (index, v) in s.aliases.as_object().into_iter().flatten() {
        if index.starts_with('.') {
            continue;
        }
        for alias in v
            .get("aliases")
            .and_then(Value::as_object)
            .into_iter()
            .flatten()
            .map(|(a, _)| a)
        {
            aliases
                .entry(alias.clone())
                .or_default()
                .push(index.clone());
        }
    }
    for (alias, indices) in aliases {
        if alias.starts_with('.') || !wanted(&alias, &s.patterns) {
            continue;
        }
        let rows = indices.iter().filter_map(|i| docs.get(i)).sum::<f64>();
        let columns = indices.first().map(|i| mapping_of(i)).unwrap_or_default();
        ns.aliases.push(relation(
            &alias,
            columns,
            Some(rows),
            Some(indices.join(", ")),
        ));
    }
    for stream in s
        .streams
        .get("data_streams")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        let Some(name) = stream
            .get("name")
            .and_then(Value::as_str)
            .filter(|n| wanted(n, &s.patterns))
        else {
            continue;
        };
        let backing: Vec<&str> = stream
            .get("indices")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(|i| i.get("index_name").and_then(Value::as_str))
            .collect();
        let rows = backing.iter().filter_map(|i| docs.get(*i)).sum::<f64>();
        let columns = backing.last().map(|i| mapping_of(i)).unwrap_or_default();
        let detail = stream
            .get("status")
            .and_then(Value::as_str)
            .map(|h| format!("{} · {}", h.to_lowercase(), backing.len()));
        ns.data_streams
            .push(relation(name, columns, Some(rows), detail));
    }
    Schema {
        databases: vec![s.cluster.clone()],
        current_database: s.cluster.clone(),
        namespaces: vec![ns],
    }
}

pub(crate) fn creatable_settings(settings: &Value) -> Value {
    let mut index = settings
        .get("index")
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();
    for k in [
        "creation_date",
        "uuid",
        "version",
        "provided_name",
        "resize",
        "replication",
    ] {
        index.remove(k);
    }
    Value::Object(index)
}
