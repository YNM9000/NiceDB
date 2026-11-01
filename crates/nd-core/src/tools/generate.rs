use crate::session::Sql as _;
use std::collections::{HashMap, HashSet};
use std::time::Instant;

use nd_driver::{Connection, Dialect, DriverError, Relation, Row, Schema};
use nd_gen::{Check, ColumnGen, DataLocale, Facts, Gen, GenError, Generator, Rule, Shape};
use nd_sql::TableRef;
use nd_transfer::{Record, Unit};
use serde::{Deserialize, Serialize};

use super::transfer::{Inserter, Plan, qualified};
use super::{Engine, Pace, Progress, ToolFault, ms};
use crate::guard::{Access, collect, quiet};
use crate::{Core, CoreError, Result};

const POOL: u32 = 5000;
pub const GEN_ROWS_MAX: u32 = 1_000_000;
const PREVIEW_ROWS: u32 = 20;
const BATCH: usize = 1000;
const TAKEN_MAX: u32 = 200_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "snake_case")]
pub enum RuleKind {
    Column,
    Row,
    Server,
}

#[derive(Debug, Clone, Serialize, specta::Type)]
pub struct GenRule {
    pub expr: String,
    pub kind: RuleKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "snake_case")]
pub enum AutoKind {
    Identity,
    Computed,
}

#[derive(Debug, Clone, Serialize, specta::Type)]
pub struct GenParentRef {
    pub fk: u32,
    pub namespace: String,
    pub table: String,
    pub column: String,
}

#[derive(Debug, Clone, Serialize, specta::Type)]
pub struct FakeColumn {
    pub name: String,
    pub type_name: String,
    pub nullable: bool,
    pub primary_key: bool,
    pub unique: bool,
    pub auto: Option<AutoKind>,
    pub default: Option<String>,
    pub check: Option<Check>,
    pub parent: Option<GenParentRef>,
    pub shape: Shape,
    pub suggested: ColumnGen,
}

#[derive(Debug, Clone, Serialize, specta::Type)]
pub struct GenForeignKey {
    pub namespace: String,
    pub table: String,
    pub columns: Vec<String>,
    pub ref_columns: Vec<String>,
    #[specta(type = Option<f64>)]
    pub parent_rows: Option<u64>,
    pub own: bool,
}

#[derive(Debug, Clone, Serialize, specta::Type)]
pub struct GenProfile {
    pub namespace: String,
    pub name: String,
    pub columns: Vec<FakeColumn>,
    pub foreign_keys: Vec<GenForeignKey>,
    pub unique: Vec<Vec<String>>,
    pub ancestors: Vec<TableRef>,
    pub rules: Vec<GenRule>,
    pub estimated_rows: Option<f64>,
}

#[derive(Debug, Clone, Deserialize, specta::Type)]
pub struct FakeRequest {
    pub job_id: String,
    pub session_id: String,
    pub namespace: String,
    pub name: String,
    pub columns: Vec<ColumnGen>,
    pub rows: u32,
    pub seed: u32,
    pub parents: Option<u32>,
    #[serde(default)]
    pub locale: DataLocale,
}

#[derive(Debug, Clone, Serialize, specta::Type)]
pub struct GenPreview {
    pub columns: Vec<String>,
    pub rows: Vec<Row>,
    pub waiting: Vec<String>,
    pub fault: Option<GenFault>,
}

#[derive(Debug, Clone, Serialize, specta::Type)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum GenFault {
    EmptyParent {
        table: String,
    },
    NoUnique {
        columns: Vec<String>,
    },
    Batch {
        table: String,
        #[specta(type = f64)]
        from: u64,
        #[specta(type = f64)]
        to: u64,
        message: String,
    },
    Row {
        table: String,
        #[specta(type = f64)]
        row: u64,
        message: String,
    },
    Expr {
        column: String,
        message: String,
    },
    Check {
        expr: String,
    },
    Server {
        message: String,
    },
}

#[derive(Debug, Clone, Serialize, specta::Type)]
pub struct GenWritten {
    pub namespace: String,
    pub name: String,
    #[specta(type = f64)]
    pub rows: u64,
}

#[derive(Debug, Clone, Serialize, specta::Type)]
pub struct GenDone {
    pub tables: Vec<GenWritten>,
    #[specta(type = f64)]
    pub inserted: u64,
    pub committed: bool,
    pub kept: bool,
    pub cancelled: bool,
    pub fault: Option<GenFault>,
    #[specta(type = f64)]
    pub elapsed_ms: u64,
}

#[derive(Debug, Default, Clone)]
struct ColumnFacts {
    default: Option<String>,
    auto: Option<AutoKind>,
    values: Option<Vec<String>>,
}

#[derive(Debug, Default)]
struct Catalog {
    columns: HashMap<String, ColumnFacts>,
    checks: Vec<String>,
}

const PG_COLUMNS: &str = "SELECT column_name, column_default, is_identity, is_generated \
FROM information_schema.columns WHERE table_schema = $1 AND table_name = $2";
const PG_ENUMS: &str = "SELECT a.attname, e.enumlabel FROM pg_attribute a \
JOIN pg_class c ON c.oid = a.attrelid JOIN pg_namespace n ON n.oid = c.relnamespace \
JOIN pg_enum e ON e.enumtypid = a.atttypid \
WHERE n.nspname = $1 AND c.relname = $2 AND a.attnum > 0 ORDER BY a.attnum, e.enumsortorder";
const PG_CHECKS: &str = "SELECT pg_get_constraintdef(k.oid) FROM pg_constraint k \
JOIN pg_class c ON c.oid = k.conrelid JOIN pg_namespace n ON n.oid = c.relnamespace \
WHERE n.nspname = $1 AND c.relname = $2 AND k.contype = 'c'";
const MY_COLUMNS: &str = "SELECT COLUMN_NAME, COLUMN_DEFAULT, EXTRA, '' FROM information_schema.COLUMNS \
WHERE TABLE_SCHEMA = ? AND TABLE_NAME = ?";
const MY_CHECKS: &str = "SELECT cc.CHECK_CLAUSE FROM information_schema.CHECK_CONSTRAINTS cc \
JOIN information_schema.TABLE_CONSTRAINTS tc ON tc.CONSTRAINT_SCHEMA = cc.CONSTRAINT_SCHEMA \
AND tc.CONSTRAINT_NAME = cc.CONSTRAINT_NAME AND tc.CONSTRAINT_TYPE = 'CHECK' \
WHERE tc.TABLE_SCHEMA = ? AND tc.TABLE_NAME = ?";
const MARIA_CHECKS: &str = "SELECT CHECK_CLAUSE FROM information_schema.CHECK_CONSTRAINTS \
WHERE CONSTRAINT_SCHEMA = ? AND TABLE_NAME = ?";
const MS_COLUMNS: &str = "SELECT c.name, dc.definition, \
CASE WHEN c.is_identity = 1 THEN 'YES' ELSE 'NO' END, \
CASE WHEN c.is_computed = 1 OR ty.name IN ('timestamp', 'rowversion') THEN 'ALWAYS' ELSE 'NEVER' END \
FROM sys.columns c JOIN sys.tables t ON t.object_id = c.object_id \
JOIN sys.schemas s ON s.schema_id = t.schema_id JOIN sys.types ty ON ty.user_type_id = c.user_type_id \
LEFT JOIN sys.default_constraints dc ON dc.parent_object_id = c.object_id AND dc.parent_column_id = c.column_id \
WHERE s.name = @P1 AND t.name = @P2";
const MS_CHECKS: &str = "SELECT cc.definition FROM sys.check_constraints cc \
JOIN sys.tables t ON t.object_id = cc.parent_object_id JOIN sys.schemas s ON s.schema_id = t.schema_id \
WHERE s.name = @P1 AND t.name = @P2";
const ORA_COLUMNS: &str = "SELECT column_name, data_default, identity_column, \
CASE virtual_column WHEN 'YES' THEN 'ALWAYS' ELSE 'NEVER' END FROM all_tab_cols \
WHERE owner = :1 AND table_name = :2 AND hidden_column = 'NO'";
const ORA_CHECKS: &str = "SELECT search_condition_vc FROM all_constraints \
WHERE owner = :1 AND table_name = :2 AND constraint_type = 'C' AND search_condition_vc NOT LIKE '%IS NOT NULL'";
const LITE_COLUMNS: &str = "SELECT name, dflt_value, CAST(pk AS TEXT), CAST(hidden AS TEXT), type \
FROM pragma_table_xinfo(?)";
const LITE_SQL: &str = "SELECT sql FROM sqlite_schema WHERE type = 'table' AND name = ?";
const DUCK_COLUMNS: &str = "SELECT column_name, column_default, 'NO', 'NEVER' FROM duckdb_columns() \
WHERE database_name = current_database() AND schema_name = ? AND table_name = ?";
const DUCK_CHECKS: &str = "SELECT expression FROM duckdb_constraints() \
WHERE database_name = current_database() AND schema_name = ? AND table_name = ? AND constraint_type = 'CHECK'";

async fn rows_of(conn: &dyn Connection, sql: &str, params: &[Option<String>]) -> Vec<Row> {
    collect(conn, sql, Some(params))
        .await
        .map(|(rows, _)| rows)
        .unwrap_or_default()
}

fn cell(r: &Row, i: usize) -> Option<String> {
    r.get(i).cloned().flatten()
}

fn sqlite_checks(sql: &str) -> Vec<String> {
    let chars: Vec<(usize, char)> = sql.char_indices().collect();
    let mut out = Vec::new();
    let mut quote: Option<char> = None;
    let mut depth = 0usize;
    let mut open: Option<(usize, usize)> = None;
    let mut armed = false;
    let mut word = String::new();
    for &(at, c) in &chars {
        if let Some(q) = quote {
            if c == q {
                quote = None;
            }
            continue;
        }
        if c.is_alphanumeric() || c == '_' {
            word.push(c);
            continue;
        }
        if !word.is_empty() {
            armed = open.is_none() && word.eq_ignore_ascii_case("check");
            word.clear();
        }
        match c {
            '\'' | '"' | '`' => quote = Some(c),
            '(' => {
                depth += 1;
                if armed {
                    open = Some((at + 1, depth));
                }
                armed = false;
            }
            ')' => {
                if let Some((start, level)) = open
                    && level == depth
                {
                    out.push(sql[start..at].to_owned());
                    open = None;
                }
                depth = depth.saturating_sub(1);
            }
            c if c.is_whitespace() => {}
            _ => armed = false,
        }
    }
    out
}

fn add_columns(cat: &mut Catalog, rows: Vec<Row>) {
    for r in rows {
        let Some(name) = cell(&r, 0) else { continue };
        let default = cell(&r, 1).filter(|d| !d.trim().is_empty() && d.trim() != "NULL");
        let identity = cell(&r, 2).is_some_and(|v| v.eq_ignore_ascii_case("yes"));
        let extra = cell(&r, 2).unwrap_or_default().to_ascii_lowercase();
        let generated = cell(&r, 3).is_some_and(|v| v.eq_ignore_ascii_case("always"))
            || (extra.contains("generated") && !extra.contains("default_generated"));
        let serial = default.as_deref().is_some_and(|d| {
            let d = d.to_ascii_lowercase();
            d.contains("nextval(") || d.contains("unique_rowid()")
        });
        let auto = if generated {
            Some(AutoKind::Computed)
        } else if identity || serial || extra.contains("auto_increment") {
            Some(AutoKind::Identity)
        } else {
            None
        };
        cat.columns.insert(
            name,
            ColumnFacts {
                default,
                auto,
                values: None,
            },
        );
    }
}

async fn catalog(conn: &dyn Connection, namespace: &str, name: &str) -> Catalog {
    let mut cat = Catalog::default();
    let both = [Some(namespace.to_owned()), Some(name.to_owned())];
    let checks = |rows: Vec<Row>| rows.iter().filter_map(|r| cell(r, 0)).collect::<Vec<_>>();
    match Engine::of(conn) {
        Engine::Postgres | Engine::Cockroach => {
            add_columns(&mut cat, rows_of(conn, PG_COLUMNS, &both).await);
            for r in rows_of(conn, PG_ENUMS, &both).await {
                if let (Some(col), Some(label)) = (cell(&r, 0), cell(&r, 1)) {
                    let f = cat.columns.entry(col).or_default();
                    f.values.get_or_insert_with(Vec::new).push(label);
                }
            }
            cat.checks = checks(rows_of(conn, PG_CHECKS, &both).await);
        }
        engine @ (Engine::Mysql | Engine::Mariadb) => {
            add_columns(&mut cat, rows_of(conn, MY_COLUMNS, &both).await);
            let sql = if engine == Engine::Mariadb {
                MARIA_CHECKS
            } else {
                MY_CHECKS
            };
            cat.checks = checks(rows_of(conn, sql, &both).await);
        }
        Engine::Mssql => {
            add_columns(&mut cat, rows_of(conn, MS_COLUMNS, &both).await);
            cat.checks = checks(rows_of(conn, MS_CHECKS, &both).await);
        }
        Engine::Sqlite | Engine::Libsql => {
            let one = [Some(name.to_owned())];
            let rows = rows_of(conn, LITE_COLUMNS, &one).await;
            let pk = rows
                .iter()
                .filter(|r| cell(r, 2).is_some_and(|p| p != "0"))
                .count();
            for r in rows {
                let Some(col) = cell(&r, 0) else { continue };
                let hidden = cell(&r, 3).is_some_and(|h| h == "2" || h == "3");
                let rowid = pk == 1
                    && cell(&r, 2).is_some_and(|p| p == "1")
                    && cell(&r, 4).is_some_and(|t| t.trim().eq_ignore_ascii_case("integer"));
                let auto = if hidden {
                    Some(AutoKind::Computed)
                } else if rowid {
                    Some(AutoKind::Identity)
                } else {
                    None
                };
                let default = cell(&r, 1).filter(|d| !d.eq_ignore_ascii_case("null"));
                cat.columns.insert(
                    col,
                    ColumnFacts {
                        default,
                        auto,
                        values: None,
                    },
                );
            }
            if let Some(sql) = rows_of(conn, LITE_SQL, &one)
                .await
                .first()
                .and_then(|r| cell(r, 0))
            {
                cat.checks = sqlite_checks(&sql);
            }
        }
        Engine::Duckdb => {
            add_columns(&mut cat, rows_of(conn, DUCK_COLUMNS, &both).await);
            cat.checks = checks(rows_of(conn, DUCK_CHECKS, &both).await);
        }
        Engine::Oracle => {
            add_columns(&mut cat, rows_of(conn, ORA_COLUMNS, &both).await);
            cat.checks = checks(rows_of(conn, ORA_CHECKS, &both).await);
        }
        Engine::Clickhouse
        | Engine::Mongo
        | Engine::Firebird
        | Engine::Hana
        | Engine::Db2
        | Engine::Odbc => {}
    }
    cat
}

fn relation<'a>(schema: &'a Schema, namespace: &str, name: &str) -> Option<&'a Relation> {
    schema
        .namespaces
        .iter()
        .filter(|n| namespace.is_empty() || n.name == namespace)
        .flat_map(|n| &n.tables)
        .find(|t| t.name == name)
}

fn namespace_of(schema: &Schema, namespace: &str, name: &str) -> String {
    schema
        .namespaces
        .iter()
        .find(|n| {
            (namespace.is_empty() || n.name == namespace) && n.tables.iter().any(|t| t.name == name)
        })
        .map_or_else(|| namespace.to_owned(), |n| n.name.clone())
}

fn unique_sets(rel: &Relation) -> Vec<Vec<String>> {
    let pk: Vec<String> = rel
        .columns
        .iter()
        .filter(|c| c.primary_key)
        .map(|c| c.name.clone())
        .collect();
    let mut sets: Vec<Vec<String>> = Vec::new();
    for set in std::iter::once(pk).chain(
        rel.indexes
            .iter()
            .filter(|i| i.unique)
            .map(|i| i.columns.clone()),
    ) {
        if !set.is_empty() && !sets.contains(&set) {
            sets.push(set);
        }
    }
    sets
}

fn limited(dialect: Dialect, select: &str, from: &str, order: &str, n: u32) -> String {
    match dialect {
        Dialect::Mssql => format!("SELECT DISTINCT TOP {n} {select} FROM {from} ORDER BY {order}"),
        _ => format!("SELECT DISTINCT {select} FROM {from} ORDER BY {order} LIMIT {n}"),
    }
}

async fn pool(
    conn: &dyn Connection,
    namespace: &str,
    fk: &nd_driver::ForeignKey,
) -> Result<Vec<Vec<String>>> {
    let d = conn.sql()?;
    let cols: Vec<String> = fk.ref_columns.iter().map(|c| nd_sql::ident(d, c)).collect();
    let list = cols.join(", ");
    let from = qualified(
        d,
        if fk.ref_namespace.is_empty() {
            namespace
        } else {
            &fk.ref_namespace
        },
        &fk.ref_table,
    );
    let filter = cols
        .iter()
        .map(|c| format!("{c} IS NOT NULL"))
        .collect::<Vec<_>>()
        .join(" AND ");
    let sql = limited(d, &list, &format!("{from} WHERE {filter}"), &list, POOL);
    let (rows, _) = collect(conn, &sql, None).await?;
    Ok(rows
        .into_iter()
        .filter_map(|r| r.into_iter().collect::<Option<Vec<String>>>())
        .collect())
}

async fn max_of(conn: &dyn Connection, table: &str, column: &str) -> Option<i64> {
    let sql = format!(
        "SELECT MAX({}) FROM {table}",
        nd_sql::ident(conn.dialect()?, column)
    );
    let rows = collect(conn, &sql, None).await.ok()?.0;
    let v = rows.first().and_then(|r| cell(r, 0))?;
    v.trim().parse::<i64>().ok().map(|n| n.saturating_add(1))
}

fn ancestors(schema: &Schema, namespace: &str, name: &str) -> Vec<TableRef> {
    fn visit(
        schema: &Schema,
        ns: &str,
        name: &str,
        seen: &mut HashSet<(String, String)>,
        out: &mut Vec<TableRef>,
    ) {
        let Some(rel) = relation(schema, ns, name) else {
            return;
        };
        for fk in &rel.foreign_keys {
            let pns = if fk.ref_namespace.is_empty() {
                ns.to_owned()
            } else {
                fk.ref_namespace.clone()
            };
            let key = (pns.clone(), fk.ref_table.clone());
            if seen.insert(key) {
                visit(schema, &pns, &fk.ref_table, seen, out);
                out.push(TableRef {
                    namespace: pns,
                    name: fk.ref_table.clone(),
                });
            }
        }
    }
    let mut seen = HashSet::from([(namespace.to_owned(), name.to_owned())]);
    let mut out = Vec::new();
    visit(schema, namespace, name, &mut seen, &mut out);
    out
}

#[allow(clippy::too_many_lines)]
async fn profile_of(
    conn: &dyn Connection,
    schema: &Schema,
    namespace: &str,
    name: &str,
) -> Result<GenProfile> {
    let rel =
        relation(schema, namespace, name).ok_or_else(|| CoreError::NotFound(name.to_owned()))?;
    let namespace = namespace_of(schema, namespace, name);
    let cat = catalog(conn, &namespace, name).await;
    let table = qualified(conn.sql()?, &namespace, name);
    let unique = unique_sets(rel);
    let mut foreign_keys = Vec::new();
    for fk in &rel.foreign_keys {
        let pns = if fk.ref_namespace.is_empty() {
            namespace.clone()
        } else {
            fk.ref_namespace.clone()
        };
        let parent_rows = relation(schema, &pns, &fk.ref_table)
            .and_then(|r| r.estimated_rows)
            .map(|n| {
                #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
                let n = n.max(0.0) as u64;
                n
            });
        foreign_keys.push(GenForeignKey {
            own: pns == namespace && fk.ref_table == name,
            namespace: pns,
            table: fk.ref_table.clone(),
            columns: fk.columns.clone(),
            ref_columns: fk.ref_columns.clone(),
            parent_rows,
        });
    }
    let mut columns = Vec::new();
    for c in &rel.columns {
        let facts = cat.columns.get(&c.name).cloned().unwrap_or_default();
        let check = facts
            .values
            .clone()
            .map(|values| Check::Values { values })
            .or_else(|| {
                cat.checks
                    .iter()
                    .find_map(|e| nd_gen::parse_check(e, &c.name))
            });
        let parent = foreign_keys.iter().enumerate().find_map(|(i, fk)| {
            let at = fk.columns.iter().position(|x| x == &c.name)?;
            Some(GenParentRef {
                fk: u32::try_from(i).unwrap_or(0),
                namespace: fk.namespace.clone(),
                table: fk.table.clone(),
                column: fk.ref_columns.get(at).cloned().unwrap_or_default(),
            })
        });
        let alone = unique.iter().any(|s| s.len() == 1 && s[0] == c.name);
        let shape = nd_gen::shape(&c.type_name);
        let next = if alone
            && facts.auto.is_none()
            && parent.is_none()
            && matches!(shape, Shape::Int { .. })
        {
            max_of(conn, &table, &c.name).await.or(Some(1))
        } else {
            None
        };
        let f = Facts {
            table: name.to_owned(),
            name: c.name.clone(),
            type_name: c.type_name.clone(),
            nullable: c.nullable,
            unique: alone,
            auto: facts.auto.is_some(),
            has_default: facts.default.is_some(),
            check: check.clone(),
            parent: parent.as_ref().map(|p| {
                let pos = foreign_keys[p.fk as usize]
                    .columns
                    .iter()
                    .position(|x| x == &c.name)
                    .unwrap_or(0);
                (p.fk, u32::try_from(pos).unwrap_or(0))
            }),
            next,
        };
        columns.push(FakeColumn {
            name: c.name.clone(),
            type_name: c.type_name.clone(),
            nullable: c.nullable,
            primary_key: c.primary_key,
            unique: alone,
            auto: facts.auto,
            default: facts.default,
            check,
            parent,
            shape,
            suggested: nd_gen::guess(&f),
        });
    }
    let written: Vec<String> = columns
        .iter()
        .filter(|c| c.auto.is_none())
        .map(|c| c.name.clone())
        .collect();
    let rules = cat
        .checks
        .iter()
        .map(|e| {
            let kind = if rel
                .columns
                .iter()
                .any(|c| nd_gen::parse_check(e, &c.name).is_some())
            {
                RuleKind::Column
            } else if Rule::compile(e, &written).is_some() {
                RuleKind::Row
            } else {
                RuleKind::Server
            };
            GenRule {
                expr: e.clone(),
                kind,
            }
        })
        .collect();
    Ok(GenProfile {
        rules,
        ancestors: ancestors(schema, &namespace, name),
        namespace,
        name: name.to_owned(),
        columns,
        foreign_keys,
        unique,
        estimated_rows: rel.estimated_rows,
    })
}

fn seed_for(seed: u32, table: &str) -> u64 {
    table.bytes().fold(u64::from(seed), |h, b| {
        (h ^ u64::from(b)).wrapping_mul(0x0100_0000_01B3)
    })
}

fn sanitized(profile: &GenProfile, columns: &[ColumnGen]) -> Vec<ColumnGen> {
    profile
        .columns
        .iter()
        .map(|c| {
            if c.auto.is_some() {
                return ColumnGen {
                    generator: Gen::Auto,
                    ..c.suggested.clone()
                };
            }
            columns.iter().find(|x| x.name == c.name).map_or_else(
                || c.suggested.clone(),
                |a| ColumnGen {
                    null_ratio: a.null_ratio.clamp(0.0, 1.0),
                    ..a.clone()
                },
            )
        })
        .collect()
}

async fn pools(
    conn: &dyn Connection,
    schema: &Schema,
    profile: &GenProfile,
) -> Result<Vec<Vec<Vec<String>>>> {
    let rel = relation(schema, &profile.namespace, &profile.name)
        .ok_or_else(|| CoreError::NotFound(profile.name.clone()))?;
    let mut out = Vec::new();
    for fk in &rel.foreign_keys {
        out.push(pool(conn, &profile.namespace, fk).await?);
    }
    Ok(out)
}

fn fault_of(profile: &GenProfile, e: &GenError) -> GenFault {
    match e {
        GenError::EmptyPool { fk } => GenFault::EmptyParent {
            table: profile
                .foreign_keys
                .get(*fk as usize)
                .map(|f| f.table.clone())
                .unwrap_or_default(),
        },
        GenError::NoUnique { columns } => GenFault::NoUnique {
            columns: columns.clone(),
        },
        GenError::BadExpr { column, message } => GenFault::Expr {
            column: column.clone(),
            message: message.clone(),
        },
        GenError::Check { expr } => GenFault::Check { expr: expr.clone() },
    }
}

fn this_year() -> i32 {
    let days = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs() / 86_400);
    nd_gen::format_date(i64::try_from(days).unwrap_or(0))
        .get(..4)
        .and_then(|y| y.parse().ok())
        .unwrap_or(2026)
}

struct Step {
    profile: GenProfile,
    columns: Vec<ColumnGen>,
    rows: u32,
    seed: u64,
    locale: DataLocale,
}

fn rules_for(profile: &GenProfile, g: Generator) -> Generator {
    let names = g.columns();
    let rules = profile
        .rules
        .iter()
        .filter(|r| r.kind != RuleKind::Server)
        .filter_map(|r| Rule::compile(&r.expr, &names))
        .collect();
    g.rules(rules)
}

async fn taken(
    conn: &dyn Connection,
    dialect: Dialect,
    table: &str,
    unique: &[Vec<String>],
    g: &mut Generator,
) {
    let names = g.columns();
    for set in unique
        .iter()
        .filter(|s| s.iter().all(|c| names.contains(c)))
    {
        let cols = set
            .iter()
            .map(|c| nd_sql::ident(dialect, c))
            .collect::<Vec<_>>()
            .join(", ");
        let sql = match dialect {
            Dialect::Mssql => format!("SELECT DISTINCT TOP {TAKEN_MAX} {cols} FROM {table}"),
            Dialect::Firebird => format!("SELECT DISTINCT FIRST {TAKEN_MAX} {cols} FROM {table}"),
            _ => format!("SELECT DISTINCT {cols} FROM {table} LIMIT {TAKEN_MAX}"),
        };
        match collect(conn, &sql, None).await {
            Ok((rows, _)) => g.taken(set, rows),
            Err(e) => tracing::debug!(error = %e, "existing unique values not read"),
        }
    }
}

impl Step {
    fn generator(&self, pools: Vec<Vec<Vec<String>>>) -> Generator {
        let g = Generator::new(self.seed, &self.columns, &self.profile.unique, pools)
            .locale(self.locale)
            .year(this_year());
        rules_for(&self.profile, g)
    }
}

struct Writer<'a> {
    conn: &'a dyn Connection,
    job: &'a super::Job,
    schema: &'a Schema,
    total: u64,
    pace: Pace,
    progress: &'a (dyn Fn(Progress) + Send + Sync),
    done: GenDone,
    error: Option<String>,
    script: Vec<String>,
}

async fn settle(conn: &dyn Connection, atomic: bool, done: &mut GenDone, script: &mut Vec<String>) {
    let ok = !done.cancelled && done.fault.is_none();
    if !atomic {
        done.committed = ok;
        return;
    }
    done.committed = ok && quiet(conn, "COMMIT").await.is_ok();
    if !done.committed {
        let _ = quiet(conn, "ROLLBACK").await;
        done.inserted = 0;
        for t in &mut done.tables {
            t.rows = 0;
        }
    }
    script.push(if done.committed { "COMMIT" } else { "ROLLBACK" }.into());
}

fn keep(done: &mut GenDone, p: &GenProfile, rows: u64) {
    if done.kept && rows > 0 {
        done.tables.push(GenWritten {
            namespace: p.namespace.clone(),
            name: p.name.clone(),
            rows,
        });
    }
}

impl Writer<'_> {
    fn fail(&mut self, fault: GenFault, message: String) -> bool {
        self.done.fault = Some(fault);
        self.error = Some(message);
        false
    }

    #[allow(clippy::too_many_lines)]
    async fn step(&mut self, step: &Step) -> bool {
        let p = &step.profile;
        let (conn, Some(dialect)) = (self.conn, self.conn.dialect()) else {
            return self.fail(
                GenFault::Server {
                    message: crate::CoreError::NotSql.to_string(),
                },
                crate::CoreError::NotSql.to_string(),
            );
        };
        let table = qualified(dialect, &p.namespace, &p.name);
        let pools = match pools(conn, self.schema, p).await {
            Ok(pl) => pl,
            Err(e) => {
                return self.fail(
                    GenFault::Server {
                        message: e.to_string(),
                    },
                    e.to_string(),
                );
            }
        };
        let mut g = step.generator(pools);
        taken(conn, dialect, &table, &p.unique, &mut g).await;
        let names = g.columns();
        let types = names
            .iter()
            .map(|n| {
                p.columns
                    .iter()
                    .find(|c| &c.name == n)
                    .map(|c| c.type_name.clone())
            })
            .collect();
        let plan = Plan::direct(dialect, table.clone(), names, types);
        let mut ins = if self.done.kept {
            Inserter::batches(conn, plan)
        } else {
            Inserter::stopping(conn, plan)
        };
        self.script
            .push(format!("-- {} × INSERT INTO {table}", step.rows));
        let mut made: u64 = 0;
        while made < u64::from(step.rows) {
            if self.job.stopped() {
                self.done.cancelled = true;
                keep(&mut self.done, p, made);
                return false;
            }
            let n = (u64::from(step.rows) - made).min(BATCH as u64);
            let mut records = Vec::with_capacity(BATCH);
            for i in 0..n {
                match g.next_row() {
                    Ok(values) => records.push(Record {
                        line: made + i + 1,
                        values,
                    }),
                    Err(e) => return self.fail(fault_of(p, &e), e.to_string()),
                }
            }
            let result = ins.rows(&records).await;
            let partial = ins.inserted - made;
            if result.is_err() || ins.failed {
                self.done.inserted += partial;
                keep(&mut self.done, p, ins.inserted);
            }
            match result {
                Err(DriverError::Cancelled) => {
                    self.done.cancelled = true;
                    return false;
                }
                Err(e) => {
                    return self.fail(
                        GenFault::Server {
                            message: e.to_string(),
                        },
                        e.to_string(),
                    );
                }
                Ok(()) if ins.failed => {
                    let f = ins.faults.first();
                    let message = f.map(|f| f.message.clone()).unwrap_or_default();
                    let row = f.map_or(made + 1, |f| f.line);
                    let fault = match ins.refused {
                        Some((from, to)) => GenFault::Batch {
                            table: p.name.clone(),
                            from,
                            to,
                            message: message.clone(),
                        },
                        None => GenFault::Row {
                            table: p.name.clone(),
                            row,
                            message: message.clone(),
                        },
                    };
                    return self.fail(fault, message);
                }
                Ok(()) => {}
            }
            made += n;
            self.done.inserted += n;
            if self.pace.due() {
                (self.progress)(Progress {
                    done: self.done.inserted,
                    total: Some(self.total),
                    unit: Unit::Rows,
                    rows: self.done.inserted,
                    bytes: 0,
                    stage: Some(p.name.clone()),
                });
            }
        }
        self.done.tables.push(GenWritten {
            namespace: p.namespace.clone(),
            name: p.name.clone(),
            rows: made,
        });
        true
    }
}

impl Core {
    pub async fn gen_profile(
        &self,
        session_id: &str,
        namespace: &str,
        name: &str,
    ) -> Result<GenProfile> {
        let session = self.live(session_id).await?;
        self.authorize(&session, Access::Read)?;
        let side = self.side(&session).await?;
        let conn = side.conn()?;
        let schema = side.check(conn.schema().await)?;
        profile_of(conn, &schema, namespace, name).await
    }

    pub async fn gen_preview(&self, req: FakeRequest) -> Result<GenPreview> {
        let session = self.live(&req.session_id).await?;
        self.authorize(&session, Access::Read)?;
        let side = self.side(&session).await?;
        let conn = side.conn()?;
        let schema = side.check(conn.schema().await)?;
        let profile = profile_of(conn, &schema, &req.namespace, &req.name).await?;
        let mut columns = sanitized(&profile, &req.columns);
        let pools = pools(conn, &schema, &profile).await?;
        let mut waiting = Vec::new();
        if req.parents.is_some() {
            for c in &mut columns {
                if let Gen::Parent { fk, .. } = c.generator
                    && pools.get(fk as usize).is_none_or(Vec::is_empty)
                {
                    c.generator = Gen::Null;
                    if let Some(f) = profile.foreign_keys.get(fk as usize)
                        && !waiting.contains(&f.table)
                    {
                        waiting.push(f.table.clone());
                    }
                }
            }
        }
        let mut g = rules_for(
            &profile,
            Generator::new(
                seed_for(req.seed, &profile.name),
                &columns,
                &profile.unique,
                pools,
            )
            .locale(req.locale)
            .year(this_year()),
        );
        let mut rows = Vec::new();
        let mut fault = None;
        for _ in 0..req.rows.min(PREVIEW_ROWS) {
            match g.next_row() {
                Ok(r) => rows.push(r),
                Err(e) => {
                    fault = Some(fault_of(&profile, &e));
                    break;
                }
            }
        }
        Ok(GenPreview {
            columns: g.columns(),
            rows,
            waiting,
            fault,
        })
    }

    pub async fn gen_run(
        &self,
        req: FakeRequest,
        progress: impl Fn(Progress) + Send + Sync,
    ) -> Result<GenDone> {
        let session = self.live(&req.session_id).await?;
        if session.guarded() {
            return Err(ToolFault::Production.into());
        }
        self.authorize(&session, Access::Write { confirm: None })?;
        if req.rows == 0 || req.rows > GEN_ROWS_MAX || req.parents.is_some_and(|p| p > GEN_ROWS_MAX)
        {
            return Err(ToolFault::Empty.into());
        }
        let slot = self.job_start(&req.job_id)?;
        let started = Instant::now();
        let side = self.dial_tool(&session).await?;
        let conn = side.conn.as_ref();
        slot.job.watch(conn.canceller());
        let dialect = conn.sql()?;
        let schema = conn.schema().await?;
        let target = profile_of(conn, &schema, &req.namespace, &req.name).await?;
        let mut steps = Vec::new();
        if let Some(n) = req.parents.filter(|n| *n > 0) {
            for t in &target.ancestors {
                let profile = profile_of(conn, &schema, &t.namespace, &t.name).await?;
                let columns = profile
                    .columns
                    .iter()
                    .map(|c| c.suggested.clone())
                    .collect();
                steps.push(Step {
                    seed: seed_for(req.seed, &profile.name),
                    profile,
                    columns,
                    rows: n,
                    locale: req.locale,
                });
            }
        }
        steps.push(Step {
            columns: sanitized(&target, &req.columns),
            seed: seed_for(req.seed, &target.name),
            profile: target,
            rows: req.rows,
            locale: req.locale,
        });
        let total: u64 = steps.iter().map(|s| u64::from(s.rows)).sum();
        let atomic = dialect != Dialect::Clickhouse;
        if atomic {
            quiet(conn, nd_sql::begin(dialect)).await?;
        }
        let mut w = Writer {
            conn,
            job: &slot.job,
            schema: &schema,
            total,
            pace: Pace::new(),
            progress: &progress,
            done: GenDone {
                tables: Vec::new(),
                inserted: 0,
                committed: false,
                kept: !atomic,
                cancelled: false,
                fault: None,
                elapsed_ms: 0,
            },
            error: None,
            script: if atomic {
                vec![nd_sql::begin(dialect).to_owned()]
            } else {
                Vec::new()
            },
        };
        for step in &steps {
            if !w.step(step).await {
                break;
            }
        }
        let (mut done, error, mut script) = (w.done, w.error, w.script);
        if slot.job.stopped() {
            done.cancelled = true;
        }
        settle(conn, atomic, &mut done, &mut script).await;
        self.audit(
            &session,
            &script.join(";\n"),
            (done.committed || done.kept).then_some(done.inserted),
            error,
        );
        done.elapsed_ms = ms(started);
        Ok(done)
    }
}
