use std::collections::BTreeMap;

use mysql_async::Conn;
use mysql_async::prelude::{FromRow, Queryable};
use nd_driver::{
    ColumnInfo, FkAction, ForeignKey, Function, Index, Namespace, Object, Relation, Result, Schema,
};

use crate::error::map_error;

pub(crate) async fn load(conn: &mut Conn, current: &str) -> Result<Schema> {
    let databases = conn
        .query("SELECT schema_name FROM information_schema.schemata ORDER BY 1")
        .await
        .map_err(|e| map_error(&e, ""))?;
    if current.is_empty() {
        return Ok(Schema {
            databases,
            current_database: String::new(),
            namespaces: Vec::new(),
        });
    }

    let mut columns = load_columns(conn, current).await?;
    let mut indexes = load_indexes(conn, current).await?;
    let mut foreign_keys = load_foreign_keys(conn, current).await?;

    let mut ns = Namespace::new(current);
    for (name, kind, rows) in relations(conn, current).await? {
        #[allow(clippy::cast_precision_loss)]
        let relation = Relation {
            columns: columns.remove(&name).unwrap_or_default(),
            indexes: indexes.remove(&name).unwrap_or_default(),
            foreign_keys: foreign_keys.remove(&name).unwrap_or_default(),
            estimated_rows: rows.map(|r| r as f64),
            detail: None,
            name,
        };
        if kind == "VIEW" {
            ns.views.push(relation);
        } else if kind == "SEQUENCE" {
            ns.sequences.push(Object {
                name: relation.name,
                detail: None,
            });
        } else {
            ns.tables.push(relation);
        }
    }

    ns.functions = q::<(String, String)>(
        conn,
        "SELECT r.routine_name,
                COALESCE(GROUP_CONCAT(CONCAT_WS(' ', NULLIF(p.parameter_mode, 'IN'), p.parameter_name, p.dtd_identifier)
                                      ORDER BY p.ordinal_position SEPARATOR ', '), '')
         FROM information_schema.routines r
         LEFT JOIN information_schema.parameters p
           ON p.specific_schema = r.routine_schema AND p.specific_name = r.specific_name AND p.ordinal_position > 0
         WHERE r.routine_schema = ?
         GROUP BY r.routine_schema, r.specific_name, r.routine_name, r.routine_type ORDER BY 1",
        current,
    )
    .await?
    .into_iter()
    .map(|(name, signature)| Function { name, signature })
    .collect();

    ns.triggers = q::<(String, String)>(
        conn,
        "SELECT trigger_name, event_object_table FROM information_schema.triggers
         WHERE trigger_schema = ? ORDER BY 1",
        current,
    )
    .await?
    .into_iter()
    .map(|(name, table)| Object {
        name,
        detail: Some(table),
    })
    .collect();

    Ok(Schema {
        databases,
        current_database: current.to_owned(),
        namespaces: vec![ns],
    })
}

async fn relations(conn: &mut Conn, current: &str) -> Result<Vec<(String, String, Option<u64>)>> {
    let expiry: Option<u64> = conn
        .query_first("SELECT @@SESSION.information_schema_stats_expiry")
        .await
        .ok()
        .flatten()
        .filter(|e| *e != 0);
    if expiry.is_some() {
        conn.query_drop("SET SESSION information_schema_stats_expiry = 0")
            .await
            .map_err(|e| map_error(&e, ""))?;
    }
    let found = q::<(String, String, Option<u64>)>(
        conn,
        "SELECT table_name, table_type, table_rows FROM information_schema.tables
         WHERE table_schema = ? ORDER BY table_name",
        current,
    )
    .await;
    if let Some(e) = expiry {
        conn.query_drop(format!("SET SESSION information_schema_stats_expiry = {e}"))
            .await
            .map_err(|e| map_error(&e, ""))?;
    }
    found
}

async fn load_columns(conn: &mut Conn, current: &str) -> Result<BTreeMap<String, Vec<ColumnInfo>>> {
    let mut columns: BTreeMap<String, Vec<ColumnInfo>> = BTreeMap::new();
    for (table, name, type_name, nullable, primary_key) in
        q::<(String, String, String, bool, bool)>(
            conn,
            "SELECT table_name, column_name, column_type, is_nullable = 'YES', column_key = 'PRI'
         FROM information_schema.columns WHERE table_schema = ?
         ORDER BY table_name, ordinal_position",
            current,
        )
        .await?
    {
        columns.entry(table).or_default().push(ColumnInfo {
            name,
            type_name,
            nullable,
            primary_key,
        });
    }
    Ok(columns)
}

async fn load_indexes(conn: &mut Conn, current: &str) -> Result<BTreeMap<String, Vec<Index>>> {
    let mut indexes: BTreeMap<String, Vec<Index>> = BTreeMap::new();
    for (table, name, unique, column) in q::<(String, String, bool, Option<String>)>(
        conn,
        "SELECT table_name, index_name, non_unique = 0, column_name
         FROM information_schema.statistics WHERE table_schema = ?
         ORDER BY table_name, index_name, seq_in_index",
        current,
    )
    .await?
    {
        let list = indexes.entry(table).or_default();
        if list.last().is_none_or(|i| i.name != name) {
            list.push(Index {
                name,
                columns: Vec::new(),
                unique,
            });
        }
        if let Some(i) = list.last_mut() {
            i.columns
                .push(column.unwrap_or_else(|| "(expression)".into()));
        }
    }
    Ok(indexes)
}

async fn load_foreign_keys(
    conn: &mut Conn,
    current: &str,
) -> Result<BTreeMap<String, Vec<ForeignKey>>> {
    let mut keys: BTreeMap<String, Vec<ForeignKey>> = BTreeMap::new();
    for (table, name, ref_ns, ref_table, column, ref_column, on_delete, on_update) in
        q::<(String, String, String, String, String, String, String, String)>(
            conn,
            "SELECT k.table_name, k.constraint_name, k.referenced_table_schema, k.referenced_table_name,
                    k.column_name, k.referenced_column_name, r.delete_rule, r.update_rule
             FROM information_schema.key_column_usage k
             JOIN information_schema.referential_constraints r
               ON r.constraint_schema = k.constraint_schema AND r.constraint_name = k.constraint_name
              AND r.table_name = k.table_name
             WHERE k.table_schema = ? AND k.referenced_table_name IS NOT NULL
             ORDER BY k.table_name, k.constraint_name, k.ordinal_position",
            current,
        )
        .await?
    {
        let list = keys.entry(table).or_default();
        if list.last().is_none_or(|k| k.name != name) {
            list.push(ForeignKey {
                name,
                columns: Vec::new(),
                ref_namespace: ref_ns,
                ref_table,
                ref_columns: Vec::new(),
                on_delete: FkAction::parse(&on_delete),
                on_update: FkAction::parse(&on_update),
            });
        }
        if let Some(k) = list.last_mut() {
            k.columns.push(column);
            k.ref_columns.push(ref_column);
        }
    }
    Ok(keys)
}

async fn q<T: FromRow + Send + 'static>(conn: &mut Conn, sql: &str, db: &str) -> Result<Vec<T>> {
    conn.exec(sql, (db,)).await.map_err(|e| map_error(&e, sql))
}
