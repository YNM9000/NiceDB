use mysql_async::Conn;
use mysql_async::prelude::Queryable;
use nd_driver::{DdlKind, Dialect, ObjectRef, Result, ident};

use crate::error::map_error;

fn terminated(s: &str) -> String {
    let s = s.trim();
    if s.ends_with(';') {
        s.to_owned()
    } else {
        format!("{s};")
    }
}

fn qualified(ns: &str, name: &str) -> String {
    format!(
        "{}.{}",
        ident(Dialect::Mysql, ns),
        ident(Dialect::Mysql, name)
    )
}

async fn show(conn: &mut Conn, what: &str, ns: &str, name: &str, at: usize) -> Result<String> {
    let sql = format!("SHOW CREATE {what} {}", qualified(ns, name));
    let row: Option<mysql_async::Row> = conn
        .query_first(&sql)
        .await
        .map_err(|e| map_error(&e, &sql))?;
    row.and_then(|r| {
        r.get_opt::<Option<String>, _>(at)
            .and_then(std::result::Result::ok)
    })
    .flatten()
    .ok_or_else(|| nd_driver::DriverError::query(format!("{name}: definition not readable")))
}

pub(crate) async fn ddl(conn: &mut Conn, object: &ObjectRef) -> Result<String> {
    let ns = object.namespace.as_str();
    let name = object.name.as_str();
    match object.kind {
        DdlKind::Table => table(conn, ns, name).await,
        DdlKind::View => Ok(terminated(&show(conn, "VIEW", ns, name, 1).await?)),
        DdlKind::Sequence => Ok(terminated(&show(conn, "SEQUENCE", ns, name, 1).await?)),
        DdlKind::Trigger => Ok(terminated(&show(conn, "TRIGGER", ns, name, 2).await?)),
        DdlKind::Routine => {
            let sql = "SELECT routine_type FROM information_schema.routines WHERE routine_schema = ? AND routine_name = ? ORDER BY routine_type = 'FUNCTION' DESC LIMIT 1";
            let kind: Option<String> = conn
                .exec_first(sql, (ns, name))
                .await
                .map_err(|e| map_error(&e, sql))?;
            let kind = kind.ok_or_else(|| object.not_found())?;
            Ok(terminated(&show(conn, &kind, ns, name, 2).await?))
        }
        DdlKind::Index => index(conn, object).await,
    }
}

async fn table(conn: &mut Conn, ns: &str, name: &str) -> Result<String> {
    let mut out = terminated(&show(conn, "TABLE", ns, name, 1).await?);
    let sql = "SELECT trigger_name FROM information_schema.triggers WHERE trigger_schema = ? AND event_object_table = ? ORDER BY action_timing, event_manipulation, action_order";
    let triggers: Vec<String> = conn
        .exec(sql, (ns, name))
        .await
        .map_err(|e| map_error(&e, sql))?;
    for t in triggers {
        out.push_str("\n\n");
        out.push_str(&terminated(&show(conn, "TRIGGER", ns, &t, 2).await?));
    }
    Ok(out)
}

async fn index(conn: &mut Conn, object: &ObjectRef) -> Result<String> {
    let ns = object.namespace.as_str();
    let table = if let Some(t) = &object.table {
        t.clone()
    } else {
        let sql = "SELECT table_name FROM information_schema.statistics WHERE table_schema = ? AND index_name = ? LIMIT 1";
        let found: Option<String> = conn
            .exec_first(sql, (ns, &object.name))
            .await
            .map_err(|e| map_error(&e, sql))?;
        found.ok_or_else(|| object.not_found())?
    };
    let create = show(conn, "TABLE", ns, &table, 1).await?;
    let line = index_line(&create, &object.name).ok_or_else(|| object.not_found())?;
    Ok(format!("ALTER TABLE {} ADD {line};", qualified(ns, &table)))
}

fn index_line<'a>(create: &'a str, name: &str) -> Option<&'a str> {
    let key = format!("KEY {} ", ident(Dialect::Mysql, name));
    create
        .lines()
        .map(|l| l.trim().trim_end_matches(','))
        .find(|l| {
            if name == "PRIMARY" {
                l.starts_with("PRIMARY KEY ")
            } else {
                l.contains(&key) && !l.starts_with("CONSTRAINT ")
            }
        })
}
