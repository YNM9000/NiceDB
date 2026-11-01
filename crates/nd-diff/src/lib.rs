pub mod design;
pub mod rows;
mod script;

use nd_driver::{Dialect, FkAction};
use serde::Serialize;

pub use design::{DesignEngine, TableDesign, design_script};
pub use script::script;

#[derive(Debug, Clone, Default)]
pub struct Snapshot {
    pub namespace: String,
    pub tables: Vec<Table>,
    pub views: Vec<View>,
    pub routines: Vec<Routine>,
}

#[derive(Debug, Clone, Default)]
pub struct Table {
    pub name: String,
    pub columns: Vec<Col>,
    pub primary_key: Vec<String>,
    pub pk_name: Option<String>,
    pub indexes: Vec<Idx>,
    pub foreign_keys: Vec<Fk>,
    pub create: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Col {
    pub name: String,
    pub type_name: String,
    pub nullable: bool,
    pub default: Option<String>,
    pub generated: bool,
    pub default_name: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Idx {
    pub name: String,
    pub columns: Vec<String>,
    pub unique: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Fk {
    pub name: String,
    pub columns: Vec<String>,
    pub ref_table: String,
    pub ref_columns: Vec<String>,
    pub on_delete: FkAction,
    pub on_update: FkAction,
}

#[derive(Debug, Clone)]
pub struct View {
    pub name: String,
    pub definition: Option<String>,
}

#[derive(Debug, Clone)]
pub struct Routine {
    pub name: String,
    pub signature: String,
    pub procedure: bool,
    pub definition: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "snake_case")]
pub enum ObjectKind {
    Table,
    Column,
    PrimaryKey,
    Index,
    ForeignKey,
    View,
    Function,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "snake_case")]
pub enum DiffState {
    Added,
    Removed,
    Changed,
    Same,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "snake_case")]
pub enum Note {
    Destructive,
    Rebuild,
    NoDefinition,
}

#[derive(Debug, Clone, Serialize, specta::Type)]
pub struct DiffItem {
    pub kind: ObjectKind,
    pub table: Option<String>,
    pub name: String,
    pub state: DiffState,
    pub source: Option<String>,
    pub target: Option<String>,
    pub notes: Vec<Note>,
}

pub(crate) fn in_place_keys(dialect: Dialect) -> bool {
    !matches!(
        dialect,
        Dialect::Sqlite | Dialect::Duckdb | Dialect::Clickhouse
    )
}

pub(crate) fn computed_kind(default: &str) -> bool {
    ["MATERIALIZED ", "ALIAS ", "EPHEMERAL"]
        .iter()
        .any(|k| default.starts_with(k))
}

fn squash(s: &str) -> String {
    s.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .trim_end_matches(';')
        .to_ascii_lowercase()
}

fn same_type(a: &str, b: &str) -> bool {
    let norm = |t: &str| {
        let t = squash(t);
        match t.as_str() {
            "int4" | "int" | "integer" | "serial" | "int(11)" => "integer".to_owned(),
            "int8" | "bigint" | "bigserial" => "bigint".to_owned(),
            "int2" | "smallint" => "smallint".to_owned(),
            "bool" | "boolean" => "boolean".to_owned(),
            "float8" | "double precision" | "double" => "double".to_owned(),
            _ => {
                let base = t.split('(').next().unwrap_or_default();
                if matches!(base, "int" | "bigint" | "smallint" | "mediumint") {
                    base.to_owned() + t.split_once(')').map_or("", |(_, rest)| rest)
                } else {
                    t.replace(", ", ",")
                }
            }
        }
    };
    norm(a) == norm(b)
}

pub(crate) fn describe_col(c: &Col) -> String {
    let mut s = c.type_name.clone();
    if !c.nullable {
        s.push_str(" NOT NULL");
    }
    if c.generated {
        s.push_str(" · auto");
    } else if let Some(d) = &c.default {
        s.push_str(if computed_kind(d) { " " } else { " DEFAULT " });
        s.push_str(d);
    }
    s
}

fn describe_idx(i: &Idx) -> String {
    let unique = if i.unique { "UNIQUE " } else { "" };
    format!("{unique}({})", i.columns.join(", "))
}

pub(crate) fn describe_fk(f: &Fk) -> String {
    format!(
        "({}) → {}({})",
        f.columns.join(", "),
        f.ref_table,
        f.ref_columns.join(", ")
    )
}

fn same_col(a: &Col, b: &Col) -> bool {
    same_type(&a.type_name, &b.type_name)
        && a.nullable == b.nullable
        && a.generated == b.generated
        && (a.generated || a.default.as_deref().map(squash) == b.default.as_deref().map(squash))
}

pub(crate) fn action_sql(a: FkAction) -> &'static str {
    match a {
        FkAction::NoAction => "NO ACTION",
        FkAction::Restrict => "RESTRICT",
        FkAction::Cascade => "CASCADE",
        FkAction::SetNull => "SET NULL",
        FkAction::SetDefault => "SET DEFAULT",
    }
}

pub(crate) fn same_action(a: FkAction, b: FkAction) -> bool {
    let restrict = |x| matches!(x, FkAction::NoAction | FkAction::Restrict);
    a == b || (restrict(a) && restrict(b))
}

fn fk_key(f: &Fk) -> (Vec<String>, String, Vec<String>) {
    (
        f.columns.clone(),
        f.ref_table.clone(),
        f.ref_columns.clone(),
    )
}

fn pair_indexes<'a>(src: &'a [Idx], dst: &'a [Idx]) -> Vec<(Option<&'a Idx>, Option<&'a Idx>)> {
    let mut used = vec![false; dst.len()];
    let mut out = Vec::new();
    for s in src {
        let hit = dst
            .iter()
            .enumerate()
            .find(|(i, d)| !used[*i] && d.name == s.name)
            .or_else(|| {
                dst.iter()
                    .enumerate()
                    .find(|(i, d)| !used[*i] && d.columns == s.columns && d.unique == s.unique)
            });
        match hit {
            Some((i, d)) => {
                used[i] = true;
                out.push((Some(s), Some(d)));
            }
            None => out.push((Some(s), None)),
        }
    }
    for (i, d) in dst.iter().enumerate() {
        if !used[i] {
            out.push((None, Some(d)));
        }
    }
    out
}

fn item(
    kind: ObjectKind,
    table: Option<&str>,
    name: &str,
    state: DiffState,
    source: Option<String>,
    target: Option<String>,
) -> DiffItem {
    DiffItem {
        kind,
        table: table.map(str::to_owned),
        name: name.to_owned(),
        state,
        source,
        target,
        notes: Vec::new(),
    }
}

fn state_of(a: bool, b: bool, same: bool) -> DiffState {
    match (a, b) {
        (true, false) => DiffState::Added,
        (false, true) => DiffState::Removed,
        _ if same => DiffState::Same,
        _ => DiffState::Changed,
    }
}

fn diff_columns(s: &Table, t: &Table, dialect: Dialect, out: &mut Vec<DiffItem>) {
    let tn = Some(s.name.as_str());
    for c in &s.columns {
        match t.columns.iter().find(|x| x.name == c.name) {
            None => {
                let mut it = item(
                    ObjectKind::Column,
                    tn,
                    &c.name,
                    DiffState::Added,
                    Some(describe_col(c)),
                    None,
                );
                if dialect == Dialect::Sqlite
                    && (c.generated || (!c.nullable && c.default.is_none()))
                {
                    it.notes.push(Note::Rebuild);
                }
                out.push(it);
            }
            Some(x) if !same_col(c, x) => {
                let mut it = item(
                    ObjectKind::Column,
                    tn,
                    &c.name,
                    DiffState::Changed,
                    Some(describe_col(c)),
                    Some(describe_col(x)),
                );
                if dialect == Dialect::Sqlite
                    || (dialect == Dialect::Mssql && c.generated != x.generated)
                {
                    it.notes.push(Note::Rebuild);
                }
                out.push(it);
            }
            Some(_) => {}
        }
    }
    for x in t
        .columns
        .iter()
        .filter(|x| !s.columns.iter().any(|c| c.name == x.name))
    {
        let mut it = item(
            ObjectKind::Column,
            tn,
            &x.name,
            DiffState::Removed,
            None,
            Some(describe_col(x)),
        );
        it.notes.push(Note::Destructive);
        out.push(it);
    }
}

fn diff_keys(s: &Table, t: &Table, dialect: Dialect, out: &mut Vec<DiffItem>) {
    let tn = Some(s.name.as_str());
    if s.primary_key != t.primary_key {
        let show = |k: &[String]| (!k.is_empty()).then(|| format!("({})", k.join(", ")));
        let state = state_of(!s.primary_key.is_empty(), !t.primary_key.is_empty(), false);
        let mut it = item(
            ObjectKind::PrimaryKey,
            tn,
            "PRIMARY KEY",
            state,
            show(&s.primary_key),
            show(&t.primary_key),
        );
        if !in_place_keys(dialect) {
            it.notes.push(Note::Rebuild);
        }
        out.push(it);
    }
    for (a, b) in pair_indexes(&s.indexes, &t.indexes) {
        if matches!((a, b), (Some(a), Some(b)) if a.columns == b.columns && a.unique == b.unique) {
            continue;
        }
        let name = a.or(b).map(|i| i.name.as_str()).unwrap_or_default();
        let state = state_of(a.is_some(), b.is_some(), false);
        out.push(item(
            ObjectKind::Index,
            tn,
            name,
            state,
            a.map(describe_idx),
            b.map(describe_idx),
        ));
    }
}

fn diff_foreign_keys(s: &Table, t: &Table, dialect: Dialect, out: &mut Vec<DiffItem>) {
    let tn = Some(s.name.as_str());
    let actions = |f: &Fk| {
        format!(
            "{} ON DELETE {} ON UPDATE {}",
            describe_fk(f),
            action_sql(f.on_delete),
            action_sql(f.on_update)
        )
    };
    let start = out.len();
    for f in &s.foreign_keys {
        match t.foreign_keys.iter().find(|x| fk_key(x) == fk_key(f)) {
            None => out.push(item(
                ObjectKind::ForeignKey,
                tn,
                &f.name,
                DiffState::Added,
                Some(describe_fk(f)),
                None,
            )),
            Some(x)
                if !same_action(x.on_delete, f.on_delete)
                    || !same_action(x.on_update, f.on_update) =>
            {
                out.push(item(
                    ObjectKind::ForeignKey,
                    tn,
                    &f.name,
                    DiffState::Changed,
                    Some(actions(f)),
                    Some(actions(x)),
                ));
            }
            Some(_) => {}
        }
    }
    for x in t
        .foreign_keys
        .iter()
        .filter(|x| !s.foreign_keys.iter().any(|f| fk_key(f) == fk_key(x)))
    {
        out.push(item(
            ObjectKind::ForeignKey,
            tn,
            &x.name,
            DiffState::Removed,
            None,
            Some(describe_fk(x)),
        ));
    }
    if !in_place_keys(dialect) {
        out[start..]
            .iter_mut()
            .for_each(|it| it.notes.push(Note::Rebuild));
    }
}

fn diff_tables(source: &Snapshot, target: &Snapshot, dialect: Dialect, out: &mut Vec<DiffItem>) {
    let count = |x: &Table| Some(format!("{} col.", x.columns.len()));
    for s in &source.tables {
        let Some(t) = target.tables.iter().find(|t| t.name == s.name) else {
            out.push(item(
                ObjectKind::Table,
                None,
                &s.name,
                DiffState::Added,
                count(s),
                None,
            ));
            continue;
        };
        let at = out.len();
        diff_columns(s, t, dialect, out);
        diff_keys(s, t, dialect, out);
        diff_foreign_keys(s, t, dialect, out);
        let state = if out.len() == at {
            DiffState::Same
        } else {
            DiffState::Changed
        };
        out.insert(
            at,
            item(ObjectKind::Table, None, &s.name, state, count(s), count(t)),
        );
    }
    for t in target
        .tables
        .iter()
        .filter(|t| !source.tables.iter().any(|s| s.name == t.name))
    {
        let mut it = item(
            ObjectKind::Table,
            None,
            &t.name,
            DiffState::Removed,
            None,
            count(t),
        );
        it.notes.push(Note::Destructive);
        out.push(it);
    }
}

fn defined(mut it: DiffItem, definition: Option<&str>) -> DiffItem {
    if definition.is_none() && it.state != DiffState::Same && it.state != DiffState::Removed {
        it.notes.push(Note::NoDefinition);
    }
    it
}

fn diff_views(source: &Snapshot, target: &Snapshot, out: &mut Vec<DiffItem>) {
    for s in &source.views {
        let t = target.views.iter().find(|t| t.name == s.name);
        let same = t.is_some_and(|t| {
            t.definition.as_deref().map(squash) == s.definition.as_deref().map(squash)
        });
        let state = state_of(true, t.is_some(), same);
        let it = item(
            ObjectKind::View,
            None,
            &s.name,
            state,
            s.definition.clone(),
            t.and_then(|t| t.definition.clone()),
        );
        out.push(defined(it, s.definition.as_deref()));
    }
    for t in target
        .views
        .iter()
        .filter(|t| !source.views.iter().any(|s| s.name == t.name))
    {
        out.push(item(
            ObjectKind::View,
            None,
            &t.name,
            DiffState::Removed,
            None,
            t.definition.clone(),
        ));
    }
}

fn diff_routines(source: &Snapshot, target: &Snapshot, out: &mut Vec<DiffItem>) {
    let key = |r: &Routine| (r.name.clone(), squash(&r.signature));
    let label = |r: &Routine| format!("{}({})", r.name, r.signature);
    for s in &source.routines {
        let t = target.routines.iter().find(|t| key(t) == key(s));
        let same = t.is_some_and(|t| {
            t.definition.as_deref().map(squash) == s.definition.as_deref().map(squash)
        });
        let state = state_of(true, t.is_some(), same);
        let it = item(
            ObjectKind::Function,
            None,
            &label(s),
            state,
            s.definition.clone(),
            t.and_then(|t| t.definition.clone()),
        );
        out.push(defined(it, s.definition.as_deref()));
    }
    for t in target
        .routines
        .iter()
        .filter(|t| !source.routines.iter().any(|s| key(s) == key(t)))
    {
        out.push(item(
            ObjectKind::Function,
            None,
            &label(t),
            DiffState::Removed,
            None,
            t.definition.clone(),
        ));
    }
}

pub fn diff(source: &Snapshot, target: &Snapshot, dialect: Dialect) -> Vec<DiffItem> {
    let mut out = Vec::new();
    diff_tables(source, target, dialect, &mut out);
    diff_views(source, target, &mut out);
    diff_routines(source, target, &mut out);
    out
}

fn name_len(s: &str) -> usize {
    let b = s.as_bytes();
    let mut i = 0;
    loop {
        let close = match b.get(i) {
            Some(b'[') => Some(b']'),
            Some(b'"') => Some(b'"'),
            _ => None,
        };
        if let Some(close) = close {
            i += 1;
            while i < b.len() {
                if b[i] == close {
                    if b.get(i + 1) == Some(&close) {
                        i += 1;
                    } else {
                        break;
                    }
                }
                i += 1;
            }
            i += 1;
        } else {
            while i < b.len() && !b[i].is_ascii_whitespace() && !matches!(b[i], b'.' | b'(' | b';')
            {
                i += 1;
            }
        }
        if b.get(i) == Some(&b'.') {
            i += 1;
        } else {
            return i.min(b.len());
        }
    }
}

pub fn restate(def: &str, name: &str, dialect: Dialect, namespace: Option<&str>) -> Option<String> {
    let mut rest = def.trim_start();
    let mut word = || {
        let end = rest.find(|c: char| c.is_whitespace()).unwrap_or(rest.len());
        let w = &rest[..end];
        rest = rest[end..].trim_start();
        w.to_ascii_uppercase()
    };
    if word() != "CREATE" {
        return None;
    }
    let kind = loop {
        match word().as_str() {
            "OR" | "ALTER" | "REPLACE" | "TEMP" | "TEMPORARY" => {}
            "VIEW" => break "VIEW",
            "PROCEDURE" | "PROC" => break "PROCEDURE",
            "FUNCTION" => break "FUNCTION",
            "MACRO" => break "MACRO",
            _ => return None,
        }
    };
    let verb = if dialect == Dialect::Mssql {
        "CREATE OR ALTER"
    } else {
        "CREATE OR REPLACE"
    };
    let tail = &rest[name_len(rest)..];
    let q = |n: &str| nd_sql::ident(dialect, n);
    let full = match namespace.filter(|n| !n.is_empty()) {
        Some(ns) => format!("{}.{}", q(ns), q(name)),
        None => q(name),
    };
    Some(format!("{verb} {kind} {full}{tail}"))
}
