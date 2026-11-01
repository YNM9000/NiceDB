use std::collections::HashMap;

use nd_driver::{
    ColumnInfo, DdlKind, Dependency, Dialect, Function, Index, Namespace, ObjectRef, Relation,
    Result, Row, Schema, by_text, ident, tidy,
};

use crate::ClickhouseConnection;

const DATABASES: &str = "SELECT name FROM system.databases ORDER BY name";
const TABLES: &str = "SELECT name, engine, total_rows, partition_key, sorting_key, primary_key, \
    sampling_key, create_table_query FROM system.tables \
    WHERE database = currentDatabase() AND NOT is_temporary ORDER BY name";
const COLUMNS: &str = "SELECT table, name, type, is_in_primary_key FROM system.columns \
    WHERE database = currentDatabase() ORDER BY table, position";
const SKIPPING: &str = "SELECT table, name, type, expr FROM system.data_skipping_indices \
    WHERE database = currentDatabase() ORDER BY table, name";
const DICTIONARIES: &str = "SELECT name, status, element_count, type FROM system.dictionaries \
    WHERE database = currentDatabase()";
const FUNCTIONS: &str = "SELECT name, create_query FROM system.functions \
    WHERE origin = 'SQLUserDefined' ORDER BY name";

fn text(r: &Row, i: usize) -> String {
    r.get(i).cloned().flatten().unwrap_or_default()
}

pub fn key_parts(key: &str) -> Vec<String> {
    let mut out = Vec::new();
    let (mut depth, mut start) = (0i32, 0);
    for (i, c) in key.char_indices() {
        match c {
            '(' | '[' => depth += 1,
            ')' | ']' => depth -= 1,
            ',' if depth == 0 => {
                out.push(key[start..i].trim().to_owned());
                start = i + 1;
            }
            _ => {}
        }
    }
    let last = key[start..].trim();
    if !last.is_empty() {
        out.push(last.to_owned());
    }
    out
}

pub(crate) fn mv_target(create: &str) -> Option<String> {
    let head = create.split(" AS ").next()?;
    let after = head.split_once(" TO ")?.1.trim();
    let name = after
        .split(|c: char| c.is_whitespace() || c == '(')
        .next()?;
    let name = name.rsplit('.').next().unwrap_or(name).trim_matches('`');
    (!name.is_empty()).then(|| name.to_owned())
}

fn key(name: &str, expr: &str) -> Option<Index> {
    (!expr.trim().is_empty()).then(|| Index {
        name: name.to_owned(),
        columns: key_parts(expr),
        unique: false,
    })
}

pub(crate) async fn load(conn: &ClickhouseConnection) -> Result<Schema> {
    let databases = conn.rows(DATABASES, false).await?;
    let tables = conn.rows(TABLES, false).await?;
    let columns = conn.rows(COLUMNS, false).await?;
    let skipping = conn.rows(SKIPPING, false).await.unwrap_or_default();
    let dictionaries = conn.rows(DICTIONARIES, false).await.unwrap_or_default();
    let functions = conn.rows(FUNCTIONS, false).await.unwrap_or_default();

    let mut cols: HashMap<String, Vec<ColumnInfo>> = HashMap::new();
    for r in &columns {
        let type_name = text(r, 2);
        cols.entry(text(r, 0)).or_default().push(ColumnInfo {
            name: text(r, 1),
            nullable: type_name.starts_with("Nullable(")
                || type_name.starts_with("LowCardinality(Nullable("),
            type_name,
            primary_key: text(r, 3) == "1",
        });
    }
    let mut skips: HashMap<String, Vec<Index>> = HashMap::new();
    for r in &skipping {
        skips.entry(text(r, 0)).or_default().push(Index {
            name: text(r, 1),
            columns: vec![format!("{} · {}", text(r, 3), text(r, 2))],
            unique: false,
        });
    }
    let dicts: HashMap<String, (String, String, String)> = dictionaries
        .iter()
        .map(|r| (text(r, 0), (text(r, 1), text(r, 2), text(r, 3))))
        .collect();

    let mut ns = Namespace::new(conn.database.clone());
    for r in &tables {
        let name = text(r, 0);
        let engine = text(r, 1);
        let mut indexes = Vec::new();
        let (partition, sorting, primary, sampling) =
            (text(r, 3), text(r, 4), text(r, 5), text(r, 6));
        indexes.extend(key("ORDER BY", &sorting));
        if primary.trim() != sorting.trim() {
            indexes.extend(key("PRIMARY KEY", &primary));
        }
        indexes.extend(key("PARTITION BY", &partition));
        indexes.extend(key("SAMPLE BY", &sampling));
        indexes.extend(skips.remove(&name).unwrap_or_default());
        let mut rel = Relation {
            columns: cols.remove(&name).unwrap_or_default(),
            indexes,
            foreign_keys: Vec::new(),
            #[allow(clippy::cast_precision_loss)]
            estimated_rows: r
                .get(2)
                .cloned()
                .flatten()
                .and_then(|n| n.parse::<u64>().ok())
                .map(|n| n as f64),
            detail: Some(engine.clone()),
            name,
        };
        match engine.as_str() {
            "View" => {
                rel.detail = None;
                ns.views.push(rel);
            }
            "MaterializedView" => {
                rel.detail = mv_target(&text(r, 7)).map(|t| format!("→ {t}"));
                ns.materialized_views.push(rel);
            }
            "Dictionary" => {
                if let Some((status, count, layout)) = dicts.get(&rel.name) {
                    rel.detail = Some(
                        format!("{layout} · {status}")
                            .trim_start_matches(" · ")
                            .to_owned(),
                    );
                    rel.estimated_rows = count.parse::<f64>().ok().filter(|n| *n > 0.0);
                }
                ns.dictionaries.push(rel);
            }
            _ => ns.tables.push(rel),
        }
    }
    ns.functions = functions
        .iter()
        .map(|r| Function {
            name: text(r, 0),
            signature: text(r, 1)
                .split_once(" AS ")
                .map(|(_, s)| s.trim().to_owned())
                .unwrap_or_default(),
        })
        .collect();
    Ok(Schema {
        databases: databases.iter().map(|r| text(r, 0)).collect(),
        current_database: conn.database.clone(),
        namespaces: vec![ns],
    })
}

pub(crate) async fn ddl(conn: &ClickhouseConnection, object: &ObjectRef) -> Result<String> {
    let d = Dialect::Clickhouse;
    let qualified = |name: &str| format!("{}.{}", ident(d, &object.namespace), ident(d, name));
    let sql = match object.kind {
        DdlKind::Table | DdlKind::View => format!("SHOW CREATE TABLE {}", qualified(&object.name)),
        DdlKind::Index => {
            let table = object.table.as_deref().unwrap_or(&object.name);
            format!("SHOW CREATE TABLE {}", qualified(table))
        }
        DdlKind::Routine => format!(
            "SELECT create_query FROM system.functions WHERE name = {}",
            nd_sql::literal(d, Some(&object.name))
        ),
        DdlKind::Trigger | DdlKind::Sequence => return Err(object.not_found()),
    };
    let found = conn.rows(&sql, false).await?;
    let body = found
        .first()
        .map(|r| text(r, 0))
        .filter(|s| !s.is_empty())
        .ok_or_else(|| object.not_found())?;
    Ok(format!("{};\n", body.trim_end().trim_end_matches(';')))
}

pub(crate) async fn dependencies(
    conn: &ClickhouseConnection,
    object: &ObjectRef,
) -> Result<Vec<Dependency>> {
    let sql = format!(
        "SELECT 'object', if(engine IN ('View', 'MaterializedView'), 'view', 'table'), database, name, '', \
         create_table_query FROM system.tables WHERE database = {} AND NOT is_temporary",
        nd_sql::literal(Dialect::Clickhouse, Some(&object.namespace))
    );
    let rows = conn.rows(&sql, false).await?;
    Ok(tidy(by_text(object, &rows)))
}
