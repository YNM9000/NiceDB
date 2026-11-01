use std::collections::BTreeMap;

use crate::{ColumnInfo, FkAction, ForeignKey, Function, Index, Namespace, Object, Relation};

#[derive(Debug, Default)]
pub struct CatalogRows {
    pub relations: Vec<Vec<Option<String>>>,
    pub columns: Vec<Vec<Option<String>>>,
    pub indexes: Vec<Vec<Option<String>>>,
    pub foreign_keys: Vec<Vec<Option<String>>>,
    pub routines: Vec<Vec<Option<String>>>,
    pub sequences: Vec<Vec<Option<String>>>,
    pub triggers: Vec<Vec<Option<String>>>,
}

fn s(r: &[Option<String>], i: usize) -> String {
    r.get(i)
        .cloned()
        .flatten()
        .map(|v| v.trim_end().to_owned())
        .unwrap_or_default()
}

fn yes(r: &[Option<String>], i: usize) -> bool {
    matches!(
        s(r, i).to_ascii_uppercase().as_str(),
        "Y" | "YES" | "TRUE" | "1" | "T"
    )
}

pub fn fk_action(word: &str) -> FkAction {
    match word.trim().to_ascii_uppercase().as_str() {
        "CASCADE" => FkAction::Cascade,
        "SET NULL" => FkAction::SetNull,
        "SET DEFAULT" => FkAction::SetDefault,
        "RESTRICT" => FkAction::Restrict,
        _ => FkAction::NoAction,
    }
}

#[derive(Default)]
struct Parts {
    tables: BTreeMap<String, Relation>,
    views: BTreeMap<String, Relation>,
    materialized: BTreeMap<String, Relation>,
}

impl Parts {
    fn get(&mut self, name: &str) -> Option<&mut Relation> {
        if let Some(r) = self.tables.get_mut(name) {
            return Some(r);
        }
        if let Some(r) = self.views.get_mut(name) {
            return Some(r);
        }
        self.materialized.get_mut(name)
    }
}

fn relation(name: String, rows: Option<f64>) -> Relation {
    Relation {
        name,
        columns: Vec::new(),
        indexes: Vec::new(),
        foreign_keys: Vec::new(),
        estimated_rows: rows,
        detail: None,
    }
}

fn add_objects(rows: &CatalogRows, spaces: &mut BTreeMap<String, Namespace>) {
    let ensure = |spaces: &mut BTreeMap<String, Namespace>, ns: &str| {
        spaces
            .entry(ns.to_owned())
            .or_insert_with(|| Namespace::new(ns))
            .name
            .clone()
    };
    for r in &rows.routines {
        let ns = s(r, 0);
        ensure(spaces, &ns);
        if let Some(n) = spaces.get_mut(&ns) {
            n.functions.push(Function {
                name: s(r, 1),
                signature: s(r, 2),
            });
        }
    }
    for r in &rows.sequences {
        let ns = s(r, 0);
        ensure(spaces, &ns);
        if let Some(n) = spaces.get_mut(&ns) {
            n.sequences.push(Object {
                name: s(r, 1),
                detail: None,
            });
        }
    }
    for r in &rows.triggers {
        let ns = s(r, 0);
        ensure(spaces, &ns);
        if let Some(n) = spaces.get_mut(&ns) {
            n.triggers.push(Object {
                name: s(r, 1),
                detail: Some(s(r, 2)).filter(|t| !t.is_empty()),
            });
        }
    }
}

pub fn assemble(rows: &CatalogRows, first: Option<&str>) -> Vec<Namespace> {
    let mut parts: BTreeMap<String, Parts> = BTreeMap::new();
    let mut spaces: BTreeMap<String, Namespace> = BTreeMap::new();
    for r in &rows.relations {
        let (ns, name) = (s(r, 0), s(r, 1));
        let estimate = s(r, 3).parse::<f64>().ok().filter(|n| *n >= 0.0);
        let p = parts.entry(ns.clone()).or_default();
        let rel = relation(name.clone(), estimate);
        match s(r, 2).as_str() {
            "view" => p.views.insert(name, rel),
            "materialized" => p.materialized.insert(name, rel),
            _ => p.tables.insert(name, rel),
        };
        spaces
            .entry(ns.clone())
            .or_insert_with(|| Namespace::new(ns));
    }
    let mut pk: BTreeMap<(String, String), Vec<String>> = BTreeMap::new();
    let mut indexes: BTreeMap<(String, String, String), Index> = BTreeMap::new();
    for r in &rows.indexes {
        let (ns, table, index, column) = (s(r, 0), s(r, 1), s(r, 2), s(r, 3));
        if yes(r, 5) {
            pk.entry((ns.clone(), table.clone()))
                .or_default()
                .push(column.clone());
        }
        indexes
            .entry((ns, table, index.clone()))
            .or_insert_with(|| Index {
                name: index,
                columns: Vec::new(),
                unique: yes(r, 4),
            })
            .columns
            .push(column);
    }
    for r in &rows.columns {
        let (ns, table, name) = (s(r, 0), s(r, 1), s(r, 2));
        let keyed = pk
            .get(&(ns.clone(), table.clone()))
            .is_some_and(|k| k.contains(&name));
        if let Some(rel) = parts.get_mut(&ns).and_then(|p| p.get(&table)) {
            rel.columns.push(ColumnInfo {
                name,
                type_name: s(r, 3),
                nullable: !matches!(
                    s(r, 4).to_ascii_uppercase().as_str(),
                    "N" | "NO" | "FALSE" | "0"
                ),
                primary_key: keyed,
            });
        }
    }
    for ((ns, table, _), index) in indexes {
        if let Some(rel) = parts.get_mut(&ns).and_then(|p| p.get(&table)) {
            rel.indexes.push(index);
        }
    }
    for r in &rows.foreign_keys {
        let (ns, table, name) = (s(r, 0), s(r, 1), s(r, 2));
        let Some(rel) = parts.get_mut(&ns).and_then(|p| p.get(&table)) else {
            continue;
        };
        if rel.foreign_keys.last().is_none_or(|f| f.name != name) {
            rel.foreign_keys.push(ForeignKey {
                name,
                columns: Vec::new(),
                ref_namespace: s(r, 3),
                ref_table: s(r, 4),
                ref_columns: Vec::new(),
                on_delete: fk_action(&s(r, 7)),
                on_update: fk_action(&s(r, 8)),
            });
        }
        if let Some(f) = rel.foreign_keys.last_mut() {
            f.columns.push(s(r, 5));
            f.ref_columns.push(s(r, 6));
        }
    }
    add_objects(rows, &mut spaces);
    for (ns, p) in parts {
        if let Some(n) = spaces.get_mut(&ns) {
            n.tables = p.tables.into_values().collect();
            n.views = p.views.into_values().collect();
            n.materialized_views = p.materialized.into_values().collect();
        }
    }
    let mut out: Vec<Namespace> = spaces.into_values().collect();
    if let Some(f) = first
        && let Some(at) = out.iter().position(|n| n.name.eq_ignore_ascii_case(f))
    {
        let own = out.remove(at);
        out.insert(0, own);
    }
    out
}
