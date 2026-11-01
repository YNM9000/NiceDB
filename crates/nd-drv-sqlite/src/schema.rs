use nd_driver::{ColumnInfo, FkAction, ForeignKey, Index, Namespace, Object, Relation, Schema};
use std::collections::HashMap;

use rusqlite::Connection;

const OBJECTS: &str = "m.type IN ('table', 'view') AND m.name NOT LIKE 'sqlite_%'";

pub(crate) fn load(conn: &Connection) -> rusqlite::Result<Schema> {
    let mut stmt = conn.prepare(&format!(
        "SELECT m.name, m.type FROM sqlite_schema m WHERE {OBJECTS} ORDER BY m.name"
    ))?;
    let objects: Vec<(String, String)> = stmt
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?
        .collect::<Result<_, _>>()?;
    let mut columns = columns(conn)?;
    let mut indexes = indexes(conn)?;
    let mut foreign_keys = foreign_keys(conn)?;
    let mut tables = Vec::new();
    let mut views = Vec::new();
    for (name, kind) in objects {
        let relation = Relation {
            columns: columns.remove(&name).unwrap_or_default(),
            indexes: indexes.remove(&name).unwrap_or_default(),
            foreign_keys: foreign_keys.remove(&name).unwrap_or_default(),
            estimated_rows: None,
            detail: None,
            name,
        };
        if kind == "view" {
            views.push(relation);
        } else {
            tables.push(relation);
        }
    }
    let triggers = conn
        .prepare("SELECT name, tbl_name FROM sqlite_schema WHERE type = 'trigger' ORDER BY name")?
        .query_map([], |r| {
            Ok(Object {
                name: r.get(0)?,
                detail: r.get(1)?,
            })
        })?
        .collect::<Result<_, _>>()?;
    Ok(Schema {
        databases: vec!["main".into()],
        current_database: "main".into(),
        namespaces: vec![Namespace {
            tables,
            views,
            triggers,
            materialized_views: Vec::new(),
            dictionaries: Vec::new(),
            packages: Vec::new(),
            synonyms: Vec::new(),
            aliases: Vec::new(),
            data_streams: Vec::new(),
            ..Namespace::new("main")
        }],
    })
}

fn columns(conn: &Connection) -> rusqlite::Result<HashMap<String, Vec<ColumnInfo>>> {
    let mut stmt = conn.prepare(&format!(
        "SELECT m.name, p.name, p.type, p.\"notnull\", p.pk FROM sqlite_schema m, pragma_table_info(m.name) p WHERE {OBJECTS} ORDER BY m.name, p.cid"
    ))?;
    let mut out: HashMap<String, Vec<ColumnInfo>> = HashMap::new();
    let mut rows = stmt.query([])?;
    while let Some(r) = rows.next()? {
        out.entry(r.get(0)?).or_default().push(ColumnInfo {
            name: r.get(1)?,
            type_name: r.get(2)?,
            nullable: r.get::<_, i64>(3)? == 0,
            primary_key: r.get::<_, i64>(4)? > 0,
        });
    }
    Ok(out)
}

fn indexes(conn: &Connection) -> rusqlite::Result<HashMap<String, Vec<Index>>> {
    let mut stmt = conn.prepare(&format!(
        "SELECT m.name, l.name, l.\"unique\", i.name FROM sqlite_schema m, pragma_index_list(m.name) l, pragma_index_info(l.name) i
         WHERE {OBJECTS} ORDER BY m.name, l.name, i.seqno"
    ))?;
    let mut out: HashMap<String, Vec<Index>> = HashMap::new();
    let mut rows = stmt.query([])?;
    while let Some(r) = rows.next()? {
        let list = out.entry(r.get(0)?).or_default();
        let name: String = r.get(1)?;
        if list.last().is_none_or(|i| i.name != name) {
            list.push(Index {
                name,
                columns: Vec::new(),
                unique: r.get::<_, i64>(2)? == 1,
            });
        }
        if let (Some(i), Some(column)) = (list.last_mut(), r.get::<_, Option<String>>(3)?) {
            i.columns.push(column);
        }
    }
    Ok(out)
}

fn foreign_keys(conn: &Connection) -> rusqlite::Result<HashMap<String, Vec<ForeignKey>>> {
    let mut stmt = conn.prepare(&format!(
        "SELECT m.name, f.id, f.\"table\", f.\"from\", f.\"to\", f.on_update, f.on_delete
         FROM sqlite_schema m, pragma_foreign_key_list(m.name) f WHERE {OBJECTS} ORDER BY m.name, f.id DESC, f.seq"
    ))?;
    let mut keys: HashMap<String, Vec<(i64, ForeignKey)>> = HashMap::new();
    let mut rows = stmt.query([])?;
    while let Some(r) = rows.next()? {
        let table: String = r.get(0)?;
        let id: i64 = r.get(1)?;
        let list = keys.entry(table.clone()).or_default();
        if list.last().is_none_or(|(last, _)| *last != id) {
            let on_update: String = r.get(5)?;
            let on_delete: String = r.get(6)?;
            list.push((
                id,
                ForeignKey {
                    name: format!("{table}_fk{id}"),
                    columns: Vec::new(),
                    ref_namespace: "main".into(),
                    ref_columns: Vec::new(),
                    ref_table: r.get(2)?,
                    on_delete: FkAction::parse(&on_delete),
                    on_update: FkAction::parse(&on_update),
                },
            ));
        }
        if let Some((_, k)) = list.last_mut() {
            k.columns.push(r.get(3)?);
            k.ref_columns
                .push(r.get::<_, Option<String>>(4)?.unwrap_or_default());
        }
    }
    keys.into_iter()
        .map(|(table, list)| {
            let list = list
                .into_iter()
                .map(|(_, mut k)| {
                    if k.ref_columns.iter().any(String::is_empty) {
                        k.ref_columns = primary_key(conn, &k.ref_table)?;
                    }
                    Ok(k)
                })
                .collect::<rusqlite::Result<_>>()?;
            Ok((table, list))
        })
        .collect()
}

fn primary_key(conn: &Connection, table: &str) -> rusqlite::Result<Vec<String>> {
    conn.prepare("SELECT name FROM pragma_table_info(?1) WHERE pk > 0 ORDER BY pk")?
        .query_map([table], |r| r.get(0))?
        .collect()
}
