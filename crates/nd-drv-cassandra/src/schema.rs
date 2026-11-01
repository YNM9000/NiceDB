use std::collections::BTreeMap;

use nd_driver::{ColumnInfo, Index, Namespace, Object, Relation};

pub(crate) struct Col {
    pub table: String,
    pub name: String,
    pub kind: String,
    pub position: i32,
    pub type_name: String,
    pub order: String,
}

fn rank(kind: &str) -> u8 {
    match kind {
        "partition_key" => 0,
        "clustering" => 1,
        "static" => 2,
        _ => 3,
    }
}

pub(crate) fn columns_of(mut cols: Vec<&Col>) -> (Vec<ColumnInfo>, String) {
    cols.sort_by(|a, b| {
        rank(&a.kind)
            .cmp(&rank(&b.kind))
            .then(a.position.cmp(&b.position))
            .then(a.name.cmp(&b.name))
    });
    let partition: Vec<&str> = cols
        .iter()
        .filter(|c| c.kind == "partition_key")
        .map(|c| c.name.as_str())
        .collect();
    let clustering: Vec<String> = cols
        .iter()
        .filter(|c| c.kind == "clustering")
        .map(|c| {
            if c.order.eq_ignore_ascii_case("desc") {
                format!("{} DESC", c.name)
            } else {
                c.name.clone()
            }
        })
        .collect();
    let mut key = format!("(({})", partition.join(", "));
    for c in &clustering {
        key.push_str(", ");
        key.push_str(c);
    }
    key.push(')');
    let info = cols
        .iter()
        .map(|c| ColumnInfo {
            name: c.name.clone(),
            type_name: if c.kind == "static" {
                format!("{} static", c.type_name)
            } else {
                c.type_name.clone()
            },
            nullable: !matches!(c.kind.as_str(), "partition_key" | "clustering"),
            primary_key: matches!(c.kind.as_str(), "partition_key" | "clustering"),
        })
        .collect();
    (info, key)
}

pub(crate) struct Catalog {
    pub keyspace: String,
    pub tables: Vec<String>,
    pub views: Vec<(String, String)>,
    pub columns: Vec<Col>,
    pub indexes: Vec<(String, String, String)>,
    pub types: Vec<String>,
}

fn relation(name: &str, cols: &[Col], detail_prefix: Option<&str>) -> Relation {
    let (columns, key) = columns_of(cols.iter().filter(|c| c.table == name).collect());
    let detail = match detail_prefix {
        Some(base) => format!("{base} · PRIMARY KEY {key}"),
        None => format!("PRIMARY KEY {key}"),
    };
    let pk: Vec<String> = columns
        .iter()
        .filter(|c| c.primary_key)
        .map(|c| c.name.clone())
        .collect();
    Relation {
        name: name.to_owned(),
        indexes: vec![Index {
            name: "PRIMARY KEY".into(),
            columns: pk,
            unique: true,
        }],
        columns,
        foreign_keys: Vec::new(),
        estimated_rows: None,
        detail: Some(detail),
    }
}

pub(crate) fn namespace(c: &Catalog) -> Namespace {
    let mut ns = Namespace::new(c.keyspace.clone());
    let mut by_table: BTreeMap<&str, Vec<Index>> = BTreeMap::new();
    for (table, name, target) in &c.indexes {
        by_table.entry(table.as_str()).or_default().push(Index {
            name: name.clone(),
            columns: vec![target.clone()],
            unique: false,
        });
    }
    for t in &c.tables {
        let mut r = relation(t, &c.columns, None);
        r.indexes
            .extend(by_table.remove(t.as_str()).unwrap_or_default());
        ns.tables.push(r);
    }
    for (v, base) in &c.views {
        ns.materialized_views
            .push(relation(v, &c.columns, Some(base)));
    }
    ns.types = c
        .types
        .iter()
        .map(|t| Object {
            name: t.clone(),
            detail: Some("type".into()),
        })
        .collect();
    ns
}

pub(crate) fn index_target(target: &str) -> String {
    let t = target.trim();
    let inner = t
        .find('(')
        .and_then(|a| t.rfind(')').map(|b| &t[a + 1..b]))
        .unwrap_or(t);
    inner.trim().trim_matches('"').to_owned()
}
