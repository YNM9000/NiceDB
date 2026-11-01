use std::collections::BTreeMap;

use nd_driver::{
    ColumnInfo, FkAction, ForeignKey, Function, Index, Namespace, Object, Relation, Result, Schema,
};

use crate::error::map_error;

type Key = (String, String);

fn rows<T>(
    conn: &duckdb::Connection,
    sql: &str,
    f: impl FnMut(&duckdb::Row<'_>) -> duckdb::Result<T>,
) -> Result<Vec<T>> {
    let err = |e: duckdb::Error| map_error(&e, sql);
    let mut stmt = conn.prepare(sql).map_err(err)?;
    stmt.query_map([], f)
        .map_err(err)?
        .collect::<duckdb::Result<Vec<T>>>()
        .map_err(err)
}

fn list(text: Option<String>) -> Vec<String> {
    let Some(t) = text else {
        return Vec::new();
    };
    let inner = t.trim().trim_start_matches('[').trim_end_matches(']');
    if inner.trim().is_empty() {
        return Vec::new();
    }
    inner.split(", ").map(|s| s.trim().to_owned()).collect()
}

pub(crate) fn load(conn: &duckdb::Connection) -> Result<Schema> {
    let current: String = rows(conn, "SELECT current_database()", |r| r.get(0))?
        .pop()
        .unwrap_or_default();
    let databases = rows(
        conn,
        "SELECT database_name FROM duckdb_databases() WHERE NOT internal ORDER BY 1",
        |r| r.get(0),
    )?;
    let mut namespaces: BTreeMap<String, Namespace> = rows(
        conn,
        "SELECT schema_name FROM duckdb_schemas()
         WHERE database_name = current_database()
           AND schema_name NOT IN ('information_schema', 'pg_catalog')",
        |r| r.get::<_, String>(0),
    )?
    .into_iter()
    .map(|name| (name.clone(), Namespace::new(name)))
    .collect();

    let mut columns = load_columns(conn)?;
    let mut indexes = load_indexes(conn)?;
    let mut foreign_keys = load_foreign_keys(conn)?;
    for (ns, name, view, estimate) in relations(conn)? {
        let key = (ns.clone(), name.clone());
        #[allow(clippy::cast_precision_loss)]
        let relation = Relation {
            columns: columns.remove(&key).unwrap_or_default(),
            indexes: indexes.remove(&key).unwrap_or_default(),
            foreign_keys: foreign_keys.remove(&key).unwrap_or_default(),
            estimated_rows: estimate.map(|n| n as f64),
            detail: None,
            name,
        };
        if let Some(n) = namespaces.get_mut(&ns) {
            if view {
                n.views.push(relation);
            } else {
                n.tables.push(relation);
            }
        }
    }

    for (ns, name, signature) in rows(
        conn,
        "SELECT schema_name, function_name, CAST(parameters AS VARCHAR) FROM duckdb_functions()
         WHERE database_name = current_database() AND NOT internal
           AND function_type IN ('macro', 'table_macro')
         ORDER BY function_name",
        |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, Option<String>>(2)?,
            ))
        },
    )? {
        if let Some(n) = namespaces.get_mut(&ns) {
            n.functions.push(Function {
                name,
                signature: list(signature).join(", "),
            });
        }
    }

    for (ns, name) in rows(
        conn,
        "SELECT schema_name, sequence_name FROM duckdb_sequences()
         WHERE database_name = current_database() ORDER BY sequence_name",
        |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)),
    )? {
        if let Some(n) = namespaces.get_mut(&ns) {
            n.sequences.push(Object { name, detail: None });
        }
    }

    for (ns, name, kind) in rows(
        conn,
        "SELECT schema_name, type_name, CAST(logical_type AS VARCHAR) FROM duckdb_types()
         WHERE database_name = current_database() AND NOT internal ORDER BY type_name",
        |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, Option<String>>(2)?,
            ))
        },
    )? {
        if let Some(n) = namespaces.get_mut(&ns) {
            n.types.push(Object { name, detail: kind });
        }
    }

    let mut ordered: Vec<Namespace> = namespaces.into_values().collect();
    ordered.sort_by_key(|n| n.name != "main");
    Ok(Schema {
        databases,
        current_database: current,
        namespaces: ordered,
    })
}

fn load_columns(conn: &duckdb::Connection) -> Result<BTreeMap<Key, Vec<ColumnInfo>>> {
    let mut columns: BTreeMap<Key, Vec<ColumnInfo>> = BTreeMap::new();
    let pks = primary_keys(conn)?;
    for (ns, table, name, type_name, nullable) in rows(
        conn,
        "SELECT schema_name, table_name, column_name, data_type, is_nullable FROM duckdb_columns()
         WHERE database_name = current_database() AND NOT internal
         ORDER BY schema_name, table_name, column_index",
        |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)),
    )? {
        let key: Key = (ns, table);
        let primary_key = pks.get(&key).is_some_and(|k| k.contains(&name));
        columns.entry(key).or_default().push(ColumnInfo {
            name,
            type_name,
            nullable,
            primary_key,
        });
    }

    Ok(columns)
}

type RelationRow = (String, String, bool, Option<i64>);

fn relations(conn: &duckdb::Connection) -> Result<Vec<RelationRow>> {
    rows(
        conn,
        "SELECT schema_name, table_name, false, estimated_size FROM duckdb_tables()
         WHERE database_name = current_database() AND NOT internal
         UNION ALL
         SELECT schema_name, view_name, true, NULL FROM duckdb_views()
         WHERE database_name = current_database() AND NOT internal
         ORDER BY 1, 2",
        |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, bool>(2)?,
                r.get::<_, Option<i64>>(3)?,
            ))
        },
    )
}

type Constraint = (
    String,
    String,
    String,
    Option<String>,
    Option<String>,
    Option<String>,
    Option<String>,
);

fn constraints(conn: &duckdb::Connection, kind: &str) -> Result<Vec<Constraint>> {
    rows(
        conn,
        &format!(
            "SELECT schema_name, table_name, COALESCE(constraint_name, ''),
                    CAST(constraint_column_names AS VARCHAR), referenced_table,
                    CAST(referenced_column_names AS VARCHAR), constraint_text
             FROM duckdb_constraints()
             WHERE database_name = current_database() AND constraint_type = '{kind}'
             ORDER BY schema_name, table_name, constraint_index"
        ),
        |r| {
            Ok((
                r.get(0)?,
                r.get(1)?,
                r.get(2)?,
                r.get(3)?,
                r.get(4)?,
                r.get(5)?,
                r.get(6)?,
            ))
        },
    )
}

fn primary_keys(conn: &duckdb::Connection) -> Result<BTreeMap<Key, Vec<String>>> {
    Ok(constraints(conn, "PRIMARY KEY")?
        .into_iter()
        .map(|(ns, t, _, cols, ..)| ((ns, t), list(cols)))
        .collect())
}

fn load_indexes(conn: &duckdb::Connection) -> Result<BTreeMap<Key, Vec<Index>>> {
    let mut out: BTreeMap<Key, Vec<Index>> = BTreeMap::new();
    for (ns, table, name, cols, ..) in constraints(conn, "UNIQUE")? {
        let columns = list(cols);
        let name = if name.is_empty() {
            format!("{table}_{}_key", columns.join("_"))
        } else {
            name
        };
        out.entry((ns, table)).or_default().push(Index {
            name,
            columns,
            unique: true,
        });
    }
    for (ns, table, name, unique, expressions) in rows(
        conn,
        "SELECT schema_name, table_name, index_name, is_unique, CAST(expressions AS VARCHAR)
         FROM duckdb_indexes() WHERE database_name = current_database()
         ORDER BY schema_name, table_name, index_name",
        |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, bool>(3)?,
                r.get::<_, Option<String>>(4)?,
            ))
        },
    )? {
        let columns = list(expressions)
            .into_iter()
            .map(|c| c.trim_matches(['\'', '"']).to_owned())
            .collect();
        out.entry((ns, table)).or_default().push(Index {
            name,
            columns,
            unique,
        });
    }
    Ok(out)
}

fn action(text: &str, on: &str) -> FkAction {
    let upper = text.to_ascii_uppercase();
    upper.find(on).map_or(FkAction::NoAction, |i| {
        let rest = &upper[i + on.len()..];
        let words: Vec<&str> = rest.split_whitespace().take(2).collect();
        let two = matches!(words.first(), Some(&("SET" | "NO")));
        FkAction::parse(&words[..if two { words.len() } else { words.len().min(1) }].join(" "))
    })
}

fn load_foreign_keys(conn: &duckdb::Connection) -> Result<BTreeMap<Key, Vec<ForeignKey>>> {
    let mut out: BTreeMap<Key, Vec<ForeignKey>> = BTreeMap::new();
    for (ns, table, name, cols, ref_table, ref_cols, text) in constraints(conn, "FOREIGN KEY")? {
        let columns = list(cols);
        let text = text.unwrap_or_default();
        let name = if name.is_empty() {
            format!("{table}_{}_fkey", columns.join("_"))
        } else {
            name
        };
        out.entry((ns.clone(), table))
            .or_default()
            .push(ForeignKey {
                name,
                columns,
                ref_namespace: ns,
                ref_table: ref_table.unwrap_or_default(),
                ref_columns: list(ref_cols),
                on_delete: action(&text, "ON DELETE "),
                on_update: action(&text, "ON UPDATE "),
            });
    }
    Ok(out)
}
