use std::collections::HashMap;
use std::fmt::Write;

use nd_driver::{Dialect, FkAction};
use nd_sql::ident;
use serde::{Deserialize, Serialize};

use crate::{action_sql, same_type, squash};

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, specta::Type)]
pub struct TableDesign {
    pub namespace: String,
    pub name: String,
    pub comment: Option<String>,
    pub columns: Vec<DesignColumn>,
    pub primary_key: Vec<String>,
    pub pk_name: Option<String>,
    pub indexes: Vec<DesignIndex>,
    pub foreign_keys: Vec<DesignForeignKey>,
    pub checks: Vec<DesignCheck>,
    #[serde(default)]
    pub triggers: Vec<DesignTrigger>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, specta::Type)]
pub struct DesignTrigger {
    pub name: String,
    pub sql: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, specta::Type)]
pub struct DesignColumn {
    pub origin: Option<String>,
    pub name: String,
    pub type_name: String,
    pub nullable: bool,
    pub default: Option<String>,
    pub identity: bool,
    pub comment: Option<String>,
    pub default_name: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, specta::Type)]
pub struct DesignIndex {
    pub origin: Option<String>,
    pub name: String,
    pub columns: Vec<String>,
    pub unique: bool,
    #[serde(default)]
    pub descending: Vec<bool>,
    #[serde(default)]
    pub expressions: Vec<bool>,
    #[serde(default)]
    pub predicate: Option<String>,
}

impl DesignIndex {
    pub fn is_desc(&self, k: usize) -> bool {
        self.descending.get(k).copied().unwrap_or(false)
    }

    pub fn is_expr(&self, k: usize) -> bool {
        self.expressions.get(k).copied().unwrap_or(false)
    }

    fn parts_same(&self, other: &Self) -> bool {
        (0..self.columns.len().max(other.columns.len()))
            .all(|k| self.is_desc(k) == other.is_desc(k) && self.is_expr(k) == other.is_expr(k))
            && same_opt(
                self.predicate.as_deref().map(strip_parens),
                other.predicate.as_deref().map(strip_parens),
            )
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
pub struct DesignForeignKey {
    pub origin: Option<String>,
    pub name: String,
    pub columns: Vec<String>,
    pub ref_namespace: String,
    pub ref_table: String,
    pub ref_columns: Vec<String>,
    pub on_delete: FkAction,
    pub on_update: FkAction,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, specta::Type)]
pub struct DesignCheck {
    pub origin: Option<String>,
    pub name: String,
    pub expression: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "snake_case")]
pub enum DesignEngine {
    Postgres,
    Cockroach,
    Mysql,
    Mssql,
    Sqlite,
    Duckdb,
    Firebird,
}

impl DesignEngine {
    pub fn dialect(self) -> Dialect {
        match self {
            Self::Postgres | Self::Cockroach => Dialect::Postgres,
            Self::Mysql => Dialect::Mysql,
            Self::Mssql => Dialect::Mssql,
            Self::Sqlite => Dialect::Sqlite,
            Self::Duckdb => Dialect::Duckdb,
            Self::Firebird => Dialect::Firebird,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "snake_case")]
pub enum DesignNoteCode {
    DropsData,
    Rebuild,
    ReorderRewrites,
    ReorderUnsupported,
    DefaultConstraint,
    IdentityUnsupported,
    CommentUnsupported,
    NotNullNeedsValue,
    TypeChange,
    PrimaryKeyRequired,
    FkActionUnsupported,
    Invalid,
    TriggerRecreated,
    TriggerBlocks,
    PartialUnsupported,
    RenameViaTemp,
    RenameTableUnsupported,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
pub struct DesignNote {
    pub code: DesignNoteCode,
    pub subject: Option<String>,
    pub detail: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, specta::Type)]
pub struct DesignScript {
    pub statements: Vec<String>,
    pub notes: Vec<DesignNote>,
    pub rebuild: bool,
}

struct W<'a> {
    e: DesignEngine,
    d: Dialect,
    ns: &'a str,
    out: Vec<String>,
    notes: Vec<DesignNote>,
}

fn same_opt(a: Option<&str>, b: Option<&str>) -> bool {
    let norm = |x: Option<&str>| x.map(str::trim).filter(|x| !x.is_empty()).map(squash);
    norm(a) == norm(b)
}

fn text(a: Option<&str>) -> Option<&str> {
    a.map(str::trim).filter(|x| !x.is_empty())
}

fn strip_parens(s: &str) -> &str {
    let t = s.trim();
    if t.starts_with('(') && t.ends_with(')') {
        let inner = &t[1..t.len() - 1];
        let mut depth = 0i32;
        for c in inner.chars() {
            match c {
                '(' => depth += 1,
                ')' => {
                    depth -= 1;
                    if depth < 0 {
                        return t;
                    }
                }
                _ => {}
            }
        }
        if depth == 0 {
            return strip_parens(inner);
        }
    }
    t
}

pub fn check_body(def: &str) -> String {
    let t = def.trim();
    let t = if t.len() >= 5 && t[..5].eq_ignore_ascii_case("CHECK") {
        &t[5..]
    } else {
        t
    };
    let t = t.trim().trim_end_matches("NOT VALID").trim();
    strip_parens(t).to_owned()
}

impl W<'_> {
    fn q(&self, name: &str) -> String {
        ident(self.d, name)
    }

    fn t(&self, name: &str) -> String {
        if matches!(self.d, Dialect::Sqlite | Dialect::Firebird) || self.ns.is_empty() {
            self.q(name)
        } else {
            format!("{}.{}", self.q(self.ns), self.q(name))
        }
    }

    fn reft(&self, f: &DesignForeignKey) -> String {
        let ns = if f.ref_namespace.is_empty() {
            self.ns
        } else {
            f.ref_namespace.as_str()
        };
        if matches!(self.d, Dialect::Sqlite | Dialect::Firebird) || ns.is_empty() {
            self.q(&f.ref_table)
        } else {
            format!("{}.{}", self.q(ns), self.q(&f.ref_table))
        }
    }

    fn list(&self, names: &[String]) -> String {
        names
            .iter()
            .map(|n| self.q(n))
            .collect::<Vec<_>>()
            .join(", ")
    }

    fn lit(&self, s: &str) -> String {
        let l = nd_sql::literal(self.d, Some(s));
        if self.d == Dialect::Mssql {
            format!("N{l}")
        } else {
            l
        }
    }

    fn note(&mut self, code: DesignNoteCode, subject: Option<&str>) {
        let n = DesignNote {
            code,
            subject: subject.map(str::to_owned),
            detail: None,
        };
        if !self.notes.contains(&n) {
            self.notes.push(n);
        }
    }

    fn alter(&mut self, table: &str, what: &str) {
        let sql = format!("ALTER TABLE {} {what}", self.t(table));
        self.out.push(sql);
    }

    fn default_name(table: &str, c: &DesignColumn) -> String {
        format!("DF_{table}_{}", c.name)
    }

    fn col_def(&self, table: &str, c: &DesignColumn, inline_pk: bool) -> String {
        let mut s = format!("{} {}", self.q(&c.name), c.type_name.trim());
        if inline_pk {
            s.push_str(" PRIMARY KEY AUTOINCREMENT");
        }
        if c.identity {
            match self.e {
                DesignEngine::Postgres | DesignEngine::Cockroach | DesignEngine::Firebird => {
                    s.push_str(" GENERATED BY DEFAULT AS IDENTITY");
                }
                DesignEngine::Mssql => s.push_str(" IDENTITY(1,1)"),
                _ => {}
            }
        }
        if self.e == DesignEngine::Firebird
            && !c.identity
            && let Some(d) = text(c.default.as_deref())
        {
            s.push_str(" DEFAULT ");
            s.push_str(d);
        }
        if !c.nullable {
            s.push_str(" NOT NULL");
        } else if self.e == DesignEngine::Mssql {
            s.push_str(" NULL");
        }
        if c.identity && self.e == DesignEngine::Mysql {
            s.push_str(" AUTO_INCREMENT");
        }
        if let Some(d) = text(c.default.as_deref()).filter(|_| {
            (!c.identity || self.e == DesignEngine::Duckdb) && self.e != DesignEngine::Firebird
        }) {
            if self.e == DesignEngine::Mssql {
                let name = c
                    .default_name
                    .clone()
                    .unwrap_or_else(|| Self::default_name(table, c));
                let _ = write!(s, " CONSTRAINT {} DEFAULT {d}", self.q(&name));
            } else {
                s.push_str(" DEFAULT ");
                s.push_str(d);
            }
        }
        if self.e == DesignEngine::Mysql
            && let Some(cm) = text(c.comment.as_deref())
        {
            s.push_str(" COMMENT ");
            s.push_str(&self.lit(cm));
        }
        s
    }

    fn fk_clause(&self, f: &DesignForeignKey) -> String {
        let mut s = String::new();
        if !f.name.trim().is_empty() {
            let _ = write!(s, "CONSTRAINT {} ", self.q(&f.name));
        }
        let _ = write!(
            s,
            "FOREIGN KEY ({}) REFERENCES {} ({})",
            self.list(&f.columns),
            self.reft(f),
            self.list(&f.ref_columns)
        );
        for (word, a) in [("DELETE", f.on_delete), ("UPDATE", f.on_update)] {
            let plain = matches!(a, FkAction::NoAction | FkAction::Restrict);
            if a != FkAction::NoAction && (self.e != DesignEngine::Duckdb || plain) {
                let _ = write!(s, " ON {word} {}", action_sql(a));
            }
        }
        s
    }

    fn check_clause(&self, c: &DesignCheck) -> String {
        let body = format!("CHECK ({})", c.expression.trim());
        if c.name.trim().is_empty() {
            body
        } else {
            format!("CONSTRAINT {} {body}", self.q(&c.name))
        }
    }

    fn pk_clause(&self, t: &TableDesign) -> String {
        let body = format!("PRIMARY KEY ({})", self.list(&t.primary_key));
        match text(t.pk_name.as_deref()) {
            Some(n) if self.e != DesignEngine::Sqlite && self.e != DesignEngine::Mysql => {
                format!("CONSTRAINT {} {body}", self.q(n))
            }
            _ => body,
        }
    }

    fn sqlite_inline_pk(&self, t: &TableDesign, c: &DesignColumn) -> bool {
        self.e == DesignEngine::Sqlite && c.identity && t.primary_key == [c.name.clone()]
    }

    fn create_table(&self, t: &TableDesign, as_name: &str) -> String {
        let mut parts: Vec<String> = t
            .columns
            .iter()
            .map(|c| self.col_def(&t.name, c, self.sqlite_inline_pk(t, c)))
            .collect();
        let inline = t.columns.iter().any(|c| self.sqlite_inline_pk(t, c));
        if !t.primary_key.is_empty() && !inline {
            parts.push(self.pk_clause(t));
        }
        parts.extend(t.checks.iter().map(|c| self.check_clause(c)));
        if self.e != DesignEngine::Duckdb || !t.foreign_keys.iter().any(|f| f.ref_table == t.name) {
            parts.extend(t.foreign_keys.iter().map(|f| self.fk_clause(f)));
        }
        let mut s = format!(
            "CREATE TABLE {} (\n  {}\n)",
            self.t(as_name),
            parts.join(",\n  ")
        );
        if self.e == DesignEngine::Mysql
            && let Some(cm) = text(t.comment.as_deref())
        {
            let _ = write!(s, " COMMENT = {}", self.lit(cm));
        }
        s
    }

    fn create_index(&mut self, table: &str, i: &DesignIndex) {
        if self.e == DesignEngine::Firebird {
            self.firebird_index(table, i);
            return;
        }
        let unique = if i.unique { "UNIQUE " } else { "" };
        let parts = i
            .columns
            .iter()
            .enumerate()
            .map(|(k, c)| {
                let part = if i.is_expr(k) {
                    format!("({})", strip_parens(c))
                } else {
                    self.q(c)
                };
                if i.is_desc(k) {
                    format!("{part} DESC")
                } else {
                    part
                }
            })
            .collect::<Vec<_>>()
            .join(", ");
        let mut sql = format!(
            "CREATE {unique}INDEX {} ON {} ({parts})",
            self.q(&i.name),
            self.t(table),
        );
        if let Some(p) = text(i.predicate.as_deref()) {
            if matches!(self.e, DesignEngine::Mysql | DesignEngine::Duckdb) {
                self.note(DesignNoteCode::PartialUnsupported, Some(&i.name));
            } else {
                let _ = write!(sql, " WHERE {}", strip_parens(p));
            }
        }
        self.out.push(sql);
    }

    fn firebird_index(&mut self, table: &str, i: &DesignIndex) {
        let unique = if i.unique { "UNIQUE " } else { "" };
        let all_desc = !i.columns.is_empty() && (0..i.columns.len()).all(|k| i.is_desc(k));
        let desc = if all_desc { "DESCENDING " } else { "" };
        let body = match i.columns.as_slice() {
            [one] if i.is_expr(0) => format!("COMPUTED BY ({})", strip_parens(one)),
            _ => format!("({})", self.list(&i.columns)),
        };
        let mut sql = format!(
            "CREATE {unique}{desc}INDEX {} ON {} {body}",
            self.q(&i.name),
            self.t(table)
        );
        if let Some(p) = text(i.predicate.as_deref()) {
            let _ = write!(sql, " WHERE {}", strip_parens(p));
        }
        self.out.push(sql);
    }

    fn drop_index(&mut self, table: &str, name: &str) {
        let sql = match self.e {
            DesignEngine::Mysql | DesignEngine::Mssql => {
                format!("DROP INDEX {} ON {}", self.q(name), self.t(table))
            }
            DesignEngine::Cockroach => {
                format!("DROP INDEX {}@{} CASCADE", self.t(table), self.q(name))
            }
            _ => format!("DROP INDEX {}", self.t(name)),
        };
        self.out.push(sql);
    }

    fn mssql_property(
        &mut self,
        verb: &str,
        table: &str,
        column: Option<&str>,
        value: Option<&str>,
    ) {
        let mut sql = format!("EXEC sys.sp_{verb}extendedproperty @name = N'MS_Description', ");
        if let Some(v) = value {
            let _ = write!(sql, "@value = {}, ", self.lit(v));
        }
        let _ = write!(
            sql,
            "@level0type = N'SCHEMA', @level0name = {}, @level1type = N'TABLE', @level1name = {}",
            self.lit(self.ns),
            self.lit(table)
        );
        if let Some(c) = column {
            let _ = write!(
                sql,
                ", @level2type = N'COLUMN', @level2name = {}",
                self.lit(c)
            );
        }
        self.out.push(sql);
    }

    fn comment(
        &mut self,
        table: &str,
        column: Option<&str>,
        before: Option<&str>,
        after: Option<&str>,
    ) {
        let (before, after) = (text(before), text(after));
        if same_opt(before, after) {
            return;
        }
        match self.e {
            DesignEngine::Sqlite => self.note(DesignNoteCode::CommentUnsupported, None),
            DesignEngine::Mysql => {
                if column.is_none() {
                    let v = self.lit(after.unwrap_or(""));
                    self.alter(table, &format!("COMMENT = {v}"));
                }
            }
            DesignEngine::Mssql => match (before, after) {
                (None, Some(v)) => self.mssql_property("add", table, column, Some(v)),
                (Some(_), Some(v)) => self.mssql_property("update", table, column, Some(v)),
                (Some(_), None) => self.mssql_property("drop", table, column, None),
                (None, None) => {}
            },
            DesignEngine::Postgres
            | DesignEngine::Cockroach
            | DesignEngine::Duckdb
            | DesignEngine::Firebird => {
                let what = match column {
                    Some(c) => format!("COLUMN {}.{}", self.t(table), self.q(c)),
                    None => format!("TABLE {}", self.t(table)),
                };
                let v = after.map_or_else(|| "NULL".to_owned(), |v| self.lit(v));
                self.out.push(format!("COMMENT ON {what} IS {v}"));
            }
        }
    }
}

fn problems(t: &TableDesign) -> Vec<DesignNote> {
    let mut out = Vec::new();
    let mut bad = |subject: &str| {
        out.push(DesignNote {
            code: DesignNoteCode::Invalid,
            subject: Some(subject.to_owned()),
            detail: None,
        });
    };
    if t.name.trim().is_empty() {
        bad("table");
    }
    if t.columns.is_empty() {
        bad("columns");
    }
    let mut seen = std::collections::HashSet::new();
    for c in &t.columns {
        if c.name.trim().is_empty() {
            bad("column_name");
        } else if !seen.insert(c.name.to_lowercase()) {
            bad(&c.name);
        }
        if c.type_name.trim().is_empty() {
            bad(if c.name.is_empty() {
                "column_type"
            } else {
                &c.name
            });
        }
    }
    let known = |n: &String| t.columns.iter().any(|c| &c.name == n);
    if !t.primary_key.iter().all(known) {
        bad("primary_key");
    }
    for i in &t.indexes {
        let parts_ok = i.columns.iter().enumerate().all(|(k, c)| {
            if i.is_expr(k) {
                !c.trim().is_empty()
            } else {
                known(c)
            }
        });
        if i.name.trim().is_empty() || i.columns.is_empty() || !parts_ok {
            bad(if i.name.is_empty() { "index" } else { &i.name });
        }
    }
    for f in &t.foreign_keys {
        if f.ref_table.trim().is_empty()
            || f.columns.is_empty()
            || f.columns.len() != f.ref_columns.len()
            || !f.columns.iter().all(known)
            || f.ref_columns.iter().any(|c| c.trim().is_empty())
        {
            bad(if f.name.is_empty() {
                "foreign_key"
            } else {
                &f.name
            });
        }
    }
    for c in &t.checks {
        if c.expression.trim().is_empty() {
            bad(if c.name.is_empty() { "check" } else { &c.name });
        }
    }
    out.dedup();
    out
}

fn create(w: &mut W, t: &TableDesign) {
    if w.e == DesignEngine::Sqlite
        && (text(t.comment.as_deref()).is_some()
            || t.columns
                .iter()
                .any(|c| text(c.comment.as_deref()).is_some()))
    {
        w.note(DesignNoteCode::CommentUnsupported, None);
    }
    identity_notes(w, t);
    let sql = w.create_table(t, &t.name);
    w.out.push(sql);
    for i in &t.indexes {
        w.create_index(&t.name, i);
    }
    if w.e == DesignEngine::Duckdb {
        for f in t.foreign_keys.iter().filter(|f| f.ref_table == t.name) {
            let sql = w.fk_clause(f);
            w.alter(&t.name, &format!("ADD {sql}"));
        }
    }
    if w.e != DesignEngine::Mysql && w.e != DesignEngine::Sqlite {
        w.comment(&t.name, None, None, t.comment.as_deref());
        for c in &t.columns {
            w.comment(&t.name, Some(&c.name), None, c.comment.as_deref());
        }
    }
}

fn identity_notes(w: &mut W, t: &TableDesign) {
    for c in t.columns.iter().filter(|c| c.identity) {
        let ok = match w.e {
            DesignEngine::Duckdb => false,
            DesignEngine::Sqlite => w.sqlite_inline_pk(t, c),
            _ => true,
        };
        if !ok {
            w.note(DesignNoteCode::IdentityUnsupported, Some(&c.name));
        }
    }
}

struct Pairs<'a> {
    before: &'a TableDesign,
    after: &'a TableDesign,
    renamed: HashMap<&'a str, &'a str>,
}

impl<'a> Pairs<'a> {
    fn new(before: &'a TableDesign, after: &'a TableDesign) -> Self {
        let renamed = after
            .columns
            .iter()
            .filter_map(|c| {
                let o = c.origin.as_deref()?;
                before
                    .columns
                    .iter()
                    .any(|b| b.name == o)
                    .then_some((o, c.name.as_str()))
            })
            .collect();
        Self {
            before,
            after,
            renamed,
        }
    }

    fn map(&self, cols: &[String]) -> Vec<String> {
        cols.iter()
            .map(|c| (*self.renamed.get(c.as_str()).unwrap_or(&"\u{0}dropped")).to_owned())
            .collect()
    }

    fn column(&self, c: &DesignColumn) -> Option<&'a DesignColumn> {
        let o = c.origin.as_deref()?;
        self.before.columns.iter().find(|b| b.name == o)
    }

    fn dropped(&self) -> Vec<&'a DesignColumn> {
        self.before
            .columns
            .iter()
            .filter(|b| !self.renamed.contains_key(b.name.as_str()))
            .collect()
    }

    fn index(&self, i: &DesignIndex) -> Option<&'a DesignIndex> {
        let o = i.origin.as_deref()?;
        self.before.indexes.iter().find(|b| b.name == o)
    }

    fn rewrite(&self, text: &str) -> String {
        let mut out = String::with_capacity(text.len());
        let mut last = 0;
        for (from, to, name) in idents(text) {
            let Some(new) = self.renamed.get(name.as_str()).filter(|n| **n != name) else {
                continue;
            };
            out.push_str(&text[last..from]);
            match text[from..].chars().next() {
                Some('[') => {
                    let _ = write!(out, "[{}]", new.replace(']', "]]"));
                }
                Some(q @ ('"' | '`')) => {
                    let _ = write!(out, "{q}{}{q}", new.replace(q, &format!("{q}{q}")));
                }
                _ => out.push_str(new),
            }
            last = to;
        }
        out.push_str(&text[last..]);
        out
    }

    fn names_renamed(&self, i: &DesignIndex) -> bool {
        i.columns
            .iter()
            .enumerate()
            .filter(|(k, _)| i.is_expr(*k))
            .map(|(_, c)| c.as_str())
            .chain(i.predicate.as_deref())
            .any(|t| self.rewrite(t) != t)
    }

    fn blocks_rename(&self, e: DesignEngine, i: &DesignIndex) -> bool {
        matches!(e, DesignEngine::Mysql | DesignEngine::Mssql) && self.names_renamed(i)
    }

    fn index_same(&self, b: &DesignIndex, a: &DesignIndex) -> bool {
        let cols = b.columns.iter().enumerate().map(|(k, c)| {
            if b.is_expr(k) {
                squash(&self.rewrite(c))
            } else {
                self.map(std::slice::from_ref(c)).swap_remove(0)
            }
        });
        let theirs = a
            .columns
            .iter()
            .enumerate()
            .map(|(k, c)| if a.is_expr(k) { squash(c) } else { c.clone() });
        let moved = DesignIndex {
            predicate: b.predicate.as_deref().map(|x| self.rewrite(x)),
            ..b.clone()
        };
        b.name == a.name && b.unique == a.unique && cols.eq(theirs) && moved.parts_same(a)
    }

    fn fk(&self, f: &DesignForeignKey) -> Option<&'a DesignForeignKey> {
        let o = f.origin.as_deref()?;
        self.before.foreign_keys.iter().find(|b| b.name == o)
    }

    fn fk_same(&self, b: &DesignForeignKey, a: &DesignForeignKey) -> bool {
        let ns = |x: &DesignForeignKey| {
            if x.ref_namespace.is_empty() {
                self.before.namespace.clone()
            } else {
                x.ref_namespace.clone()
            }
        };
        let rt = if b.ref_table == self.before.name {
            self.after.name.clone()
        } else {
            b.ref_table.clone()
        };
        let rc = if b.ref_table == self.before.name {
            self.map(&b.ref_columns)
        } else {
            b.ref_columns.clone()
        };
        b.name == a.name
            && self.map(&b.columns) == a.columns
            && ns(b) == ns(a)
            && rt == a.ref_table
            && rc == a.ref_columns
            && crate::same_action(b.on_delete, a.on_delete)
            && crate::same_action(b.on_update, a.on_update)
    }

    fn check(&self, c: &DesignCheck) -> Option<&'a DesignCheck> {
        let o = c.origin.as_deref()?;
        self.before.checks.iter().find(|b| b.name == o)
    }

    fn check_same(b: &DesignCheck, a: &DesignCheck) -> bool {
        b.name == a.name && squash(&b.expression) == squash(&a.expression)
    }

    fn pk_changed(&self) -> bool {
        self.map(&self.before.primary_key) != self.after.primary_key
    }

    fn natural_order(&self) -> Vec<String> {
        let mut out: Vec<String> = self
            .before
            .columns
            .iter()
            .filter_map(|b| self.renamed.get(b.name.as_str()).map(|n| (*n).to_owned()))
            .collect();
        out.extend(
            self.after
                .columns
                .iter()
                .filter(|c| self.column(c).is_none())
                .map(|c| c.name.clone()),
        );
        out
    }

    fn reordered(&self) -> bool {
        let want: Vec<String> = self.after.columns.iter().map(|c| c.name.clone()).collect();
        self.natural_order() != want
    }

    fn kept_order_changed(&self) -> bool {
        let kept: Vec<&str> = self
            .after
            .columns
            .iter()
            .filter(|c| self.column(c).is_some())
            .map(|c| c.name.as_str())
            .collect();
        let natural: Vec<&str> = self
            .before
            .columns
            .iter()
            .filter_map(|b| self.renamed.get(b.name.as_str()).copied())
            .collect();
        kept != natural
    }
}

fn col_changed(e: DesignEngine, b: &DesignColumn, a: &DesignColumn) -> (bool, bool, bool, bool) {
    let ty = !same_type(&b.type_name, &a.type_name);
    let null = b.nullable != a.nullable;
    let default = !same_opt(b.default.as_deref(), a.default.as_deref());
    let identity = b.identity != a.identity;
    let _ = e;
    (ty, null, default, identity)
}

fn needs_rebuild(e: DesignEngine, p: &Pairs) -> bool {
    if !matches!(e, DesignEngine::Sqlite | DesignEngine::Duckdb) {
        return false;
    }
    let (b, a) = (p.before, p.after);
    if p.pk_changed() || p.kept_order_changed() {
        return true;
    }
    let indexed = e == DesignEngine::Duckdb && !b.indexes.is_empty();
    let col_change = a.columns.iter().any(|c| match p.column(c) {
        None => indexed,
        Some(old) => {
            let (ty, null, default, identity) = col_changed(e, old, c);
            match e {
                DesignEngine::Sqlite => ty || null || default || identity,
                _ => indexed && (ty || null || default || old.name != c.name),
            }
        }
    }) || (indexed && !p.dropped().is_empty());
    let add_hard = a.columns.iter().any(|c| {
        p.column(c).is_none()
            && ((!c.nullable && text(c.default.as_deref()).is_none())
                || c.identity
                || a.primary_key.contains(&c.name))
    });
    let reorder_adds = p.reordered();
    let fk_change = a.foreign_keys.len() != b.foreign_keys.len()
        || a.foreign_keys
            .iter()
            .any(|f| p.fk(f).is_none_or(|x| !p.fk_same(x, f)));
    let check_change = a.checks.len() != b.checks.len()
        || a.checks
            .iter()
            .any(|c| p.check(c).is_none_or(|x| !Pairs::check_same(x, c)));
    let dropped_key = p.dropped().iter().any(|d| {
        b.foreign_keys.iter().any(|f| f.columns.contains(&d.name))
            || b.checks
                .iter()
                .any(|c| c.expression.contains(d.name.as_str()))
    });
    let auto_index = e == DesignEngine::Sqlite
        && b.indexes
            .iter()
            .filter(|i| i.name.starts_with("sqlite_autoindex"))
            .any(|i| {
                a.indexes
                    .iter()
                    .find(|x| x.origin.as_deref() == Some(&i.name))
                    .is_none_or(|x| !p.index_same(i, x))
            });
    col_change || add_hard || reorder_adds || fk_change || check_change || dropped_key || auto_index
}

fn rebuild(w: &mut W, p: &Pairs) {
    let (b, a) = (p.before, p.after);
    w.note(DesignNoteCode::Rebuild, Some(&a.name));
    identity_notes(w, a);
    for d in p.dropped() {
        w.note(DesignNoteCode::DropsData, Some(&d.name));
    }
    for c in a.columns.iter().filter(|c| p.column(c).is_none()) {
        if !c.nullable && text(c.default.as_deref()).is_none() && !c.identity {
            w.note(DesignNoteCode::NotNullNeedsValue, Some(&c.name));
        }
    }
    for c in &a.columns {
        if let Some(x) = p.column(c)
            && !same_type(&x.type_name, &c.type_name)
        {
            w.note(DesignNoteCode::TypeChange, Some(&c.name));
        }
    }
    let temp = format!("{}_nd_new", a.name);
    if w.e == DesignEngine::Sqlite {
        w.out.push("PRAGMA defer_foreign_keys = ON".to_owned());
    }
    let create = w.create_table(a, &temp);
    w.out.push(create);
    let kept: Vec<(&DesignColumn, &DesignColumn)> = a
        .columns
        .iter()
        .filter_map(|c| p.column(c).map(|x| (c, x)))
        .collect();
    if !kept.is_empty() {
        let to = kept
            .iter()
            .map(|(c, _)| w.q(&c.name))
            .collect::<Vec<_>>()
            .join(", ");
        let from = kept
            .iter()
            .map(|(_, x)| w.q(&x.name))
            .collect::<Vec<_>>()
            .join(", ");
        let sql = format!(
            "INSERT INTO {} ({to})\nSELECT {from} FROM {}",
            w.t(&temp),
            w.t(&b.name)
        );
        w.out.push(sql);
    }
    let sql = format!("DROP TABLE {}", w.t(&b.name));
    w.out.push(sql);
    let sql = format!("ALTER TABLE {} RENAME TO {}", w.t(&temp), w.q(&a.name));
    w.out.push(sql);
    for i in &a.indexes {
        if i.name.starts_with("sqlite_autoindex") {
            let named = DesignIndex {
                name: format!("{}_{}_key", a.name, i.columns.join("_")),
                ..i.clone()
            };
            w.create_index(&a.name, &named);
        } else {
            w.create_index(&a.name, i);
        }
    }
    for t in &b.triggers {
        w.note(DesignNoteCode::TriggerRecreated, Some(&t.name));
        let sql = retarget(&t.sql, &b.name, &a.name);
        w.out
            .push(sql.trim().trim_end_matches(';').trim_end().to_owned());
    }
    if w.e == DesignEngine::Duckdb {
        for f in a.foreign_keys.iter().filter(|f| f.ref_table == a.name) {
            let sql = w.fk_clause(f);
            w.alter(&a.name, &format!("ADD {sql}"));
        }
        w.comment(&a.name, None, None, a.comment.as_deref());
        for c in &a.columns {
            w.comment(&a.name, Some(&c.name), None, c.comment.as_deref());
        }
    } else {
        w.comment(&a.name, None, b.comment.as_deref(), a.comment.as_deref());
        for c in &a.columns {
            let before = p.column(c).and_then(|x| x.comment.as_deref());
            w.comment(&a.name, Some(&c.name), before, c.comment.as_deref());
        }
    }
}

fn idents(sql: &str) -> Vec<(usize, usize, String)> {
    let b = sql.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    let close = |from: usize, end: u8| -> usize {
        let mut j = from;
        while j < b.len() {
            if b[j] == end {
                if b.get(j + 1) == Some(&end) && end != b']' {
                    j += 2;
                    continue;
                }
                return j;
            }
            j += 1;
        }
        b.len()
    };
    while i < b.len() {
        let c = b[i];
        if c == b'-' && b.get(i + 1) == Some(&b'-') {
            while i < b.len() && b[i] != b'\n' {
                i += 1;
            }
        } else if c == b'/' && b.get(i + 1) == Some(&b'*') {
            i = sql[i + 2..].find("*/").map_or(b.len(), |p| i + 4 + p);
        } else if c == b'\'' {
            i = close(i + 1, b'\'') + 1;
        } else if matches!(c, b'"' | b'`' | b'[') {
            let end = close(i + 1, if c == b'[' { b']' } else { c });
            let inner = sql.get(i + 1..end).unwrap_or_default();
            let quote = char::from(c);
            out.push((
                i,
                (end + 1).min(b.len()),
                inner.replace(&format!("{quote}{quote}"), &quote.to_string()),
            ));
            i = end + 1;
        } else if c.is_ascii_alphabetic() || c == b'_' || c >= 0x80 {
            let start = i;
            while i < b.len()
                && (b[i].is_ascii_alphanumeric() || b[i] == b'_' || b[i] == b'$' || b[i] >= 0x80)
            {
                i += 1;
            }
            out.push((start, i, sql[start..i].to_owned()));
        } else {
            i += 1;
        }
    }
    out
}

fn retarget(sql: &str, old: &str, new: &str) -> String {
    if old == new {
        return sql.to_owned();
    }
    let toks = idents(sql);
    let on = toks
        .iter()
        .take_while(|t| !t.2.eq_ignore_ascii_case("BEGIN"))
        .position(|t| t.2.eq_ignore_ascii_case("ON"));
    let Some(table) = on
        .and_then(|i| toks.get(i + 1))
        .filter(|t| t.2.eq_ignore_ascii_case(old))
    else {
        return sql.to_owned();
    };
    format!(
        "{}{}{}",
        &sql[..table.0],
        ident(Dialect::Sqlite, new),
        &sql[table.1..]
    )
}

fn trigger_blocks(p: &Pairs) -> Vec<DesignNote> {
    let (b, a) = (p.before, p.after);
    let mut gone: Vec<&str> = p.dropped().iter().map(|c| c.name.as_str()).collect();
    gone.extend(p.renamed.iter().filter(|(o, n)| o != n).map(|(o, _)| *o));
    let mut out = Vec::new();
    for t in &b.triggers {
        let toks = idents(&t.sql);
        let body = toks
            .iter()
            .position(|x| x.2.eq_ignore_ascii_case("BEGIN"))
            .unwrap_or(toks.len());
        let column = gone
            .iter()
            .find(|g| toks.iter().any(|x| x.2.eq_ignore_ascii_case(g)));
        let table = (b.name != a.name
            && toks[body.min(toks.len())..]
                .iter()
                .any(|x| x.2.eq_ignore_ascii_case(&b.name)))
        .then_some(b.name.as_str());
        if let Some(what) = column.copied().or(table) {
            out.push(DesignNote {
                code: DesignNoteCode::TriggerBlocks,
                subject: Some(t.name.clone()),
                detail: Some(what.to_owned()),
            });
        }
    }
    out
}

fn drop_constraint(w: &mut W, table: &str, name: &str, fk: bool) {
    let sql = match (w.e, fk) {
        (DesignEngine::Mysql, true) => format!("DROP FOREIGN KEY {}", w.q(name)),
        _ => format!("DROP CONSTRAINT {}", w.q(name)),
    };
    w.alter(table, &sql);
}

fn drops(w: &mut W, p: &Pairs, touched: &[String]) {
    let (b, a) = (p.before, p.after);
    for f in &b.foreign_keys {
        let keep = a
            .foreign_keys
            .iter()
            .find(|x| x.origin.as_deref() == Some(&f.name))
            .is_some_and(|x| p.fk_same(f, x));
        if !keep {
            drop_constraint(w, &b.name, &f.name, true);
        }
    }
    for c in &b.checks {
        let keep = a
            .checks
            .iter()
            .find(|x| x.origin.as_deref() == Some(&c.name))
            .is_some_and(|x| Pairs::check_same(c, x));
        if !keep {
            drop_constraint(w, &b.name, &c.name, false);
        }
    }
    for i in &b.indexes {
        let keep = a
            .indexes
            .iter()
            .find(|x| x.origin.as_deref() == Some(&i.name))
            .is_some_and(|x| p.index_same(i, x))
            && !i.columns.iter().any(|c| touched.contains(c))
            && !p.blocks_rename(w.e, i);
        if !keep {
            w.drop_index(&b.name, &i.name);
        }
    }
    let pk_touched = b.primary_key.iter().any(|c| touched.contains(c));
    if !b.primary_key.is_empty() && (p.pk_changed() || pk_touched) && w.e != DesignEngine::Cockroach
    {
        if w.e == DesignEngine::Mysql {
            w.alter(&b.name, "DROP PRIMARY KEY");
        } else {
            let name = b
                .pk_name
                .clone()
                .unwrap_or_else(|| format!("{}_pkey", b.name));
            let sql = format!("DROP CONSTRAINT {}", w.q(&name));
            w.alter(&b.name, &sql);
        }
    }
}

fn renames(w: &mut W, p: &Pairs) {
    let (b, a) = (p.before, p.after);
    if a.name != b.name {
        let sql = match w.e {
            DesignEngine::Mssql => format!(
                "EXEC sp_rename {}, {}",
                w.lit(&format!("{}.{}", w.ns, b.name)),
                w.lit(&a.name)
            ),
            DesignEngine::Mysql => format!("RENAME TABLE {} TO {}", w.t(&b.name), w.t(&a.name)),
            _ => format!("ALTER TABLE {} RENAME TO {}", w.t(&b.name), w.q(&a.name)),
        };
        w.out.push(sql);
    }
    let moves: Vec<(&str, &str)> = a
        .columns
        .iter()
        .filter_map(|c| {
            let old = p.column(c)?;
            (old.name != c.name).then_some((old.name.as_str(), c.name.as_str()))
        })
        .collect();
    let taken = |n: &str| b.columns.iter().any(|x| x.name.eq_ignore_ascii_case(n));
    let mut parked = Vec::new();
    for &(old, new) in moves.iter().filter(|(_, new)| taken(new)) {
        let mut tmp = format!("{old}_nd_tmp");
        while taken(&tmp) || a.columns.iter().any(|x| x.name.eq_ignore_ascii_case(&tmp)) {
            tmp.push('_');
        }
        w.note(DesignNoteCode::RenameViaTemp, Some(old));
        rename_column(w, &a.name, old, &tmp);
        parked.push((tmp, new));
    }
    for &(old, new) in moves.iter().filter(|(_, new)| !taken(new)) {
        rename_column(w, &a.name, old, new);
    }
    for (tmp, new) in parked {
        rename_column(w, &a.name, &tmp, new);
    }
}

fn rename_column(w: &mut W, table: &str, old: &str, new: &str) {
    if w.e == DesignEngine::Mssql {
        let sql = format!(
            "EXEC sp_rename {}, {}, N'COLUMN'",
            w.lit(&format!("{}.{}.{}", w.ns, table, old)),
            w.lit(new)
        );
        w.out.push(sql);
    } else if w.e == DesignEngine::Firebird {
        let sql = format!("ALTER COLUMN {} TO {}", w.q(old), w.q(new));
        w.alter(table, &sql);
    } else {
        let sql = format!("RENAME COLUMN {} TO {}", w.q(old), w.q(new));
        w.alter(table, &sql);
    }
}

fn mssql_drop_default(w: &mut W, table: &str, x: &DesignColumn) {
    if let Some(n) = &x.default_name {
        let sql = format!("DROP CONSTRAINT {}", w.q(n));
        w.alter(table, &sql);
    }
}

fn mysql_position(w: &W, a: &TableDesign, at: usize) -> String {
    match at.checked_sub(1).and_then(|i| a.columns.get(i)) {
        Some(prev) => format!(" AFTER {}", w.q(&prev.name)),
        None => " FIRST".to_owned(),
    }
}

fn columns(w: &mut W, p: &Pairs) {
    let a = p.after;
    let table = a.name.as_str();
    for d in p.dropped() {
        w.note(DesignNoteCode::DropsData, Some(&d.name));
        if w.e == DesignEngine::Mssql {
            mssql_drop_default(w, table, d);
        }
        let word = if w.e == DesignEngine::Firebird {
            "DROP"
        } else {
            "DROP COLUMN"
        };
        let sql = format!("{word} {}", w.q(&d.name));
        w.alter(table, &sql);
    }
    let natural = p.natural_order();
    for (at, c) in a.columns.iter().enumerate() {
        if p.column(c).is_some() {
            continue;
        }
        if !c.nullable && text(c.default.as_deref()).is_none() && !c.identity {
            w.note(DesignNoteCode::NotNullNeedsValue, Some(&c.name));
        }
        if c.identity && w.e == DesignEngine::Duckdb {
            w.note(DesignNoteCode::IdentityUnsupported, Some(&c.name));
        }
        let mut def = w.col_def(table, c, false);
        if w.e == DesignEngine::Mysql && natural.iter().position(|n| n == &c.name) != Some(at) {
            def.push_str(&mysql_position(w, a, at));
        }
        let word = if matches!(w.e, DesignEngine::Mssql | DesignEngine::Firebird) {
            "ADD"
        } else {
            "ADD COLUMN"
        };
        w.alter(table, &format!("{word} {def}"));
    }
    for c in &a.columns {
        let Some(x) = p.column(c) else { continue };
        alter_column(w, table, x, c);
    }
}

fn alter_column(w: &mut W, t: &str, x: &DesignColumn, c: &DesignColumn) {
    let (ty, null, default, identity) = col_changed(w.e, x, c);
    let comment = !same_opt(x.comment.as_deref(), c.comment.as_deref());
    if ty {
        w.note(DesignNoteCode::TypeChange, Some(&c.name));
    }
    let col = w.q(&c.name);
    match w.e {
        DesignEngine::Mysql => {
            if ty || null || default || identity || comment {
                let sql = format!("MODIFY COLUMN {}", w.col_def(t, c, false));
                w.alter(t, &sql);
            }
        }
        DesignEngine::Postgres
        | DesignEngine::Cockroach
        | DesignEngine::Duckdb
        | DesignEngine::Firebird => {
            if ty {
                let using = if w.e == DesignEngine::Postgres {
                    format!(" USING {col}::{}", c.type_name.trim())
                } else {
                    String::new()
                };
                let word = if w.e == DesignEngine::Duckdb {
                    "ALTER"
                } else {
                    "ALTER COLUMN"
                };
                w.alter(
                    t,
                    &format!("{word} {col} TYPE {}{using}", c.type_name.trim()),
                );
            }
            if null {
                let how = if c.nullable { "DROP" } else { "SET" };
                w.alter(t, &format!("ALTER COLUMN {col} {how} NOT NULL"));
            }
            if identity {
                if w.e == DesignEngine::Postgres {
                    let how = if c.identity {
                        "ADD GENERATED BY DEFAULT AS IDENTITY"
                    } else {
                        "DROP IDENTITY"
                    };
                    w.alter(t, &format!("ALTER COLUMN {col} {how}"));
                } else {
                    w.note(DesignNoteCode::IdentityUnsupported, Some(&c.name));
                }
            }
            if default && !(c.identity && w.e != DesignEngine::Duckdb) {
                match text(c.default.as_deref()) {
                    Some(expr) => w.alter(t, &format!("ALTER COLUMN {col} SET DEFAULT {expr}")),
                    None => w.alter(t, &format!("ALTER COLUMN {col} DROP DEFAULT")),
                }
            }
            if comment {
                w.comment(t, Some(&c.name), x.comment.as_deref(), c.comment.as_deref());
            }
        }
        DesignEngine::Mssql => {
            let had_default = x.default_name.is_some() && text(x.default.as_deref()).is_some();
            let redo_default = default || (ty && had_default);
            if redo_default && had_default {
                w.note(DesignNoteCode::DefaultConstraint, x.default_name.as_deref());
                mssql_drop_default(w, t, x);
            }
            if ty || null {
                let null_sql = if c.nullable { "NULL" } else { "NOT NULL" };
                w.alter(
                    t,
                    &format!("ALTER COLUMN {col} {} {null_sql}", c.type_name.trim()),
                );
            }
            if redo_default && let Some(d) = text(c.default.as_deref()) {
                let name = W::default_name(t, c);
                w.alter(
                    t,
                    &format!("ADD CONSTRAINT {} DEFAULT {d} FOR {col}", w.q(&name)),
                );
            }
            if identity {
                w.note(DesignNoteCode::IdentityUnsupported, Some(&c.name));
            }
            if comment {
                w.comment(t, Some(&c.name), x.comment.as_deref(), c.comment.as_deref());
            }
        }
        DesignEngine::Sqlite => {
            if comment {
                w.note(DesignNoteCode::CommentUnsupported, None);
            }
        }
    }
}

fn reorder(w: &mut W, p: &Pairs) {
    if !p.reordered() {
        return;
    }
    let a = p.after;
    match w.e {
        DesignEngine::Mysql => {
            let want: Vec<String> = a.columns.iter().map(|c| c.name.clone()).collect();
            let mut cur = p.natural_order();
            for c in a.columns.iter().filter(|c| p.column(c).is_none()) {
                cur.retain(|n| n != &c.name);
                let at = want.iter().position(|n| n == &c.name).unwrap_or(cur.len());
                cur.insert(at.min(cur.len()), c.name.clone());
            }
            for i in 0..want.len() {
                if cur.get(i) == want.get(i) {
                    continue;
                }
                let name = &want[i];
                w.note(DesignNoteCode::ReorderRewrites, Some(name));
                if let Some(c) = a.columns.iter().find(|c| &c.name == name) {
                    let sql = format!(
                        "MODIFY COLUMN {}{}",
                        w.col_def(&a.name, c, false),
                        mysql_position(w, a, i)
                    );
                    w.alter(&a.name, &sql);
                }
                cur.retain(|n| n != name);
                cur.insert(i, name.clone());
            }
        }
        DesignEngine::Firebird => {
            for (i, c) in a.columns.iter().enumerate() {
                let sql = format!("ALTER COLUMN {} POSITION {}", w.q(&c.name), i + 1);
                w.alter(&a.name, &sql);
            }
        }
        _ => w.note(DesignNoteCode::ReorderUnsupported, None),
    }
}

fn adds(w: &mut W, p: &Pairs, touched: &[String]) {
    let (b, a) = (p.before, p.after);
    let table = a.name.as_str();
    let pk_touched = b.primary_key.iter().any(|c| touched.contains(c));
    if p.pk_changed() || pk_touched {
        if w.e == DesignEngine::Cockroach {
            if a.primary_key.is_empty() {
                w.note(DesignNoteCode::PrimaryKeyRequired, None);
            } else {
                let sql = format!(
                    "ALTER PRIMARY KEY USING COLUMNS ({})",
                    w.list(&a.primary_key)
                );
                w.alter(table, &sql);
            }
        } else if !a.primary_key.is_empty() {
            let sql = format!("ADD {}", w.pk_clause(a));
            w.alter(table, &sql);
        }
    }
    for i in &a.indexes {
        let keep = p.index(i).is_some_and(|x| {
            p.index_same(x, i)
                && !x.columns.iter().any(|c| touched.contains(c))
                && !p.blocks_rename(w.e, x)
        });
        if !keep {
            w.create_index(table, i);
        }
    }
    for c in &a.checks {
        if !p.check(c).is_some_and(|x| Pairs::check_same(x, c)) {
            let sql = format!("ADD {}", w.check_clause(c));
            w.alter(table, &sql);
        }
    }
    for f in &a.foreign_keys {
        if !p.fk(f).is_some_and(|x| p.fk_same(x, f)) {
            let sql = format!("ADD {}", w.fk_clause(f));
            w.alter(table, &sql);
        }
    }
    w.comment(table, None, b.comment.as_deref(), a.comment.as_deref());
}

pub fn design_script(
    before: Option<&TableDesign>,
    after: &TableDesign,
    engine: DesignEngine,
) -> DesignScript {
    let invalid = problems(after);
    if !invalid.is_empty() {
        return DesignScript {
            notes: invalid,
            ..DesignScript::default()
        };
    }
    let mut w = W {
        e: engine,
        d: engine.dialect(),
        ns: &after.namespace,
        out: Vec::new(),
        notes: Vec::new(),
    };
    if engine == DesignEngine::Duckdb {
        for f in &after.foreign_keys {
            let plain = |a| matches!(a, FkAction::NoAction | FkAction::Restrict);
            if !plain(f.on_delete) || !plain(f.on_update) {
                w.note(DesignNoteCode::FkActionUnsupported, Some(&f.name));
            }
        }
    }
    let Some(before) = before else {
        create(&mut w, after);
        return DesignScript {
            statements: w.out,
            notes: w.notes,
            rebuild: false,
        };
    };
    let kept_name;
    let after = if engine == DesignEngine::Firebird && after.name != before.name {
        w.note(DesignNoteCode::RenameTableUnsupported, Some(&before.name));
        kept_name = TableDesign {
            name: before.name.clone(),
            ..after.clone()
        };
        &kept_name
    } else {
        after
    };
    let p = Pairs::new(before, after);
    if needs_rebuild(engine, &p) {
        let blocked = trigger_blocks(&p);
        if !blocked.is_empty() {
            return DesignScript {
                notes: blocked,
                rebuild: true,
                ..DesignScript::default()
            };
        }
        rebuild(&mut w, &p);
        return DesignScript {
            statements: w.out,
            notes: w.notes,
            rebuild: true,
        };
    }
    let touched: Vec<String> = if engine == DesignEngine::Mssql {
        after
            .columns
            .iter()
            .filter_map(|c| {
                let x = p.column(c)?;
                let (ty, null, _, _) = col_changed(engine, x, c);
                (ty || null).then(|| x.name.clone())
            })
            .collect()
    } else {
        Vec::new()
    };
    drops(&mut w, &p, &touched);
    renames(&mut w, &p);
    columns(&mut w, &p);
    reorder(&mut w, &p);
    adds(&mut w, &p, &touched);
    DesignScript {
        statements: w.out,
        notes: w.notes,
        rebuild: false,
    }
}
