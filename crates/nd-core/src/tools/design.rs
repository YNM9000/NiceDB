use crate::session::Sql as _;
use std::collections::HashMap;

use nd_diff::design::{
    DesignCheck, DesignColumn, DesignForeignKey, DesignIndex, DesignTrigger, TableDesign,
    check_body,
};
use nd_driver::{Connection, Dialect, DriverError, Relation};

use super::Engine;
use super::compare::{cell, columns_extra, table_with};
use crate::guard::{Access, collect};
use crate::{Core, CoreError, Result};

type Found<T> = std::result::Result<T, DriverError>;

#[derive(Default)]
struct More {
    table_comment: Option<String>,
    comments: HashMap<String, String>,
    checks: Vec<(String, String)>,
    pk: Option<(Option<String>, Vec<String>)>,
    identity: Vec<String>,
    triggers: Vec<DesignTrigger>,
    indexes: HashMap<String, IndexShape>,
}

#[derive(Default, Debug, PartialEq)]
struct IndexShape {
    parts: Vec<(String, bool, bool)>,
    predicate: Option<String>,
}

impl IndexShape {
    fn add(&mut self, part: String, desc: bool, expr: bool) {
        self.parts.push((part, desc, expr));
    }
}

fn shapes(
    found: Vec<nd_driver::Row>,
    mut part: impl FnMut(&nd_driver::Row) -> Option<(String, bool, bool, Option<String>)>,
) -> HashMap<String, IndexShape> {
    let mut out: HashMap<String, IndexShape> = HashMap::new();
    for r in found {
        let (Some(name), Some((p, desc, expr, predicate))) = (cell(&r, 0), part(&r)) else {
            continue;
        };
        let shape = out.entry(name).or_default();
        shape.add(p, desc, expr);
        if shape.predicate.is_none() {
            shape.predicate = predicate
                .filter(|x| !x.trim().is_empty())
                .map(|x| check_body(&x));
        }
    }
    out
}

fn yes(r: &nd_driver::Row, i: usize) -> bool {
    cell(r, i).is_some_and(|v| matches!(v.as_str(), "1" | "t" | "true" | "D"))
}

async fn rows(conn: &dyn Connection, sql: &str, params: &[&str]) -> Found<Vec<nd_driver::Row>> {
    let p: Vec<Option<String>> = params.iter().map(|s| Some((*s).to_owned())).collect();
    collect(conn, sql, Some(&p)).await.map(|(r, _)| r)
}

const PG_COMMENTS: &str = "SELECT a.attname, col_description(c.oid, a.attnum) FROM pg_attribute a \
JOIN pg_class c ON c.oid = a.attrelid JOIN pg_namespace n ON n.oid = c.relnamespace \
WHERE n.nspname = $1 AND c.relname = $2 AND a.attnum > 0 AND NOT a.attisdropped \
AND col_description(c.oid, a.attnum) IS NOT NULL \
UNION ALL SELECT '', obj_description(c.oid, 'pg_class') FROM pg_class c \
JOIN pg_namespace n ON n.oid = c.relnamespace WHERE n.nspname = $1 AND c.relname = $2 \
AND obj_description(c.oid, 'pg_class') IS NOT NULL";

const PG_CONSTRAINTS: &str = "SELECT k.conname, k.contype::text, pg_get_constraintdef(k.oid) FROM pg_constraint k \
JOIN pg_class c ON c.oid = k.conrelid JOIN pg_namespace n ON n.oid = c.relnamespace \
WHERE n.nspname = $1 AND c.relname = $2 AND k.contype IN ('c', 'p') ORDER BY k.conname";

const MY_COMMENTS: &str = "SELECT COLUMN_NAME, COLUMN_COMMENT FROM information_schema.COLUMNS \
WHERE TABLE_SCHEMA = ? AND TABLE_NAME = ? AND COLUMN_COMMENT <> '' \
UNION ALL SELECT '', TABLE_COMMENT FROM information_schema.TABLES \
WHERE TABLE_SCHEMA = ? AND TABLE_NAME = ? AND TABLE_COMMENT <> ''";

const MY_PK: &str = "SELECT COLUMN_NAME FROM information_schema.KEY_COLUMN_USAGE \
WHERE TABLE_SCHEMA = ? AND TABLE_NAME = ? AND CONSTRAINT_NAME = 'PRIMARY' ORDER BY ORDINAL_POSITION";

const MY_CHECKS: &str = "SELECT cc.CONSTRAINT_NAME, cc.CHECK_CLAUSE FROM information_schema.CHECK_CONSTRAINTS cc \
JOIN information_schema.TABLE_CONSTRAINTS tc ON tc.CONSTRAINT_SCHEMA = cc.CONSTRAINT_SCHEMA \
AND tc.CONSTRAINT_NAME = cc.CONSTRAINT_NAME AND tc.CONSTRAINT_TYPE = 'CHECK' \
WHERE tc.TABLE_SCHEMA = ? AND tc.TABLE_NAME = ? ORDER BY cc.CONSTRAINT_NAME";

const MARIA_CHECKS: &str = "SELECT CONSTRAINT_NAME, CHECK_CLAUSE FROM information_schema.CHECK_CONSTRAINTS \
WHERE CONSTRAINT_SCHEMA = ? AND TABLE_NAME = ? ORDER BY CONSTRAINT_NAME";

const MS_COMMENTS: &str = "SELECT COALESCE(c.name, ''), CAST(ep.value AS nvarchar(max)) FROM sys.extended_properties ep \
JOIN sys.tables t ON t.object_id = ep.major_id JOIN sys.schemas s ON s.schema_id = t.schema_id \
LEFT JOIN sys.columns c ON c.object_id = ep.major_id AND c.column_id = ep.minor_id \
WHERE ep.class = 1 AND ep.name = 'MS_Description' AND s.name = @P1 AND t.name = @P2";

const MS_CHECKS: &str = "SELECT k.name, k.definition FROM sys.check_constraints k \
JOIN sys.tables t ON t.object_id = k.parent_object_id JOIN sys.schemas s ON s.schema_id = t.schema_id \
WHERE s.name = @P1 AND t.name = @P2 ORDER BY k.name";

const MS_PK: &str = "SELECT c.name FROM sys.indexes i JOIN sys.tables t ON t.object_id = i.object_id \
JOIN sys.schemas s ON s.schema_id = t.schema_id \
JOIN sys.index_columns ic ON ic.object_id = i.object_id AND ic.index_id = i.index_id \
JOIN sys.columns c ON c.object_id = ic.object_id AND c.column_id = ic.column_id \
WHERE i.is_primary_key = 1 AND s.name = @P1 AND t.name = @P2 ORDER BY ic.key_ordinal";

const DUCK_COMMENTS: &str = "SELECT column_name, comment FROM duckdb_columns() \
WHERE database_name = current_database() AND schema_name = ? AND table_name = ? AND comment IS NOT NULL AND comment <> '' \
UNION ALL SELECT '', comment FROM duckdb_tables() \
WHERE database_name = current_database() AND schema_name = ? AND table_name = ? AND comment IS NOT NULL AND comment <> ''";

const DUCK_CHECKS: &str = "SELECT constraint_text FROM duckdb_constraints() \
WHERE database_name = current_database() AND schema_name = ? AND table_name = ? AND constraint_type = 'CHECK' \
ORDER BY constraint_index";

const PG_INDEXES: &str = "SELECT i.relname, COALESCE(a.attname, pg_get_indexdef(ix.indexrelid, k.n, true)), \
CASE WHEN (ix.indoption[k.n - 1] & 1) = 1 THEN '1' ELSE '0' END, CASE WHEN ix.indkey[k.n - 1] = 0 THEN '1' ELSE '0' END, \
COALESCE(pg_get_expr(ix.indpred, ix.indrelid), '') FROM pg_index ix \
JOIN pg_class i ON i.oid = ix.indexrelid JOIN pg_class c ON c.oid = ix.indrelid JOIN pg_namespace n ON n.oid = c.relnamespace \
CROSS JOIN LATERAL generate_series(1, ix.indnkeyatts::int) AS k(n) \
LEFT JOIN pg_attribute a ON a.attrelid = ix.indrelid AND a.attnum = ix.indkey[k.n - 1] AND ix.indkey[k.n - 1] <> 0 \
WHERE n.nspname = $1 AND c.relname = $2 AND NOT ix.indisprimary ORDER BY i.relname, k.n";

const MY_INDEXES: &str = "SELECT INDEX_NAME, COALESCE(COLUMN_NAME, EXPRESSION), COLLATION, CASE WHEN COLUMN_NAME IS NULL THEN '1' ELSE '0' END \
FROM information_schema.STATISTICS WHERE TABLE_SCHEMA = ? AND TABLE_NAME = ? AND INDEX_NAME <> 'PRIMARY' \
ORDER BY INDEX_NAME, SEQ_IN_INDEX";

const MARIA_INDEXES: &str = "SELECT INDEX_NAME, COLUMN_NAME, COLLATION, '0' \
FROM information_schema.STATISTICS WHERE TABLE_SCHEMA = ? AND TABLE_NAME = ? AND INDEX_NAME <> 'PRIMARY' \
ORDER BY INDEX_NAME, SEQ_IN_INDEX";

const MS_INDEXES: &str = "SELECT i.name, c.name, CASE WHEN ic.is_descending_key = 1 THEN '1' ELSE '0' END, COALESCE(i.filter_definition, '') \
FROM sys.indexes i JOIN sys.tables t ON t.object_id = i.object_id JOIN sys.schemas s ON s.schema_id = t.schema_id \
JOIN sys.index_columns ic ON ic.object_id = i.object_id AND ic.index_id = i.index_id AND ic.is_included_column = 0 \
JOIN sys.columns c ON c.object_id = ic.object_id AND c.column_id = ic.column_id \
WHERE s.name = @P1 AND t.name = @P2 AND i.is_primary_key = 0 AND i.type > 0 ORDER BY i.name, ic.key_ordinal";

const SQLITE_INDEXES: &str =
    "SELECT name, sql FROM sqlite_schema WHERE type = 'index' AND tbl_name = ? AND sql IS NOT NULL";

const DUCK_INDEXES: &str = "SELECT index_name, sql FROM duckdb_indexes() \
WHERE database_name = current_database() AND schema_name = ? AND table_name = ? AND sql IS NOT NULL";

const SQLITE_SQL: &str = "SELECT sql FROM sqlite_schema WHERE type = 'table' AND name = ?";

const SQLITE_TRIGGERS: &str = "SELECT name, sql FROM sqlite_schema \
WHERE type = 'trigger' AND tbl_name = ? AND sql IS NOT NULL ORDER BY name";

fn comments_from(rows: Vec<nd_driver::Row>, m: &mut More) {
    for r in rows {
        let (Some(col), Some(text)) = (cell(&r, 0), cell(&r, 1)) else {
            continue;
        };
        if col.is_empty() {
            m.table_comment = Some(text);
        } else {
            m.comments.insert(col, text);
        }
    }
}

async fn pg_more(conn: &dyn Connection, ns: &str, t: &str, cockroach: bool) -> Found<More> {
    let mut m = More::default();
    comments_from(rows(conn, PG_COMMENTS, &[ns, t]).await?, &mut m);
    for r in rows(conn, PG_CONSTRAINTS, &[ns, t]).await? {
        let (Some(name), Some(kind), Some(def)) = (cell(&r, 0), cell(&r, 1), cell(&r, 2)) else {
            continue;
        };
        if kind == "p" {
            let cols = def
                .trim_start_matches("PRIMARY KEY")
                .trim()
                .trim_start_matches('(')
                .split(')')
                .next()
                .unwrap_or_default()
                .split(',')
                .map(|c| {
                    let c = c.trim();
                    let c = c
                        .strip_suffix(" ASC")
                        .or_else(|| c.strip_suffix(" DESC"))
                        .unwrap_or(c);
                    c.trim_matches('"').replace("\"\"", "\"")
                })
                .filter(|c| !c.is_empty())
                .collect();
            m.pk = Some((Some(name), cols));
        } else if !(cockroach && name.starts_with("check_crdb_internal")) {
            m.checks.push((name, check_body(&def)));
        }
    }
    m.indexes = shapes(rows(conn, PG_INDEXES, &[ns, t]).await?, |r| {
        Some((cell(r, 1)?, yes(r, 2), yes(r, 3), cell(r, 4)))
    });
    Ok(m)
}

async fn mysql_more(conn: &dyn Connection, ns: &str, t: &str) -> Found<More> {
    let mut m = More::default();
    comments_from(rows(conn, MY_COMMENTS, &[ns, t, ns, t]).await?, &mut m);
    let pk: Vec<String> = rows(conn, MY_PK, &[ns, t])
        .await?
        .iter()
        .filter_map(|r| cell(r, 0))
        .collect();
    if !pk.is_empty() {
        m.pk = Some((None, pk));
    }
    let maria = conn.server().product.contains("MariaDB");
    let indexes = if maria { MARIA_INDEXES } else { MY_INDEXES };
    m.indexes = shapes(rows(conn, indexes, &[ns, t]).await?, |r| {
        Some((cell(r, 1)?, yes(r, 2), yes(r, 3), None))
    });
    let sql = if maria { MARIA_CHECKS } else { MY_CHECKS };
    if let Ok(found) = rows(conn, sql, &[ns, t]).await {
        for r in found {
            if let (Some(n), Some(d)) = (cell(&r, 0), cell(&r, 1)) {
                m.checks.push((n, check_body(&d)));
            }
        }
    }
    Ok(m)
}

async fn mssql_more(conn: &dyn Connection, ns: &str, t: &str) -> Found<More> {
    let mut m = More::default();
    comments_from(rows(conn, MS_COMMENTS, &[ns, t]).await?, &mut m);
    for r in rows(conn, MS_CHECKS, &[ns, t]).await? {
        if let (Some(n), Some(d)) = (cell(&r, 0), cell(&r, 1)) {
            m.checks.push((n, check_body(&d)));
        }
    }
    let pk: Vec<String> = rows(conn, MS_PK, &[ns, t])
        .await?
        .iter()
        .filter_map(|r| cell(r, 0))
        .collect();
    if !pk.is_empty() {
        m.pk = Some((None, pk));
    }
    m.indexes = shapes(rows(conn, MS_INDEXES, &[ns, t]).await?, |r| {
        Some((cell(r, 1)?, yes(r, 2), false, cell(r, 3)))
    });
    Ok(m)
}

async fn duckdb_more(conn: &dyn Connection, ns: &str, t: &str) -> Found<More> {
    let mut m = More::default();
    comments_from(rows(conn, DUCK_COMMENTS, &[ns, t, ns, t]).await?, &mut m);
    for (i, r) in rows(conn, DUCK_CHECKS, &[ns, t]).await?.iter().enumerate() {
        if let Some(d) = cell(r, 0) {
            m.checks
                .push((format!("{t}_check_{}", i + 1), check_body(&d)));
        }
    }
    m.indexes = parsed_indexes(rows(conn, DUCK_INDEXES, &[ns, t]).await?);
    Ok(m)
}

fn parsed_indexes(found: Vec<nd_driver::Row>) -> HashMap<String, IndexShape> {
    found
        .iter()
        .filter_map(|r| Some((cell(r, 0)?, index_shape(&cell(r, 1)?)?)))
        .collect()
}

fn index_shape(sql: &str) -> Option<IndexShape> {
    let upper = sql.to_ascii_uppercase();
    let on = upper.find(" ON ")?;
    let body = &sql[on..];
    let (list, end) = parts_until(body)?;
    let mut shape = IndexShape::default();
    for p in list {
        let mut text = p.trim();
        let mut desc = false;
        for (suffix, d) in [(" DESC", true), (" ASC", false)] {
            if text.len() > suffix.len()
                && text[text.len() - suffix.len()..].eq_ignore_ascii_case(suffix)
            {
                text = text[..text.len() - suffix.len()].trim_end();
                desc = d;
            }
        }
        let name = first_name(text);
        let plain = !text.is_empty()
            && text
                .chars()
                .all(|c| c.is_alphanumeric() || c == '_' || c == '$');
        let quoted = [
            format!("\"{name}\""),
            format!("`{name}`"),
            format!("[{name}]"),
        ]
        .iter()
        .any(|b| b == text);
        let expr = !plain && !quoted;
        shape.add(if expr { text.to_owned() } else { name }, desc, expr);
    }
    let rest = body[end..].trim();
    if rest.len() > 5 && rest[..5].eq_ignore_ascii_case("WHERE") {
        shape.predicate = Some(rest[5..].trim().trim_end_matches(';').trim().to_owned());
    }
    Some(shape)
}

fn parts_until(sql: &str) -> Option<(Vec<String>, usize)> {
    let open = sql.find('(')?;
    let mut out = Vec::new();
    let mut depth = 0i32;
    let mut cur = String::new();
    let mut quote: Option<char> = None;
    for (i, ch) in sql[open + 1..].char_indices() {
        if let Some(q) = quote {
            cur.push(ch);
            if ch == q {
                quote = None;
            }
            continue;
        }
        match ch {
            '\'' | '"' | '`' | '[' => {
                quote = Some(if ch == '[' { ']' } else { ch });
                cur.push(ch);
            }
            '(' => {
                depth += 1;
                cur.push(ch);
            }
            ')' if depth == 0 => {
                out.push(cur.trim().to_owned());
                return Some((out, open + 1 + i + 1));
            }
            ')' => {
                depth -= 1;
                cur.push(ch);
            }
            ',' if depth == 0 => {
                out.push(cur.trim().to_owned());
                cur.clear();
            }
            _ => cur.push(ch),
        }
    }
    None
}

fn parts(sql: &str) -> Vec<String> {
    let Some(open) = sql.find('(') else {
        return Vec::new();
    };
    let mut out = Vec::new();
    let mut depth = 0i32;
    let mut cur = String::new();
    let mut quote: Option<char> = None;
    for ch in sql[open + 1..].chars() {
        if let Some(q) = quote {
            cur.push(ch);
            if ch == q {
                quote = None;
            }
            continue;
        }
        match ch {
            '\'' | '"' | '`' | '[' => {
                quote = Some(if ch == '[' { ']' } else { ch });
                cur.push(ch);
            }
            '(' => {
                depth += 1;
                cur.push(ch);
            }
            ')' if depth == 0 => {
                out.push(cur.trim().to_owned());
                return out;
            }
            ')' => {
                depth -= 1;
                cur.push(ch);
            }
            ',' if depth == 0 => {
                out.push(cur.trim().to_owned());
                cur.clear();
            }
            _ => cur.push(ch),
        }
    }
    out
}

fn check_in(part: &str) -> Option<String> {
    let upper = part.to_ascii_uppercase();
    let at = upper.find("CHECK")?;
    let rest = &part[at + 5..];
    let open = rest.find('(')?;
    let mut depth = 0i32;
    for (i, ch) in rest[open..].char_indices() {
        match ch {
            '(' => depth += 1,
            ')' => {
                depth -= 1;
                if depth == 0 {
                    return Some(rest[open + 1..open + i].trim().to_owned());
                }
            }
            _ => {}
        }
    }
    None
}

fn unquote(s: &str) -> String {
    let s = s.trim();
    let inner = s
        .strip_prefix('"')
        .and_then(|x| x.strip_suffix('"'))
        .or_else(|| s.strip_prefix('`').and_then(|x| x.strip_suffix('`')))
        .or_else(|| s.strip_prefix('[').and_then(|x| x.strip_suffix(']')));
    inner.map_or_else(|| s.to_owned(), |x| x.replace("\"\"", "\""))
}

fn first_name(s: &str) -> String {
    let s = s.trim_start();
    let close = match s.chars().next() {
        Some('"') => '"',
        Some('`') => '`',
        Some('[') => ']',
        _ => return s.split_whitespace().next().map(unquote).unwrap_or_default(),
    };
    let mut end = 1;
    let b: Vec<char> = s.chars().collect();
    while end < b.len() {
        if b[end] == close {
            if close != ']' && b.get(end + 1) == Some(&close) {
                end += 2;
                continue;
            }
            break;
        }
        end += 1;
    }
    unquote(&b[..=end.min(b.len() - 1)].iter().collect::<String>())
}

fn sqlite_more_from(t: &str, sql: &str) -> More {
    let mut m = More::default();
    let mut n = 0;
    for part in parts(sql) {
        let upper = part.to_ascii_uppercase();
        if let Some(body) = check_in(&part) {
            let name = upper
                .starts_with("CONSTRAINT")
                .then(|| first_name(&part["CONSTRAINT".len()..]));
            n += 1;
            m.checks
                .push((name.unwrap_or_else(|| format!("{t}_check_{n}")), body));
        }
        if upper.contains("AUTOINCREMENT") {
            m.identity.push(first_name(&part));
        }
    }
    m
}

async fn sqlite_more(conn: &dyn Connection, t: &str) -> Found<More> {
    let sql = rows(conn, SQLITE_SQL, &[t])
        .await?
        .first()
        .and_then(|r| cell(r, 0))
        .unwrap_or_default();
    let mut m = sqlite_more_from(t, &sql);
    m.indexes = parsed_indexes(rows(conn, SQLITE_INDEXES, &[t]).await?);
    m.triggers = rows(conn, SQLITE_TRIGGERS, &[t])
        .await?
        .iter()
        .filter_map(|r| {
            Some(DesignTrigger {
                name: cell(r, 0)?,
                sql: cell(r, 1)?,
            })
        })
        .collect();
    Ok(m)
}

const FB_COMMENTS: &str = "SELECT TRIM(RDB$FIELD_NAME), RDB$DESCRIPTION FROM RDB$RELATION_FIELDS \
WHERE RDB$RELATION_NAME = ? AND RDB$DESCRIPTION IS NOT NULL \
UNION ALL SELECT '', RDB$DESCRIPTION FROM RDB$RELATIONS WHERE RDB$RELATION_NAME = ? AND RDB$DESCRIPTION IS NOT NULL";

const FB_CHECKS: &str = "SELECT DISTINCT TRIM(c.RDB$CONSTRAINT_NAME), t.RDB$TRIGGER_SOURCE FROM RDB$RELATION_CONSTRAINTS c \
JOIN RDB$CHECK_CONSTRAINTS k ON k.RDB$CONSTRAINT_NAME = c.RDB$CONSTRAINT_NAME \
JOIN RDB$TRIGGERS t ON t.RDB$TRIGGER_NAME = k.RDB$TRIGGER_NAME \
WHERE c.RDB$CONSTRAINT_TYPE = 'CHECK' AND c.RDB$RELATION_NAME = ? AND t.RDB$TRIGGER_TYPE = 1";

const FB_TRIGGERS: &str = "SELECT TRIM(RDB$TRIGGER_NAME), RDB$TRIGGER_SOURCE FROM RDB$TRIGGERS \
WHERE RDB$RELATION_NAME = ? AND COALESCE(RDB$SYSTEM_FLAG, 0) = 0 \
AND RDB$TRIGGER_NAME NOT IN (SELECT RDB$TRIGGER_NAME FROM RDB$CHECK_CONSTRAINTS)";

const FB_INDEXES: &str = "SELECT TRIM(i.RDB$INDEX_NAME), COALESCE(i.RDB$INDEX_TYPE, 0), i.RDB$EXPRESSION_SOURCE, TRIM(s.RDB$FIELD_NAME) \
FROM RDB$INDICES i LEFT JOIN RDB$INDEX_SEGMENTS s ON s.RDB$INDEX_NAME = i.RDB$INDEX_NAME \
WHERE i.RDB$RELATION_NAME = ? AND COALESCE(i.RDB$SYSTEM_FLAG, 0) = 0 ORDER BY 1, s.RDB$FIELD_POSITION";

async fn firebird_more(conn: &dyn Connection, t: &str) -> Found<More> {
    let mut m = More::default();
    comments_from(rows(conn, FB_COMMENTS, &[t, t]).await?, &mut m);
    for r in rows(conn, FB_CHECKS, &[t]).await? {
        if let (Some(name), Some(src)) = (cell(&r, 0), cell(&r, 1)) {
            let body = src.trim();
            let body = body
                .get(..5)
                .filter(|w| w.eq_ignore_ascii_case("CHECK"))
                .map_or(body, |_| body[5..].trim());
            m.checks.push((name, check_body(body)));
        }
    }
    m.triggers = rows(conn, FB_TRIGGERS, &[t])
        .await?
        .iter()
        .filter_map(|r| {
            Some(DesignTrigger {
                name: cell(r, 0)?,
                sql: cell(r, 1)?,
            })
        })
        .collect();
    for r in rows(conn, FB_INDEXES, &[t]).await? {
        let Some(name) = cell(&r, 0) else { continue };
        let desc = cell(&r, 1).as_deref() == Some("1");
        let shape = m.indexes.entry(name).or_default();
        match (cell(&r, 2), cell(&r, 3)) {
            (Some(expr), _) if shape.parts.is_empty() => {
                shape.add(strip_outer(&expr), desc, true);
            }
            (None, Some(col)) => shape.add(col, desc, false),
            _ => {}
        }
    }
    Ok(m)
}

fn strip_outer(expr: &str) -> String {
    let e = expr.trim();
    e.strip_prefix('(')
        .and_then(|x| x.strip_suffix(')'))
        .map_or(e, str::trim)
        .to_owned()
}

async fn more(conn: &dyn Connection, ns: &str, t: &str) -> Found<More> {
    match Engine::of(conn) {
        Engine::Postgres => pg_more(conn, ns, t, false).await,
        Engine::Cockroach => pg_more(conn, ns, t, true).await,
        Engine::Mysql | Engine::Mariadb => mysql_more(conn, ns, t).await,
        Engine::Mssql => mssql_more(conn, ns, t).await,
        Engine::Sqlite | Engine::Libsql => sqlite_more(conn, t).await,
        Engine::Duckdb => duckdb_more(conn, ns, t).await,
        Engine::Clickhouse => Err(DriverError::query("ClickHouse tables are designed in SQL")),
        Engine::Oracle => Err(DriverError::query("Oracle tables are designed in SQL")),
        Engine::Firebird => firebird_more(conn, t).await,
        Engine::Hana | Engine::Db2 | Engine::Odbc => {
            Err(DriverError::query("these tables are designed in SQL"))
        }
        Engine::Mongo => Err(DriverError::query(
            "MongoDB collections have no table design",
        )),
    }
}

fn shaped(
    name: String,
    columns: Vec<String>,
    unique: bool,
    shape: Option<&IndexShape>,
) -> DesignIndex {
    let Some(shape) = shape.filter(|s| !s.parts.is_empty()) else {
        return DesignIndex {
            origin: Some(name.clone()),
            name,
            columns,
            unique,
            ..DesignIndex::default()
        };
    };
    let flag = |f: fn(&(String, bool, bool)) -> bool| {
        let v: Vec<bool> = shape.parts.iter().map(f).collect();
        if v.iter().any(|x| *x) { v } else { Vec::new() }
    };
    DesignIndex {
        origin: Some(name.clone()),
        name,
        columns: shape.parts.iter().map(|p| p.0.clone()).collect(),
        unique,
        descending: flag(|p| p.1),
        expressions: flag(|p| p.2),
        predicate: shape.predicate.clone(),
    }
}

fn design_of(
    rel: &Relation,
    ns: &str,
    x: &super::compare::Extra,
    m: More,
    dialect: Dialect,
) -> TableDesign {
    let t = table_with(rel, ns, x);
    let identity_default =
        |d: &Option<String>| d.as_deref().is_some_and(|d| d.starts_with("GENERATED"));
    let columns = t
        .columns
        .iter()
        .map(|c| {
            let serial =
                dialect == Dialect::Postgres && c.generated && !identity_default(&c.default);
            let identity = (c.generated && !serial) || m.identity.contains(&c.name);
            DesignColumn {
                origin: Some(c.name.clone()),
                name: c.name.clone(),
                type_name: c.type_name.clone(),
                nullable: c.nullable,
                default: if identity_default(&c.default) {
                    None
                } else {
                    c.default.clone()
                },
                identity,
                comment: m.comments.get(&c.name).cloned(),
                default_name: c.default_name.clone(),
            }
        })
        .collect();
    let (pk_name, primary_key) = match m.pk {
        Some((name, cols)) => (
            name.or(t.pk_name),
            cols.into_iter()
                .filter(|c| t.columns.iter().any(|x| &x.name == c))
                .collect(),
        ),
        None => (t.pk_name, t.primary_key.clone()),
    };
    let mut indexes: Vec<DesignIndex> = t
        .indexes
        .iter()
        .map(|i| {
            shaped(
                i.name.clone(),
                i.columns.clone(),
                i.unique,
                m.indexes.get(&i.name),
            )
        })
        .collect();
    if dialect == Dialect::Sqlite {
        indexes.extend(
            rel.indexes
                .iter()
                .filter(|i| i.name.starts_with("sqlite_autoindex") && i.columns != primary_key)
                .map(|i| shaped(i.name.clone(), i.columns.clone(), true, None)),
        );
    }
    let foreign_keys = rel
        .foreign_keys
        .iter()
        .map(|f| DesignForeignKey {
            origin: Some(f.name.clone()),
            name: f.name.clone(),
            columns: f.columns.clone(),
            ref_namespace: if f.ref_namespace == ns {
                String::new()
            } else {
                f.ref_namespace.clone()
            },
            ref_table: f.ref_table.clone(),
            ref_columns: f.ref_columns.clone(),
            on_delete: f.on_delete,
            on_update: f.on_update,
        })
        .collect();
    TableDesign {
        namespace: ns.to_owned(),
        name: rel.name.clone(),
        comment: m.table_comment,
        columns,
        primary_key,
        pk_name,
        indexes,
        foreign_keys,
        checks: m
            .checks
            .into_iter()
            .map(|(name, expression)| DesignCheck {
                origin: Some(name.clone()),
                name,
                expression,
            })
            .collect(),
        triggers: m.triggers,
    }
}

async fn load(conn: &dyn Connection, ns: &str, name: &str) -> Found<Option<TableDesign>> {
    let Some(rel) = conn.relation(ns, name).await? else {
        return Ok(None);
    };
    let x = columns_extra(conn, ns, name).await?;
    let m = more(conn, ns, name).await?;
    Ok(Some(design_of(&rel, ns, &x, m, conn.sql()?)))
}

impl Core {
    pub async fn table_design(
        &self,
        session_id: &str,
        namespace: &str,
        name: &str,
    ) -> Result<TableDesign> {
        let session = self.live(session_id).await?;
        self.authorize(&session, Access::Read)?;
        let side = self.side(&session).await?;
        let conn = side.conn()?;
        let found = load(conn, namespace, name).await;
        side.check(found)?
            .ok_or_else(|| CoreError::NotFound(name.to_owned()))
    }
}
