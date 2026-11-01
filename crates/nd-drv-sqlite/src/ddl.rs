use nd_driver::{DdlKind, DriverError, ObjectRef, Result};

pub const QUERY: &str = "SELECT type, sql FROM sqlite_schema
     WHERE sql IS NOT NULL AND ((type = ?2 AND name = ?1) OR (?3 = '1' AND type IN ('index', 'trigger') AND tbl_name = ?1 AND name <> ?1))
     ORDER BY type = ?2 AND name = ?1 DESC, type = 'index' DESC, name";

pub fn params(object: &ObjectRef) -> Result<[String; 3]> {
    let (kind, dependents) = match object.kind {
        DdlKind::Table => ("table", true),
        DdlKind::View => ("view", true),
        DdlKind::Trigger => ("trigger", false),
        DdlKind::Index => ("index", false),
        DdlKind::Routine | DdlKind::Sequence => {
            return Err(DriverError::query("SQLite has no such objects"));
        }
    };
    Ok([
        object.name.clone(),
        kind.to_owned(),
        if dependents { "1" } else { "0" }.to_owned(),
    ])
}

pub fn script(object: &ObjectRef, rows: Vec<(String, String)>) -> Result<String> {
    let [_, kind, _] = params(object)?;
    if rows.first().is_none_or(|(t, _)| *t != kind) {
        return Err(if object.kind == DdlKind::Index {
            DriverError::query(format!(
                "{} is built by a constraint of its table",
                object.name
            ))
        } else {
            object.not_found()
        });
    }
    Ok(rows
        .into_iter()
        .map(|(_, sql)| {
            let sql = sql.trim();
            if sql.ends_with(';') {
                sql.to_owned()
            } else {
                format!("{sql};")
            }
        })
        .collect::<Vec<_>>()
        .join("\n\n"))
}

pub(crate) fn load(conn: &rusqlite::Connection, object: &ObjectRef) -> Result<String> {
    let p = params(object)?;
    let rows = conn
        .prepare(QUERY)
        .and_then(|mut st| {
            st.query_map(rusqlite::params![p[0], p[1], p[2]], |r| {
                Ok((r.get(0)?, r.get(1)?))
            })?
            .collect::<rusqlite::Result<Vec<_>>>()
        })
        .map_err(crate::map_err)?;
    script(object, rows)
}

pub const DEPS: &str = "SELECT 'object', type, 'main', name, tbl_name, sql FROM sqlite_schema
     WHERE type IN ('table', 'view', 'trigger') AND name NOT LIKE 'sqlite\\_%' ESCAPE '\\'
     UNION ALL
     SELECT 'fk', 'main', m.name, 'main', f.\"table\", group_concat(f.\"from\", ', ')
       FROM sqlite_schema m JOIN pragma_foreign_key_list(m.name) f
      WHERE m.type = 'table' GROUP BY m.name, f.id";

pub(crate) fn dependencies(
    conn: &rusqlite::Connection,
    object: &ObjectRef,
) -> Result<Vec<nd_driver::Dependency>> {
    let rows = conn
        .prepare(DEPS)
        .and_then(|mut st| {
            st.query_map([], |r| {
                (0..6)
                    .map(|i| r.get::<_, Option<String>>(i))
                    .collect::<rusqlite::Result<Vec<_>>>()
            })?
            .collect::<rusqlite::Result<Vec<_>>>()
        })
        .map_err(crate::map_err)?;
    let views: Vec<String> = rows
        .iter()
        .filter(|r| r.first().and_then(Option::as_deref) == Some("object"))
        .filter(|r| r.get(1).and_then(Option::as_deref) == Some("view"))
        .filter_map(|r| r.get(3).cloned().flatten())
        .collect();
    let read = |view: &str| view_reads(conn, view);
    Ok(nd_driver::views_from_engine(
        object,
        nd_driver::by_text(object, &rows),
        &views,
        read,
    ))
}

fn view_reads(conn: &rusqlite::Connection, view: &str) -> Option<Vec<String>> {
    let roots: std::collections::HashMap<i64, String> = conn
        .prepare("SELECT rootpage, tbl_name FROM sqlite_schema WHERE type IN ('table', 'index') AND rootpage > 0")
        .and_then(|mut st| {
            st.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?
                .collect::<rusqlite::Result<_>>()
        })
        .ok()?;
    let sql = format!(
        "EXPLAIN SELECT * FROM {}",
        nd_driver::ident(nd_driver::Dialect::Sqlite, view)
    );
    let mut st = conn.prepare(&sql).ok()?;
    let opened: Vec<(String, i64, i64)> = st
        .query_map([], |r| Ok((r.get(1)?, r.get(3)?, r.get(4)?)))
        .ok()?
        .collect::<rusqlite::Result<_>>()
        .ok()?;
    let mut out: Vec<String> = opened
        .into_iter()
        .filter(|(op, _, db)| op == "OpenRead" && *db == 0)
        .filter_map(|(_, root, _)| roots.get(&root).cloned())
        .collect();
    out.sort();
    out.dedup();
    Some(out)
}
