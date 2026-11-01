use mysql_async::Conn;
use mysql_async::prelude::Queryable;
use nd_driver::{DdlKind, Dependency, Dialect, ObjectRef, Result, ident, mentions, tidy};

use crate::error::map_error;

const LINKS: &str = "SELECT 'uses', 'foreign_key', 'table', UNIQUE_CONSTRAINT_SCHEMA, REFERENCED_TABLE_NAME, CONSTRAINT_NAME
  FROM information_schema.REFERENTIAL_CONSTRAINTS WHERE CONSTRAINT_SCHEMA = ? AND TABLE_NAME = ?
UNION ALL
SELECT 'used_by', 'foreign_key', 'table', CONSTRAINT_SCHEMA, TABLE_NAME, CONSTRAINT_NAME
  FROM information_schema.REFERENTIAL_CONSTRAINTS WHERE UNIQUE_CONSTRAINT_SCHEMA = ? AND REFERENCED_TABLE_NAME = ?
UNION ALL
SELECT 'used_by', 'trigger', 'trigger', TRIGGER_SCHEMA, TRIGGER_NAME, NULL
  FROM information_schema.TRIGGERS WHERE EVENT_OBJECT_SCHEMA = ? AND EVENT_OBJECT_TABLE = ?";

const VIEW_USAGE: &str = "SELECT 'uses', 'definition', IF(t.TABLE_TYPE = 'VIEW', 'view', 'table'), u.TABLE_SCHEMA, u.TABLE_NAME, NULL
  FROM information_schema.VIEW_TABLE_USAGE u
  LEFT JOIN information_schema.TABLES t ON t.TABLE_SCHEMA = u.TABLE_SCHEMA AND t.TABLE_NAME = u.TABLE_NAME
  WHERE u.VIEW_SCHEMA = ? AND u.VIEW_NAME = ?
UNION ALL
SELECT 'used_by', 'definition', 'view', u.VIEW_SCHEMA, u.VIEW_NAME, NULL
  FROM information_schema.VIEW_TABLE_USAGE u WHERE u.TABLE_SCHEMA = ? AND u.TABLE_NAME = ?";

const SYSTEM: &str = "('mysql', 'sys', 'information_schema', 'performance_schema')";

type Link = (String, String, String, String, String, Option<String>);

type Text = (
    Option<String>,
    Option<String>,
    Option<String>,
    Option<String>,
);

async fn texts(conn: &mut Conn, sql: &str, params: Vec<&str>) -> Result<Vec<Text>> {
    conn.exec(sql, params).await.map_err(|e| map_error(&e, sql))
}

pub(crate) async fn dependencies(conn: &mut Conn, object: &ObjectRef) -> Result<Vec<Dependency>> {
    let (ns, name) = (object.namespace.as_str(), object.name.as_str());
    let links: Vec<(String, String, String, String, String, Option<String>)> = conn
        .exec(LINKS, (ns, name, ns, name, ns, name))
        .await
        .map_err(|e| map_error(&e, LINKS))?;
    let mut out: Vec<Dependency> = links
        .into_iter()
        .filter_map(|(a, b, c, d, e, f)| {
            Dependency::from_row(&[Some(a), Some(b), Some(c), Some(d), Some(e), f])
        })
        .collect();
    let usage: Option<Vec<Link>> = conn.exec(VIEW_USAGE, (ns, name, ns, name)).await.ok();
    let views_known = usage.is_some();
    out.extend(
        usage
            .into_iter()
            .flatten()
            .filter_map(|(a, b, c, d, e, f)| {
                Dependency::from_row(&[Some(a), Some(b), Some(c), Some(d), Some(e), f])
            }),
    );
    let full = format!(
        "{}.{}",
        ident(Dialect::Mysql, ns),
        ident(Dialect::Mysql, name)
    );
    let bodies = format!(
        "SELECT 'view', TABLE_SCHEMA, TABLE_NAME, VIEW_DEFINITION FROM information_schema.VIEWS WHERE TABLE_SCHEMA NOT IN {SYSTEM}
         UNION ALL SELECT 'routine', ROUTINE_SCHEMA, ROUTINE_NAME, ROUTINE_DEFINITION FROM information_schema.ROUTINES WHERE ROUTINE_SCHEMA NOT IN {SYSTEM}
         UNION ALL SELECT 'trigger', TRIGGER_SCHEMA, TRIGGER_NAME, ACTION_STATEMENT FROM information_schema.TRIGGERS
           WHERE TRIGGER_SCHEMA NOT IN {SYSTEM} AND NOT (EVENT_OBJECT_SCHEMA = ? AND EVENT_OBJECT_TABLE = ?)"
    );
    let mut own = None;
    for (kind, schema, other, body) in texts(conn, &bodies, vec![ns, name]).await? {
        let (Some(kind), Some(schema), Some(other)) = (kind, schema, other) else {
            continue;
        };
        let body = body.unwrap_or_default();
        if schema == ns && other == name {
            own = Some(body);
            continue;
        }
        if views_known && kind == "view" {
            continue;
        }
        let named = if kind == "view" {
            body.contains(&full)
        } else {
            mentions(&body, name)
        };
        if named {
            out.extend(Dependency::from_row(&[
                Some("used_by".into()),
                Some("definition".into()),
                Some(kind),
                Some(schema),
                Some(other),
                None,
            ]));
        }
    }
    if object.kind == DdlKind::View
        && !views_known
        && let Some(own) = own
    {
        let tables = format!(
            "SELECT IF(TABLE_TYPE = 'VIEW', 'view', 'table'), TABLE_SCHEMA, TABLE_NAME, NULL FROM information_schema.TABLES WHERE TABLE_SCHEMA NOT IN {SYSTEM}"
        );
        for (kind, schema, other, _) in texts(conn, &tables, Vec::new()).await? {
            let (Some(kind), Some(schema), Some(other)) = (kind, schema, other) else {
                continue;
            };
            let spelled = format!(
                "{}.{}",
                ident(Dialect::Mysql, &schema),
                ident(Dialect::Mysql, &other)
            );
            if (schema != ns || other != name) && own.contains(&spelled) {
                out.extend(Dependency::from_row(&[
                    Some("uses".into()),
                    Some("definition".into()),
                    Some(kind),
                    Some(schema),
                    Some(other),
                    None,
                ]));
            }
        }
    }
    Ok(tidy(out))
}
