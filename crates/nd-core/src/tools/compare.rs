use crate::session::Sql as _;
use std::collections::HashMap;

use nd_diff::{Col, DiffItem, Fk, Idx, Routine, Snapshot, Table, View};
use nd_driver::{Connection, Dialect, DriverError, Relation, Row};
use serde::{Deserialize, Serialize};

use crate::guard::{Access, collect};
use crate::{Core, CoreError, Result};

#[derive(Debug, Clone, Deserialize, specta::Type)]
pub struct CompareRequest {
    pub source_session: String,
    pub source_namespace: String,
    pub target_session: String,
    pub target_namespace: String,
}

#[derive(Debug, Clone, Serialize, specta::Type)]
pub struct Comparison {
    pub items: Vec<DiffItem>,
    pub statements: Vec<String>,
    pub source_dialect: Dialect,
    pub target_dialect: Dialect,
}

type Rows = std::result::Result<Vec<Row>, DriverError>;

pub(super) fn cell(r: &Row, i: usize) -> Option<String> {
    r.get(i).cloned().flatten()
}

pub(super) async fn bound(conn: &dyn Connection, sql: &str, ns: &str) -> Rows {
    collect(conn, sql, Some(&[Some(ns.to_owned())]))
        .await
        .map(|(r, _)| r)
}

pub(super) fn local(def: &str, ns: &str, dialect: Dialect) -> String {
    let quoted = format!("{}.", nd_sql::ident(dialect, ns));
    let bare = format!("{ns}.");
    let out = def.replace(&quoted, "");
    if dialect == Dialect::Mysql {
        out
    } else {
        out.replace(&format!(" {bare}"), " ")
            .replace(&format!("({bare}"), "(")
    }
}

fn restated(def: &str, name: &str, ns: &str, dialect: Dialect) -> String {
    let head = nd_diff::restate(def, name, dialect, None).unwrap_or_else(|| def.to_owned());
    local(&head, ns, dialect)
}

fn without_definer(def: &str) -> String {
    let Some(at) = def.find("DEFINER=") else {
        return def.to_owned();
    };
    let rest = &def[at..];
    let end = rest
        .find(" FUNCTION")
        .or_else(|| rest.find(" PROCEDURE"))
        .unwrap_or(rest.len());
    format!("{}{}", def[..at].trim_end(), &rest[end..])
}

fn mysql_default(
    raw: Option<String>,
    extra: &str,
    data_type: &str,
    mariadb: bool,
) -> Option<String> {
    let raw = raw?;
    if mariadb {
        return (raw != "NULL").then_some(raw);
    }
    let upper = raw.to_ascii_uppercase();
    if upper.starts_with("CURRENT_TIMESTAMP") || upper == "NULL" {
        return (upper != "NULL").then_some(raw);
    }
    if extra.contains("DEFAULT_GENERATED") {
        return Some(format!("({raw})"));
    }
    let numeric = matches!(
        data_type,
        "tinyint"
            | "smallint"
            | "mediumint"
            | "int"
            | "bigint"
            | "decimal"
            | "float"
            | "double"
            | "bit"
    );
    Some(if numeric {
        raw
    } else {
        nd_sql::literal(Dialect::Mysql, Some(&raw))
    })
}

fn table_of(rel: &Relation, ns: &str) -> Table {
    let primary_key: Vec<String> = rel
        .columns
        .iter()
        .filter(|c| c.primary_key)
        .map(|c| c.name.clone())
        .collect();
    Table {
        name: rel.name.clone(),
        columns: rel
            .columns
            .iter()
            .map(|c| Col {
                name: c.name.clone(),
                type_name: c.type_name.clone(),
                nullable: c.nullable,
                default: None,
                generated: false,
                default_name: None,
            })
            .collect(),
        indexes: rel
            .indexes
            .iter()
            .filter(|i| !(i.unique && i.columns == primary_key))
            .filter(|i| !i.name.starts_with("sqlite_autoindex"))
            .map(|i| Idx {
                name: i.name.clone(),
                columns: i.columns.clone(),
                unique: i.unique,
            })
            .collect(),
        create: None,
        foreign_keys: rel
            .foreign_keys
            .iter()
            .map(|f| Fk {
                name: f.name.clone(),
                columns: f.columns.clone(),
                ref_table: if f.ref_namespace.is_empty() || f.ref_namespace == ns {
                    f.ref_table.clone()
                } else {
                    format!("{}.{}", f.ref_namespace, f.ref_table)
                },
                ref_columns: f.ref_columns.clone(),
                on_delete: f.on_delete,
                on_update: f.on_update,
            })
            .collect(),
        primary_key,
        pk_name: None,
    }
}

const PG_DEFAULTS: &str = "SELECT c.relname, a.attname, pg_get_expr(d.adbin, d.adrelid), a.attidentity::text \
FROM pg_attribute a JOIN pg_class c ON c.oid = a.attrelid JOIN pg_namespace n ON n.oid = c.relnamespace \
LEFT JOIN pg_attrdef d ON d.adrelid = a.attrelid AND d.adnum = a.attnum \
WHERE n.nspname = $1 AND c.relkind IN ('r', 'p') AND a.attnum > 0 AND NOT a.attisdropped \
AND (d.adbin IS NOT NULL OR a.attidentity <> '')";

const PG_VIEWS: &str = "SELECT c.relname, pg_get_viewdef(c.oid, true) FROM pg_class c \
JOIN pg_namespace n ON n.oid = c.relnamespace WHERE n.nspname = $1 AND c.relkind IN ('v', 'm')";

const PG_ROUTINES: &str = "SELECT p.proname, pg_get_function_identity_arguments(p.oid), (p.prokind = 'p')::text, \
pg_get_functiondef(p.oid) FROM pg_proc p JOIN pg_namespace n ON n.oid = p.pronamespace \
WHERE n.nspname = $1 AND p.prokind IN ('f', 'p')";

const MY_COLUMNS: &str = "SELECT TABLE_NAME, COLUMN_NAME, COLUMN_DEFAULT, EXTRA, COLUMN_TYPE, DATA_TYPE \
FROM information_schema.COLUMNS WHERE TABLE_SCHEMA = ?";

const MY_VIEWS: &str =
    "SELECT TABLE_NAME, VIEW_DEFINITION FROM information_schema.VIEWS WHERE TABLE_SCHEMA = ?";

const MY_ROUTINES: &str =
    "SELECT ROUTINE_NAME, ROUTINE_TYPE FROM information_schema.ROUTINES WHERE ROUTINE_SCHEMA = ?";

const SQLITE_DEFAULTS: &str = "SELECT m.name, p.name, p.dflt_value, '' FROM sqlite_schema m \
JOIN pragma_table_info(m.name) p WHERE m.type = 'table' AND p.dflt_value IS NOT NULL";

const SQLITE_VIEWS: &str = "SELECT name, sql FROM sqlite_schema WHERE type = 'view'";

const MS_COLUMNS: &str = "SELECT t.name, c.name, dc.definition, CASE WHEN c.is_identity = 1 THEN 'true' ELSE 'false' END, dc.name \
FROM sys.columns c JOIN sys.tables t ON t.object_id = c.object_id JOIN sys.schemas s ON s.schema_id = t.schema_id \
LEFT JOIN sys.default_constraints dc ON dc.parent_object_id = c.object_id AND dc.parent_column_id = c.column_id \
WHERE s.name = @P1 AND (dc.object_id IS NOT NULL OR c.is_identity = 1)";

const MS_PRIMARY_KEYS: &str = "SELECT t.name, kc.name FROM sys.key_constraints kc \
JOIN sys.tables t ON t.object_id = kc.parent_object_id JOIN sys.schemas s ON s.schema_id = t.schema_id \
WHERE kc.type = 'PK' AND s.name = @P1";

const MS_MODULES: &str = "SELECT o.name, o.type, m.definition FROM sys.sql_modules m \
JOIN sys.objects o ON o.object_id = m.object_id JOIN sys.schemas s ON s.schema_id = o.schema_id \
WHERE s.name = @P1 AND o.is_ms_shipped = 0 AND o.type IN ('V', 'P', 'FN', 'IF', 'TF')";

const DUCK_COLUMNS: &str = "SELECT table_name, column_name, column_default FROM duckdb_columns() \
WHERE database_name = current_database() AND schema_name = ? AND NOT internal AND column_default IS NOT NULL";

const DUCK_VIEWS: &str = "SELECT view_name, sql FROM duckdb_views() \
WHERE database_name = current_database() AND schema_name = ? AND NOT internal";

const DUCK_MACROS: &str = "SELECT function_name, CAST(function_type = 'table_macro' AS VARCHAR), \
'CREATE MACRO ' || function_name || '(' || array_to_string(parameters, ', ') || ') AS ' \
|| CASE WHEN function_type = 'table_macro' THEN 'TABLE ' ELSE '' END || macro_definition \
FROM duckdb_functions() WHERE database_name = current_database() AND schema_name = ? AND NOT internal \
AND function_type IN ('macro', 'table_macro')";

struct ColumnExtra {
    default: Option<String>,
    generated: bool,
    full_type: Option<String>,
    default_name: Option<String>,
}

pub(super) struct Extra {
    defs: bool,
    columns: HashMap<(String, String), ColumnExtra>,
    views: HashMap<String, String>,
    routines: HashMap<String, (bool, Option<String>)>,
    primary_keys: HashMap<String, String>,
}

async fn scoped(conn: &dyn Connection, sql: &str, ns: &str, table: Option<&str>) -> Rows {
    match table {
        None => bound(conn, sql, ns).await,
        Some(t) => {
            let one = format!("{sql} AND t.name = @P2");
            collect(conn, &one, Some(&[Some(ns.to_owned()), Some(t.to_owned())]))
                .await
                .map(|(r, _)| r)
        }
    }
}

async fn mssql_extra(
    conn: &dyn Connection,
    ns: &str,
    table: Option<&str>,
    x: &mut Extra,
) -> std::result::Result<(), DriverError> {
    for r in scoped(conn, MS_COLUMNS, ns, table).await? {
        let key = (
            cell(&r, 0).unwrap_or_default(),
            cell(&r, 1).unwrap_or_default(),
        );
        let generated = cell(&r, 3).as_deref() == Some("true");
        x.columns.insert(
            key,
            ColumnExtra {
                default: cell(&r, 2).filter(|_| !generated),
                generated,
                full_type: None,
                default_name: cell(&r, 4),
            },
        );
    }
    for r in scoped(conn, MS_PRIMARY_KEYS, ns, table).await? {
        if let (Some(t), Some(n)) = (cell(&r, 0), cell(&r, 1)) {
            x.primary_keys.insert(t, n);
        }
    }
    if table.is_some() {
        return Ok(());
    }
    for r in defs(x, bound(conn, MS_MODULES, ns)).await? {
        let (Some(name), Some(kind), Some(def)) = (cell(&r, 0), cell(&r, 1), cell(&r, 2)) else {
            continue;
        };
        let def = restated(&def, &name, ns, Dialect::Mssql);
        if kind.trim() == "V" {
            x.views.insert(name, def);
        } else {
            x.routines.insert(name, (kind.trim() == "P", Some(def)));
        }
    }
    Ok(())
}

async fn duckdb_extra(
    conn: &dyn Connection,
    ns: &str,
    x: &mut Extra,
) -> std::result::Result<(), DriverError> {
    for r in bound(conn, DUCK_COLUMNS, ns).await? {
        let key = (
            cell(&r, 0).unwrap_or_default(),
            cell(&r, 1).unwrap_or_default(),
        );
        x.columns.insert(
            key,
            ColumnExtra {
                default: cell(&r, 2),
                generated: false,
                full_type: None,
                default_name: None,
            },
        );
    }
    for r in defs(x, bound(conn, DUCK_VIEWS, ns)).await? {
        if let (Some(n), Some(d)) = (cell(&r, 0), cell(&r, 1)) {
            let d = restated(&d, &n, ns, Dialect::Duckdb);
            x.views.insert(n, d);
        }
    }
    for r in defs(x, bound(conn, DUCK_MACROS, ns)).await? {
        if let Some(name) = cell(&r, 0) {
            let def = cell(&r, 2).map(|d| restated(&d, &name, ns, Dialect::Duckdb));
            x.routines
                .insert(name, (cell(&r, 1).as_deref() == Some("true"), def));
        }
    }
    Ok(())
}

async fn pg_extra(
    conn: &dyn Connection,
    ns: &str,
    x: &mut Extra,
) -> std::result::Result<(), DriverError> {
    let dialect = conn.sql()?;
    for r in bound(conn, PG_DEFAULTS, ns).await? {
        let (t, c) = (
            cell(&r, 0).unwrap_or_default(),
            cell(&r, 1).unwrap_or_default(),
        );
        let identity = cell(&r, 3).unwrap_or_default();
        let (default, generated) = match identity.as_str() {
            "a" => (Some("GENERATED ALWAYS AS IDENTITY".to_owned()), true),
            "d" => (Some("GENERATED BY DEFAULT AS IDENTITY".to_owned()), true),
            _ => {
                let d = cell(&r, 2);
                let serial = d.as_deref().is_some_and(|d| d.starts_with("nextval("));
                (d, serial)
            }
        };
        x.columns.insert(
            (t, c),
            ColumnExtra {
                default,
                generated,
                full_type: None,
                default_name: None,
            },
        );
    }
    for r in defs(x, bound(conn, PG_VIEWS, ns)).await? {
        if let (Some(n), Some(d)) = (cell(&r, 0), cell(&r, 1)) {
            x.views.insert(n, local(&d, ns, dialect));
        }
    }
    for r in defs(x, bound(conn, PG_ROUTINES, ns)).await? {
        let key = format!(
            "{}({})",
            cell(&r, 0).unwrap_or_default(),
            cell(&r, 1).unwrap_or_default()
        );
        let def = cell(&r, 3).map(|d| local(&d, ns, dialect));
        x.routines
            .insert(key, (cell(&r, 2).as_deref() == Some("true"), def));
    }
    Ok(())
}

async fn mysql_extra(
    conn: &dyn Connection,
    ns: &str,
    x: &mut Extra,
) -> std::result::Result<(), DriverError> {
    let dialect = conn.sql()?;
    let mariadb = conn.server().product.contains("MariaDB");
    for r in bound(conn, MY_COLUMNS, ns).await? {
        let extra = cell(&r, 3).unwrap_or_default();
        let data_type = cell(&r, 5).unwrap_or_default().to_ascii_lowercase();
        let default = mysql_default(cell(&r, 2), &extra, &data_type, mariadb);
        let key = (
            cell(&r, 0).unwrap_or_default(),
            cell(&r, 1).unwrap_or_default(),
        );
        let generated = extra.contains("auto_increment");
        x.columns.insert(
            key,
            ColumnExtra {
                default,
                generated,
                full_type: cell(&r, 4),
                default_name: None,
            },
        );
    }
    for r in defs(x, bound(conn, MY_VIEWS, ns)).await? {
        if let (Some(n), Some(d)) = (cell(&r, 0), cell(&r, 1)) {
            x.views.insert(n, local(&d, ns, dialect));
        }
    }
    for r in defs(x, bound(conn, MY_ROUTINES, ns)).await? {
        let (Some(name), Some(kind)) = (cell(&r, 0), cell(&r, 1)) else {
            continue;
        };
        let show = format!(
            "SHOW CREATE {kind} {}.{}",
            nd_sql::ident(dialect, ns),
            nd_sql::ident(dialect, &name)
        );
        let def = collect(conn, &show, None)
            .await
            .ok()
            .and_then(|(rows, _)| rows.first().and_then(|r| cell(r, 2)))
            .map(|d| without_definer(&d));
        x.routines.insert(name, (kind == "PROCEDURE", def));
    }
    Ok(())
}

async fn sqlite_extra(
    conn: &dyn Connection,
    x: &mut Extra,
) -> std::result::Result<(), DriverError> {
    for r in collect(conn, SQLITE_DEFAULTS, None).await?.0 {
        let key = (
            cell(&r, 0).unwrap_or_default(),
            cell(&r, 1).unwrap_or_default(),
        );
        x.columns.insert(
            key,
            ColumnExtra {
                default: cell(&r, 2),
                generated: false,
                full_type: None,
                default_name: None,
            },
        );
    }
    for r in defs(x, async {
        collect(conn, SQLITE_VIEWS, None).await.map(|(r, _)| r)
    })
    .await?
    {
        if let (Some(n), Some(d)) = (cell(&r, 0), cell(&r, 1)) {
            x.views.insert(n, d);
        }
    }
    Ok(())
}

const FB_COLUMNS: &str = "SELECT TRIM(rf.RDB$RELATION_NAME), TRIM(rf.RDB$FIELD_NAME), \
COALESCE(rf.RDB$DEFAULT_SOURCE, f.RDB$DEFAULT_SOURCE), CASE WHEN rf.RDB$IDENTITY_TYPE IS NULL THEN 'NO' ELSE 'YES' END \
FROM RDB$RELATION_FIELDS rf JOIN RDB$FIELDS f ON f.RDB$FIELD_NAME = rf.RDB$FIELD_SOURCE \
JOIN RDB$RELATIONS r ON r.RDB$RELATION_NAME = rf.RDB$RELATION_NAME \
WHERE COALESCE(r.RDB$SYSTEM_FLAG, 0) = 0 AND CAST(? AS VARCHAR(63)) IS NOT NULL";

const FB_PRIMARY_KEYS: &str = "SELECT TRIM(RDB$RELATION_NAME), TRIM(RDB$CONSTRAINT_NAME) FROM RDB$RELATION_CONSTRAINTS \
WHERE RDB$CONSTRAINT_TYPE = 'PRIMARY KEY' AND CAST(? AS VARCHAR(63)) IS NOT NULL";

const FB_VIEWS: &str = "SELECT TRIM(RDB$RELATION_NAME), RDB$VIEW_SOURCE FROM RDB$RELATIONS \
WHERE RDB$VIEW_BLR IS NOT NULL AND COALESCE(RDB$SYSTEM_FLAG, 0) = 0 AND CAST(? AS VARCHAR(63)) IS NOT NULL";

async fn firebird_extra(
    conn: &dyn Connection,
    ns: &str,
    x: &mut Extra,
) -> std::result::Result<(), DriverError> {
    for r in bound(conn, FB_COLUMNS, ns).await? {
        let key = (
            cell(&r, 0).unwrap_or_default(),
            cell(&r, 1).unwrap_or_default(),
        );
        let generated = cell(&r, 3).as_deref() == Some("YES");
        let default = cell(&r, 2).map(|d| {
            let d = d.trim();
            d.get(..8)
                .filter(|w| w.eq_ignore_ascii_case("DEFAULT "))
                .map_or(d, |_| d[8..].trim())
                .to_owned()
        });
        x.columns.insert(
            key,
            ColumnExtra {
                default: default.filter(|_| !generated),
                generated,
                full_type: None,
                default_name: None,
            },
        );
    }
    for r in bound(conn, FB_PRIMARY_KEYS, ns).await? {
        if let (Some(t), Some(n)) = (cell(&r, 0), cell(&r, 1)) {
            x.primary_keys.insert(t, n);
        }
    }
    for r in defs(x, bound(conn, FB_VIEWS, ns)).await? {
        if let (Some(n), Some(d)) = (cell(&r, 0), cell(&r, 1)) {
            x.views.insert(n, d.trim().to_owned());
        }
    }
    Ok(())
}

const ORA_COLUMNS: &str = "SELECT table_name, column_name, data_default, identity_column \
FROM all_tab_columns WHERE owner = :1 AND (data_default IS NOT NULL OR identity_column = 'YES')";

const ORA_PRIMARY_KEYS: &str = "SELECT table_name, constraint_name FROM all_constraints \
WHERE owner = :1 AND constraint_type = 'P'";

const ORA_VIEWS: &str = "SELECT view_name, text FROM all_views WHERE owner = :1";

async fn oracle_extra(
    conn: &dyn Connection,
    ns: &str,
    x: &mut Extra,
) -> std::result::Result<(), DriverError> {
    for r in bound(conn, ORA_COLUMNS, ns).await? {
        let key = (
            cell(&r, 0).unwrap_or_default(),
            cell(&r, 1).unwrap_or_default(),
        );
        let generated = cell(&r, 3).as_deref() == Some("YES");
        x.columns.insert(
            key,
            ColumnExtra {
                default: cell(&r, 2)
                    .map(|d| d.trim().to_owned())
                    .filter(|_| !generated),
                generated,
                full_type: None,
                default_name: None,
            },
        );
    }
    for r in bound(conn, ORA_PRIMARY_KEYS, ns).await? {
        if let (Some(t), Some(n)) = (cell(&r, 0), cell(&r, 1)) {
            x.primary_keys.insert(t, n);
        }
    }
    for r in defs(x, bound(conn, ORA_VIEWS, ns)).await? {
        if let (Some(n), Some(d)) = (cell(&r, 0), cell(&r, 1)) {
            x.views.insert(n, d.trim().to_owned());
        }
    }
    Ok(())
}

async fn defs(x: &Extra, read: impl std::future::Future<Output = Rows>) -> Rows {
    if x.defs { read.await } else { Ok(Vec::new()) }
}

async fn extra(
    conn: &dyn Connection,
    ns: &str,
    defs: bool,
) -> std::result::Result<Extra, DriverError> {
    extra_of(conn, ns, None, defs).await
}

async fn extra_of(
    conn: &dyn Connection,
    ns: &str,
    table: Option<&str>,
    defs: bool,
) -> std::result::Result<Extra, DriverError> {
    let mut x = Extra {
        defs,
        columns: HashMap::new(),
        views: HashMap::new(),
        routines: HashMap::new(),
        primary_keys: HashMap::new(),
    };
    match conn.sql()? {
        Dialect::Postgres => pg_extra(conn, ns, &mut x).await?,
        Dialect::Mysql => mysql_extra(conn, ns, &mut x).await?,
        Dialect::Sqlite => sqlite_extra(conn, &mut x).await?,
        Dialect::Mssql => mssql_extra(conn, ns, table, &mut x).await?,
        Dialect::Duckdb => duckdb_extra(conn, ns, &mut x).await?,
        Dialect::Oracle => oracle_extra(conn, ns, &mut x).await?,
        Dialect::Firebird => firebird_extra(conn, ns, &mut x).await?,
        Dialect::Clickhouse
        | Dialect::Hana
        | Dialect::Db2
        | Dialect::Ansi
        | Dialect::AnsiBacktick
        | Dialect::Cql
        | Dialect::Partiql
        | Dialect::Opensearch
        | Dialect::Elasticsearch => {}
    }
    Ok(x)
}

pub(super) fn table_with(rel: &Relation, ns: &str, x: &Extra) -> Table {
    let mut t = table_of(rel, ns);
    t.pk_name = x.primary_keys.get(&t.name).cloned();
    for c in &mut t.columns {
        if let Some(e) = x.columns.get(&(t.name.clone(), c.name.clone())) {
            c.default.clone_from(&e.default);
            c.generated = e.generated;
            c.default_name.clone_from(&e.default_name);
            if let Some(full) = &e.full_type {
                c.type_name.clone_from(full);
            }
        }
    }
    t
}

pub(super) async fn columns_extra(
    conn: &dyn Connection,
    ns: &str,
    table: &str,
) -> std::result::Result<Extra, DriverError> {
    extra_of(conn, ns, Some(table), false).await
}

async fn snapshot(conn: &dyn Connection, ns: &str) -> std::result::Result<Snapshot, DriverError> {
    if conn.sql()? == Dialect::Clickhouse {
        return super::clickhouse::snapshot(conn, ns).await;
    }
    let schema = conn.schema().await?;
    let Some(space) = schema.namespaces.iter().find(|n| n.name == ns) else {
        return Ok(Snapshot {
            namespace: ns.to_owned(),
            ..Snapshot::default()
        });
    };
    let x = extra(conn, ns, true).await?;
    let by_name = matches!(
        conn.sql()?,
        Dialect::Mysql | Dialect::Mssql | Dialect::Duckdb
    );
    let tables = space
        .tables
        .iter()
        .map(|rel| table_with(rel, ns, &x))
        .collect();
    let views = space
        .views
        .iter()
        .map(|v| View {
            name: v.name.clone(),
            definition: x.views.get(&v.name).cloned(),
        })
        .collect();
    let routines = space
        .functions
        .iter()
        .map(|f| {
            let key = if by_name {
                f.name.clone()
            } else {
                format!("{}({})", f.name, f.signature)
            };
            let (procedure, definition) = x.routines.get(&key).cloned().unwrap_or((false, None));
            Routine {
                name: f.name.clone(),
                signature: f.signature.clone(),
                procedure,
                definition,
            }
        })
        .collect();
    Ok(Snapshot {
        namespace: ns.to_owned(),
        tables,
        views,
        routines,
    })
}

impl Core {
    async fn snapshot_of(&self, session_id: &str, ns: &str) -> Result<(Snapshot, Dialect)> {
        let session = self.live(session_id).await?;
        self.authorize(&session, Access::Read)?;
        let side = self.side(&session).await?;
        let conn = side.conn()?;
        let dialect = conn.sql()?;
        let found = snapshot(conn, ns).await;
        Ok((side.check(found)?, dialect))
    }

    pub async fn compare_schemas(&self, req: CompareRequest) -> Result<Comparison> {
        if req.source_namespace.is_empty() && req.target_namespace.is_empty() {
            return Err(CoreError::NotFound("namespace".into()));
        }
        let (source, source_dialect) = self
            .snapshot_of(&req.source_session, &req.source_namespace)
            .await?;
        let (target, target_dialect) = self
            .snapshot_of(&req.target_session, &req.target_namespace)
            .await?;
        Ok(Comparison {
            items: nd_diff::diff(&source, &target, target_dialect),
            statements: nd_diff::script(&source, &target, target_dialect),
            source_dialect,
            target_dialect,
        })
    }
}
