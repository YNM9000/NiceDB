use nd_driver::{DdlKind, Dialect, DriverError, ObjectRef, Result, ident};

use crate::error::map_error;

fn texts(
    conn: &duckdb::Connection,
    sql: &str,
    params: &[&str],
    width: usize,
) -> Result<Vec<Vec<Option<String>>>> {
    let err = |e: duckdb::Error| map_error(&e, sql);
    let mut stmt = conn.prepare(sql).map_err(err)?;
    stmt.query_map(duckdb::params_from_iter(params), |r| {
        (0..width).map(|i| r.get::<_, Option<String>>(i)).collect()
    })
    .map_err(err)?
    .collect::<duckdb::Result<Vec<_>>>()
    .map_err(err)
}

fn terminated(s: &str) -> String {
    let s = s.trim();
    if s.ends_with(';') {
        s.to_owned()
    } else {
        format!("{s};")
    }
}

fn literal(s: &str) -> String {
    format!("'{}'", s.replace('\'', "''"))
}

fn qualified(ns: &str, name: &str) -> String {
    format!(
        "{}.{}",
        ident(Dialect::Duckdb, ns),
        ident(Dialect::Duckdb, name)
    )
}

pub(crate) fn load(conn: &duckdb::Connection, object: &ObjectRef) -> Result<String> {
    let ns = object.namespace.as_str();
    let name = object.name.as_str();
    let one = |sql: &str| -> Result<String> {
        texts(conn, sql, &[ns, name], 1)?
            .into_iter()
            .next()
            .and_then(|r| r.into_iter().next().flatten())
            .map(|s| terminated(&s))
            .ok_or_else(|| object.not_found())
    };
    match object.kind {
        DdlKind::Table => {
            let mut out = spread(&one(
                "SELECT sql FROM duckdb_tables() WHERE database_name = current_database() AND schema_name = ? AND table_name = ?",
            )?);
            dependents(conn, ns, name, "TABLE", &mut out)?;
            Ok(out)
        }
        DdlKind::View => {
            let mut out = one(
                "SELECT sql FROM duckdb_views() WHERE database_name = current_database() AND schema_name = ? AND view_name = ?",
            )?;
            dependents(conn, ns, name, "VIEW", &mut out)?;
            Ok(out)
        }
        DdlKind::Index => one(
            "SELECT sql FROM duckdb_indexes() WHERE database_name = current_database() AND schema_name = ? AND index_name = ?",
        ),
        DdlKind::Sequence => {
            let sql = one(
                "SELECT sql FROM duckdb_sequences() WHERE database_name = current_database() AND schema_name = ? AND sequence_name = ?",
            )?;
            let bare = format!("CREATE SEQUENCE {name} ");
            Ok(match sql.strip_prefix(&bare) {
                Some(rest) if ns != "main" => {
                    format!("CREATE SEQUENCE {} {rest}", qualified(ns, name))
                }
                _ => sql,
            })
        }
        DdlKind::Routine => routine(conn, object),
        DdlKind::Trigger => Err(DriverError::query("DuckDB has no triggers")),
    }
}

fn dependents(
    conn: &duckdb::Connection,
    ns: &str,
    name: &str,
    what: &str,
    out: &mut String,
) -> Result<()> {
    for r in texts(
        conn,
        "SELECT sql FROM duckdb_indexes() WHERE database_name = current_database() AND schema_name = ? AND table_name = ? AND sql IS NOT NULL ORDER BY index_name",
        &[ns, name],
        1,
    )? {
        if let Some(sql) = r.into_iter().next().flatten() {
            out.push_str("\n\n");
            out.push_str(&terminated(&sql));
        }
    }
    let q = qualified(ns, name);
    let mut notes = Vec::new();
    let table_comment = if what == "TABLE" {
        "SELECT comment FROM duckdb_tables() WHERE database_name = current_database() AND schema_name = ? AND table_name = ?"
    } else {
        "SELECT comment FROM duckdb_views() WHERE database_name = current_database() AND schema_name = ? AND view_name = ?"
    };
    if let Some(c) = texts(conn, table_comment, &[ns, name], 1)?
        .into_iter()
        .next()
        .and_then(|r| r.into_iter().next().flatten())
    {
        notes.push(format!("COMMENT ON {what} {q} IS {};", literal(&c)));
    }
    for r in texts(
        conn,
        "SELECT column_name, comment FROM duckdb_columns() WHERE database_name = current_database() AND schema_name = ? AND table_name = ? AND comment IS NOT NULL ORDER BY column_index",
        &[ns, name],
        2,
    )? {
        if let [Some(col), Some(c)] = r.as_slice() {
            notes.push(format!(
                "COMMENT ON COLUMN {q}.{} IS {};",
                ident(Dialect::Duckdb, col),
                literal(c)
            ));
        }
    }
    if !notes.is_empty() {
        out.push_str("\n\n");
        out.push_str(&notes.join("\n"));
    }
    Ok(())
}

fn routine(conn: &duckdb::Connection, object: &ObjectRef) -> Result<String> {
    let found = texts(
        conn,
        "SELECT function_type, array_to_string(parameters, ', '), macro_definition
         FROM duckdb_functions()
         WHERE database_name = current_database() AND schema_name = ? AND function_name = ?
           AND function_type IN ('macro', 'table_macro') AND NOT internal
         ORDER BY function_oid",
        &[&object.namespace, &object.name],
        3,
    )?;
    let pick = found
        .iter()
        .find(|r| {
            object
                .signature
                .as_deref()
                .is_none_or(|s| r.get(1).cloned().flatten().unwrap_or_default() == s)
        })
        .or_else(|| found.first())
        .ok_or_else(|| object.not_found())?;
    let text = |i: usize| pick.get(i).cloned().flatten().unwrap_or_default();
    let table = if text(0) == "table_macro" {
        "TABLE "
    } else {
        ""
    };
    Ok(format!(
        "CREATE MACRO {}({}) AS {table}{};",
        qualified(&object.namespace, &object.name),
        text(1),
        text(2).trim().trim_end_matches(';')
    ))
}

fn spread(sql: &str) -> String {
    let mut out = String::with_capacity(sql.len() + 64);
    let mut depth = 0usize;
    let mut quote: Option<char> = None;
    for c in sql.chars() {
        if let Some(q) = quote {
            out.push(c);
            if c == q {
                quote = None;
            }
            continue;
        }
        match c {
            '\'' | '"' => {
                quote = Some(c);
                out.push(c);
            }
            '(' => {
                depth += 1;
                out.push(c);
                if depth == 1 {
                    out.push_str("\n    ");
                }
            }
            ')' => {
                if depth == 1 {
                    out.push('\n');
                }
                depth = depth.saturating_sub(1);
                out.push(c);
            }
            ',' if depth == 1 => out.push_str(",\n   "),
            _ => out.push(c),
        }
    }
    out
}

const DEPS: &str = "SELECT 'object', 'table', schema_name, table_name, NULL, sql FROM duckdb_tables()
     WHERE database_name = current_database()
     UNION ALL
     SELECT 'object', 'view', schema_name, view_name, NULL, sql FROM duckdb_views()
     WHERE database_name = current_database() AND NOT internal
     UNION ALL
     SELECT 'object', 'routine', schema_name, function_name, NULL, macro_definition FROM duckdb_functions()
     WHERE database_name = current_database() AND NOT internal AND function_type IN ('macro', 'table_macro')
     UNION ALL
     SELECT 'fk', schema_name, table_name, schema_name, referenced_table, array_to_string(constraint_column_names, ', ')
     FROM duckdb_constraints()
     WHERE database_name = current_database() AND constraint_type = 'FOREIGN KEY' AND referenced_table IS NOT NULL";

pub(crate) fn dependencies(
    conn: &duckdb::Connection,
    object: &ObjectRef,
) -> Result<Vec<nd_driver::Dependency>> {
    let rows = texts(conn, DEPS, &[], 6)?;
    let views: Vec<String> = rows
        .iter()
        .filter(|r| r.first().and_then(Option::as_deref) == Some("object"))
        .filter(|r| r.get(1).and_then(Option::as_deref) == Some("view"))
        .filter_map(|r| r.get(3).cloned().flatten())
        .collect();
    let tables: Vec<String> = rows
        .iter()
        .filter(|r| r.get(1).and_then(Option::as_deref) == Some("table"))
        .filter_map(|r| r.get(3).cloned().flatten())
        .collect();
    let bodies: std::collections::HashMap<String, String> = rows
        .iter()
        .filter(|r| r.get(1).and_then(Option::as_deref) == Some("view"))
        .filter_map(|r| Some((r.get(3).cloned().flatten()?, r.get(5).cloned().flatten()?)))
        .collect();
    let known: Vec<String> = tables.iter().chain(&views).cloned().collect();
    let read = |view: &str| view_reads(conn, bodies.get(view)?, &known);
    Ok(nd_driver::views_from_engine(
        object,
        nd_driver::by_text(object, &rows),
        &views,
        read,
    ))
}

fn view_query(sql: &str) -> Option<&str> {
    let upper = sql.to_ascii_uppercase();
    let mut from = 0;
    while let Some(at) = upper[from..].find(" AS ") {
        let start = from + at + 4;
        let rest = upper[start..].trim_start();
        if ["SELECT", "WITH", "FROM", "VALUES", "("]
            .iter()
            .any(|k| rest.starts_with(k))
        {
            return Some(sql[start..].trim().trim_end_matches(';'));
        }
        from = start;
    }
    None
}

fn view_reads(conn: &duckdb::Connection, sql: &str, tables: &[String]) -> Option<Vec<String>> {
    fn walk(v: &serde_json::Value, out: &mut Vec<String>) {
        match v {
            serde_json::Value::Object(map) => {
                if map.get("type").and_then(serde_json::Value::as_str) == Some("BASE_TABLE")
                    && let Some(name) = map.get("table_name").and_then(serde_json::Value::as_str)
                {
                    out.push(name.to_owned());
                }
                map.values().for_each(|x| walk(x, out));
            }
            serde_json::Value::Array(items) => items.iter().for_each(|x| walk(x, out)),
            _ => {}
        }
    }
    let query = view_query(sql)?;
    let parsed = texts(conn, "SELECT json_serialize_sql(?::VARCHAR)", &[query], 1).ok()?;
    let json: serde_json::Value =
        serde_json::from_str(parsed.first()?.first()?.as_deref()?).ok()?;
    if json.get("error").and_then(serde_json::Value::as_bool) == Some(true) {
        return None;
    }
    let mut out = Vec::new();
    walk(&json, &mut out);
    out.retain(|n| tables.iter().any(|t| t.eq_ignore_ascii_case(n)));
    out.sort();
    out.dedup();
    Some(out)
}
