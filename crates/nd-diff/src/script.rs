use nd_driver::{Dialect, FkAction};
use nd_sql::ident;

use crate::{
    Col, Fk, Idx, Routine, Snapshot, Table, View, computed_kind, in_place_keys, pair_indexes,
    same_col, squash,
};

struct Out {
    dialect: Dialect,
    ns: String,
    blocks: Vec<Vec<String>>,
}

impl Out {
    fn q(&self, name: &str) -> String {
        ident(self.dialect, name)
    }

    fn t(&self, name: &str) -> String {
        if self.dialect == Dialect::Sqlite || self.ns.is_empty() {
            self.q(name)
        } else {
            format!("{}.{}", self.q(&self.ns), self.q(name))
        }
    }

    fn list(&self, names: &[String]) -> String {
        names
            .iter()
            .map(|n| self.q(n))
            .collect::<Vec<_>>()
            .join(", ")
    }

    fn block(&mut self, lines: Vec<String>) {
        if !lines.is_empty() {
            self.blocks.push(lines);
        }
    }

    fn col_type(&self, c: &Col) -> String {
        let serial = c.generated
            && c.default
                .as_deref()
                .is_some_and(|d| d.starts_with("nextval("));
        if self.dialect == Dialect::Postgres && serial {
            return match squash(&c.type_name).as_str() {
                "bigint" | "int8" => "bigserial".into(),
                "smallint" | "int2" => "smallserial".into(),
                _ => "serial".into(),
            };
        }
        c.type_name.clone()
    }

    fn col_def(&self, c: &Col) -> String {
        let mut s = format!("{} {}", self.q(&c.name), self.col_type(c));
        if c.generated && self.dialect == Dialect::Mssql {
            s.push_str(" IDENTITY(1,1)");
        }
        if !c.nullable && self.dialect != Dialect::Clickhouse {
            s.push_str(" NOT NULL");
        }
        match (&c.default, c.generated, self.dialect) {
            (Some(d), false, Dialect::Clickhouse) if computed_kind(d) => {
                s.push(' ');
                s.push_str(d);
            }
            (Some(d), true, Dialect::Postgres) if d.starts_with("GENERATED") => {
                s.push(' ');
                s.push_str(d);
            }
            (_, true, Dialect::Mysql) => s.push_str(" AUTO_INCREMENT"),
            (Some(d), false, _) => {
                s.push_str(" DEFAULT ");
                s.push_str(d);
            }
            _ => {}
        }
        s
    }

    fn fk_clause(&self, f: &Fk) -> String {
        let mut s = format!(
            "CONSTRAINT {} FOREIGN KEY ({}) REFERENCES {} ({})",
            self.q(&f.name),
            self.list(&f.columns),
            self.t(&f.ref_table),
            self.list(&f.ref_columns)
        );
        for (word, a) in [("DELETE", f.on_delete), ("UPDATE", f.on_update)] {
            let action = match a {
                FkAction::NoAction => continue,
                FkAction::Restrict => "RESTRICT",
                FkAction::Cascade => "CASCADE",
                FkAction::SetNull => "SET NULL",
                FkAction::SetDefault => "SET DEFAULT",
            };
            s.push_str(" ON ");
            s.push_str(word);
            s.push(' ');
            s.push_str(action);
        }
        s
    }

    fn create_table(&self, t: &Table) -> String {
        if self.dialect == Dialect::Clickhouse {
            return self.clickhouse_table(t);
        }
        let mut parts: Vec<String> = t.columns.iter().map(|c| self.col_def(c)).collect();
        if !t.primary_key.is_empty() {
            parts.push(format!("PRIMARY KEY ({})", self.list(&t.primary_key)));
        }
        if !in_place_keys(self.dialect) {
            parts.extend(t.foreign_keys.iter().map(|f| self.fk_clause(f)));
        }
        format!(
            "CREATE TABLE {} (\n  {}\n)",
            self.t(&t.name),
            parts.join(",\n  ")
        )
    }

    fn clickhouse_table(&self, t: &Table) -> String {
        if let Some(def) = &t.create {
            return def.trim().trim_end_matches(';').trim_end().to_owned();
        }
        let cols: Vec<String> = t.columns.iter().map(|c| self.col_def(c)).collect();
        let order = if t.primary_key.is_empty() {
            "tuple()".to_owned()
        } else {
            format!("({})", t.primary_key.join(", "))
        };
        format!(
            "CREATE TABLE {} (\n  {}\n)\nENGINE = MergeTree\nORDER BY {order}",
            self.t(&t.name),
            cols.join(",\n  ")
        )
    }

    fn create_index(&self, table: &str, i: &Idx) -> String {
        if self.dialect == Dialect::Clickhouse {
            let spec = i.columns.join(", ");
            return self.alter(table, &format!("ADD INDEX {} {spec}", self.q(&i.name)));
        }
        let unique = if i.unique { "UNIQUE " } else { "" };
        format!(
            "CREATE {unique}INDEX {} ON {} ({})",
            self.q(&i.name),
            self.t(table),
            self.list(&i.columns)
        )
    }

    fn drop_index(&self, table: &str, i: &Idx) -> String {
        match self.dialect {
            Dialect::Mysql | Dialect::Mssql => {
                format!("DROP INDEX {} ON {}", self.q(&i.name), self.t(table))
            }
            Dialect::Clickhouse => self.alter(table, &format!("DROP INDEX {}", self.q(&i.name))),
            _ => format!("DROP INDEX {}", self.t(&i.name)),
        }
    }

    fn drop_fk(&self, table: &str, f: &Fk) -> String {
        let how = if self.dialect == Dialect::Mysql {
            "FOREIGN KEY"
        } else {
            "CONSTRAINT"
        };
        format!(
            "ALTER TABLE {} DROP {how} {}",
            self.t(table),
            self.q(&f.name)
        )
    }

    fn alter(&self, table: &str, what: &str) -> String {
        format!("ALTER TABLE {} {what}", self.t(table))
    }

    fn add_column(&self, table: &str, c: &Col) -> String {
        match self.dialect {
            Dialect::Mssql | Dialect::Firebird => {
                self.alter(table, &format!("ADD {}", self.col_def(c)))
            }
            Dialect::Hana | Dialect::Oracle => {
                self.alter(table, &format!("ADD ({})", self.col_def(c)))
            }
            _ => self.alter(table, &format!("ADD COLUMN {}", self.col_def(c))),
        }
    }

    fn drop_column(&self, table: &str, name: &str) -> String {
        match self.dialect {
            Dialect::Firebird => self.alter(table, &format!("DROP {}", self.q(name))),
            Dialect::Hana => self.alter(table, &format!("DROP ({})", self.q(name))),
            _ => self.alter(table, &format!("DROP COLUMN {}", self.q(name))),
        }
    }

    fn change_column(&self, table: &str, s: &Col, t: &Col) -> Vec<String> {
        let c = self.q(&s.name);
        match self.dialect {
            Dialect::Mssql => {
                let mut out = Vec::new();
                if !crate::same_type(&s.type_name, &t.type_name) || s.nullable != t.nullable {
                    let null = if s.nullable { "NULL" } else { "NOT NULL" };
                    out.push(
                        self.alter(table, &format!("ALTER COLUMN {c} {} {null}", s.type_name)),
                    );
                }
                if !s.generated
                    && s.default.as_deref().map(squash) != t.default.as_deref().map(squash)
                {
                    if let Some(name) = &t.default_name {
                        out.push(self.alter(table, &format!("DROP CONSTRAINT {}", self.q(name))));
                    }
                    if let Some(d) = &s.default {
                        out.push(self.alter(table, &format!("ADD DEFAULT {d} FOR {c}")));
                    }
                }
                out
            }
            Dialect::Mysql | Dialect::Clickhouse => {
                vec![self.alter(table, &format!("MODIFY COLUMN {}", self.col_def(s)))]
            }
            Dialect::Sqlite
            | Dialect::Cql
            | Dialect::Partiql
            | Dialect::Opensearch
            | Dialect::Elasticsearch => Vec::new(),
            Dialect::Hana => vec![self.alter(table, &format!("ALTER ({})", self.col_def(s)))],
            Dialect::Oracle => {
                let mut out = Vec::new();
                if !crate::same_type(&s.type_name, &t.type_name) {
                    out.push(self.alter(table, &format!("MODIFY ({c} {})", s.type_name)));
                }
                if s.nullable != t.nullable {
                    let null = if s.nullable { "NULL" } else { "NOT NULL" };
                    out.push(self.alter(table, &format!("MODIFY ({c} {null})")));
                }
                if !s.generated
                    && s.default.as_deref().map(squash) != t.default.as_deref().map(squash)
                {
                    let d = s.default.as_deref().unwrap_or("NULL");
                    out.push(self.alter(table, &format!("MODIFY ({c} DEFAULT {d})")));
                }
                out
            }
            Dialect::Postgres
            | Dialect::Duckdb
            | Dialect::Firebird
            | Dialect::Db2
            | Dialect::Ansi
            | Dialect::AnsiBacktick => {
                let mut out = Vec::new();
                if !crate::same_type(&s.type_name, &t.type_name) {
                    let using = if self.dialect == Dialect::Postgres {
                        format!(" USING {c}::{}", s.type_name)
                    } else {
                        String::new()
                    };
                    let word = if matches!(
                        self.dialect,
                        Dialect::Db2 | Dialect::Ansi | Dialect::AnsiBacktick
                    ) {
                        "SET DATA TYPE"
                    } else {
                        "TYPE"
                    };
                    out.push(self.alter(
                        table,
                        &format!("ALTER COLUMN {c} {word} {}{using}", s.type_name),
                    ));
                }
                if s.nullable != t.nullable {
                    let how = if s.nullable { "DROP" } else { "SET" };
                    out.push(self.alter(table, &format!("ALTER COLUMN {c} {how} NOT NULL")));
                }
                if !s.generated
                    && s.default.as_deref().map(squash) != t.default.as_deref().map(squash)
                {
                    out.push(match &s.default {
                        Some(d) => self.alter(table, &format!("ALTER COLUMN {c} SET DEFAULT {d}")),
                        None => self.alter(table, &format!("ALTER COLUMN {c} DROP DEFAULT")),
                    });
                }
                out
            }
        }
    }

    fn primary_key(&self, table: &str, s: &[String], t: &Table) -> Vec<String> {
        let mut out = Vec::new();
        if !in_place_keys(self.dialect) {
            return out;
        }
        if !t.primary_key.is_empty() {
            out.push(if self.dialect == Dialect::Mysql {
                self.alter(table, "DROP PRIMARY KEY")
            } else {
                let name = t.pk_name.clone().unwrap_or_else(|| format!("{table}_pkey"));
                self.alter(table, &format!("DROP CONSTRAINT {}", self.q(&name)))
            });
        }
        if !s.is_empty() {
            out.push(self.alter(table, &format!("ADD PRIMARY KEY ({})", self.list(s))));
        }
        out
    }

    fn view(&self, v: &View, replace: bool) -> Vec<String> {
        let Some(def) = v.definition.as_deref() else {
            return Vec::new();
        };
        let def = def.trim().trim_end_matches(';').trim_end();
        match self.dialect {
            Dialect::Mssql | Dialect::Duckdb => {
                vec![
                    crate::restate(def, &v.name, self.dialect, Some(&self.ns))
                        .unwrap_or_else(|| def.to_owned()),
                ]
            }
            Dialect::Sqlite if replace => {
                vec![format!("DROP VIEW {}", self.t(&v.name)), def.to_owned()]
            }
            Dialect::Sqlite => vec![def.to_owned()],
            Dialect::Clickhouse if def.starts_with("CREATE ") => {
                let mut out = Vec::new();
                if replace {
                    out.push(format!("DROP VIEW {}", self.t(&v.name)));
                }
                out.push(def.to_owned());
                out
            }
            Dialect::Firebird => vec![format!(
                "CREATE OR ALTER VIEW {} AS\n{def}",
                self.t(&v.name)
            )],
            _ => vec![format!(
                "CREATE OR REPLACE VIEW {} AS\n{def}",
                self.t(&v.name)
            )],
        }
    }

    fn qualify_routine(&self, def: &str, r: &Routine) -> String {
        let kind = if r.procedure { "PROCEDURE" } else { "FUNCTION" };
        for name in [r.name.clone(), self.q(&r.name)] {
            let bare = format!("{kind} {name}(");
            if def.contains(&bare) {
                return def.replacen(&bare, &format!("{kind} {}(", self.t(&r.name)), 1);
            }
        }
        def.to_owned()
    }

    fn drop_routine(&self, r: &Routine) -> String {
        let kind = if r.procedure { "PROCEDURE" } else { "FUNCTION" };
        match self.dialect {
            Dialect::Postgres => format!("DROP {kind} {}({})", self.t(&r.name), r.signature),
            Dialect::Duckdb if r.procedure => format!("DROP MACRO TABLE {}", self.t(&r.name)),
            Dialect::Duckdb => format!("DROP MACRO {}", self.t(&r.name)),
            _ => format!("DROP {kind} {}", self.t(&r.name)),
        }
    }
}

fn routine_key(r: &Routine) -> (String, String) {
    (r.name.clone(), squash(&r.signature))
}

fn same_def(a: Option<&str>, b: Option<&str>) -> bool {
    a.map(squash) == b.map(squash)
}

fn rehome(def: &str, from: &str, to: &str, dialect: Dialect) -> String {
    if from == to || from.is_empty() || to.is_empty() {
        return def.to_owned();
    }
    let (a, b) = (
        format!("{}.", ident(dialect, from)),
        format!("{}.", ident(dialect, to)),
    );
    let (c, d) = (format!("{from}."), format!("{to}."));
    if def.contains(&a) {
        def.replacen(&a, &b, 1)
    } else {
        def.replacen(&c, &d, 1)
    }
}

fn fk_same(a: &Fk, b: &Fk) -> bool {
    a.columns == b.columns
        && a.ref_table == b.ref_table
        && a.ref_columns == b.ref_columns
        && a.on_delete == b.on_delete
        && a.on_update == b.on_update
}

struct Plan<'a> {
    source: &'a Snapshot,
    target: &'a Snapshot,
    pairs: Vec<(&'a Table, &'a Table)>,
    added: Vec<&'a Table>,
    removed: Vec<&'a Table>,
}

impl Plan<'_> {
    fn drop_foreign_keys(&self, o: &Out) -> Vec<String> {
        let mut lines = Vec::new();
        if !in_place_keys(o.dialect) {
            return lines;
        }
        for (s, t) in &self.pairs {
            for f in &t.foreign_keys {
                if !s.foreign_keys.iter().any(|x| fk_same(x, f)) {
                    lines.push(o.drop_fk(&t.name, f));
                }
            }
        }
        for t in &self.removed {
            lines.extend(t.foreign_keys.iter().map(|f| o.drop_fk(&t.name, f)));
        }
        lines
    }

    fn drop_views_and_routines(&self, o: &Out) -> Vec<String> {
        let mut lines = Vec::new();
        for v in &self.target.views {
            if !self.source.views.iter().any(|s| s.name == v.name) {
                lines.push(format!("DROP VIEW {}", o.t(&v.name)));
            }
        }
        for r in &self.target.routines {
            if !self
                .source
                .routines
                .iter()
                .any(|s| routine_key(s) == routine_key(r))
            {
                lines.push(o.drop_routine(r));
            }
        }
        lines
    }

    fn drop_indexes(&self, o: &Out) -> Vec<String> {
        let mut lines = Vec::new();
        for (s, t) in &self.pairs {
            for (a, b) in pair_indexes(&s.indexes, &t.indexes) {
                if let Some(b) = b
                    && a.is_none_or(|a| a.columns != b.columns || a.unique != b.unique)
                {
                    lines.push(o.drop_index(&t.name, b));
                }
            }
        }
        lines
    }

    fn alter_table(o: &Out, s: &Table, t: &Table) -> Vec<String> {
        let sqlite = o.dialect == Dialect::Sqlite;
        let mut lines = Vec::new();
        for c in &s.columns {
            match t.columns.iter().find(|x| x.name == c.name) {
                None if sqlite && (c.generated || (!c.nullable && c.default.is_none())) => {}
                None => lines.push(o.add_column(&s.name, c)),
                Some(x) if !same_col(c, x) => lines.extend(o.change_column(&s.name, c, x)),
                Some(_) => {}
            }
        }
        for x in t
            .columns
            .iter()
            .filter(|x| !s.columns.iter().any(|c| c.name == x.name))
        {
            lines.push(o.drop_column(&s.name, &x.name));
        }
        if s.primary_key != t.primary_key {
            lines.extend(o.primary_key(&s.name, &s.primary_key, t));
        }
        for (a, b) in pair_indexes(&s.indexes, &t.indexes) {
            if let Some(a) = a
                && b.is_none_or(|b| a.columns != b.columns || a.unique != b.unique)
            {
                lines.push(o.create_index(&s.name, a));
            }
        }
        lines
    }

    fn add_foreign_keys(&self, o: &Out) -> Vec<String> {
        let mut lines = Vec::new();
        if !in_place_keys(o.dialect) {
            return lines;
        }
        let add = |table: &str, f: &Fk| o.alter(table, &format!("ADD {}", o.fk_clause(f)));
        for t in &self.added {
            lines.extend(t.foreign_keys.iter().map(|f| add(&t.name, f)));
        }
        for (s, t) in &self.pairs {
            for f in &s.foreign_keys {
                if !t.foreign_keys.iter().any(|x| fk_same(x, f)) {
                    lines.push(add(&s.name, f));
                }
            }
        }
        lines
    }

    fn views(&self, o: &Out) -> Vec<String> {
        let (from, to) = (&self.source.namespace, &self.target.namespace);
        let mut lines = Vec::new();
        for v in &self.source.views {
            let t = self.target.views.iter().find(|t| t.name == v.name);
            if t.is_none_or(|t| !same_def(t.definition.as_deref(), v.definition.as_deref())) {
                let moved = View {
                    name: v.name.clone(),
                    definition: v
                        .definition
                        .as_deref()
                        .map(|d| rehome(d, from, to, o.dialect)),
                };
                lines.extend(o.view(&moved, t.is_some()));
            }
        }
        lines
    }

    fn routines(&self, o: &Out) -> Vec<String> {
        let mut lines = Vec::new();
        for r in &self.source.routines {
            let t = self
                .target
                .routines
                .iter()
                .find(|t| routine_key(t) == routine_key(r));
            if t.is_some_and(|t| same_def(t.definition.as_deref(), r.definition.as_deref())) {
                continue;
            }
            let Some(def) = r.definition.as_deref() else {
                continue;
            };
            if t.is_some() && o.dialect == Dialect::Mysql {
                lines.push(o.drop_routine(r));
            }
            let def = def.trim().trim_end_matches(';').trim_end();
            lines.push(match o.dialect {
                Dialect::Postgres => o.qualify_routine(def, r),
                Dialect::Mssql | Dialect::Duckdb => {
                    crate::restate(def, &r.name, o.dialect, Some(&o.ns))
                        .unwrap_or_else(|| def.to_owned())
                }
                _ => def.to_owned(),
            });
        }
        lines
    }
}

fn referenced_first(mut left: Vec<&Table>) -> Vec<&Table> {
    let mut out: Vec<&Table> = Vec::new();
    while !left.is_empty() {
        let at = left
            .iter()
            .position(|t| {
                t.foreign_keys.iter().all(|f| {
                    f.ref_table == t.name
                        || out.iter().any(|o| o.name == f.ref_table)
                        || !left.iter().any(|l| l.name == f.ref_table)
                })
            })
            .unwrap_or(0);
        out.push(left.remove(at));
    }
    out
}

pub fn script(source: &Snapshot, target: &Snapshot, dialect: Dialect) -> Vec<String> {
    let has = |set: &Snapshot, name: &str| set.tables.iter().any(|t| t.name == name);
    let plan = Plan {
        source,
        target,
        pairs: source
            .tables
            .iter()
            .filter_map(|s| {
                target
                    .tables
                    .iter()
                    .find(|t| t.name == s.name)
                    .map(|t| (s, t))
            })
            .collect(),
        added: source
            .tables
            .iter()
            .filter(|s| !has(target, &s.name))
            .collect(),
        removed: target
            .tables
            .iter()
            .filter(|t| !has(source, &t.name))
            .collect(),
    };
    let mut plan = plan;
    if !in_place_keys(dialect) {
        plan.added = referenced_first(plan.added);
    }
    let mut o = Out {
        dialect,
        ns: target.namespace.clone(),
        blocks: Vec::new(),
    };
    let drops = [
        plan.drop_foreign_keys(&o),
        plan.drop_views_and_routines(&o),
        plan.drop_indexes(&o),
    ];
    for b in drops {
        o.block(b);
    }
    for t in &plan.added {
        let mut lines = vec![o.create_table(t)];
        if o.dialect != Dialect::Clickhouse || t.create.is_none() {
            lines.extend(t.indexes.iter().map(|i| o.create_index(&t.name, i)));
        }
        o.block(lines);
    }
    for (s, t) in &plan.pairs {
        let lines = Plan::alter_table(&o, s, t);
        o.block(lines);
    }
    let gone = plan
        .removed
        .iter()
        .map(|t| format!("DROP TABLE {}", o.t(&t.name)))
        .collect();
    o.block(gone);
    let rest = [plan.add_foreign_keys(&o), plan.views(&o), plan.routines(&o)];
    for b in rest {
        o.block(b);
    }
    o.blocks.into_iter().flatten().collect()
}
