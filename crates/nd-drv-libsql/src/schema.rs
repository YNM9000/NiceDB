use std::collections::BTreeMap;

use nd_driver::{
    ColumnInfo, FkAction, ForeignKey, Index, Namespace, Object, Relation, Result, Row, Schema,
};

use crate::{map_error, query_rows};

const OBJECTS: &str = "m.type IN ('table', 'view') AND m.name NOT LIKE 'sqlite\\_%' ESCAPE '\\' \
                       AND m.name NOT LIKE 'libsql\\_%' ESCAPE '\\' AND m.name NOT LIKE '\\_litestream%' ESCAPE '\\'";

async fn q(conn: &libsql::Connection, sql: &str) -> Result<Vec<Row>> {
    query_rows(conn, sql, &[]).await.map_err(|e| map_error(&e))
}

fn text(row: &Row, i: usize) -> String {
    row.get(i).cloned().flatten().unwrap_or_default()
}

type ByTable<T> = BTreeMap<String, Vec<T>>;

async fn load_columns(conn: &libsql::Connection) -> Result<ByTable<ColumnInfo>> {
    let mut columns: BTreeMap<String, Vec<ColumnInfo>> = BTreeMap::new();
    for r in q(
        conn,
        &format!(
            "SELECT m.name, p.name, p.type, p.\"notnull\", p.pk
             FROM sqlite_schema m JOIN pragma_table_info(m.name) p
             WHERE {OBJECTS} ORDER BY m.name, p.cid"
        ),
    )
    .await?
    {
        columns.entry(text(&r, 0)).or_default().push(ColumnInfo {
            name: text(&r, 1),
            type_name: text(&r, 2),
            nullable: text(&r, 3) == "0",
            primary_key: text(&r, 4) != "0",
        });
    }
    Ok(columns)
}

async fn load_indexes(conn: &libsql::Connection) -> Result<ByTable<Index>> {
    let mut indexes: BTreeMap<String, Vec<Index>> = BTreeMap::new();
    for r in q(
        conn,
        &format!(
            "SELECT m.name, il.name, il.\"unique\", ii.name
             FROM sqlite_schema m JOIN pragma_index_list(m.name) il JOIN pragma_index_info(il.name) ii
             WHERE {OBJECTS} ORDER BY m.name, il.name, ii.seqno"
        ),
    )
    .await?
    {
        let list = indexes.entry(text(&r, 0)).or_default();
        let name = text(&r, 1);
        if list.last().is_none_or(|i| i.name != name) {
            list.push(Index {
                name,
                columns: Vec::new(),
                unique: text(&r, 2) == "1",
            });
        }
        if let (Some(i), Some(c)) = (list.last_mut(), r.get(3).cloned().flatten()) {
            i.columns.push(c);
        }
    }
    Ok(indexes)
}

async fn load_foreign_keys(
    conn: &libsql::Connection,
    columns: &ByTable<ColumnInfo>,
) -> Result<ByTable<ForeignKey>> {
    let mut foreign_keys: BTreeMap<String, Vec<ForeignKey>> = BTreeMap::new();
    for r in q(
        conn,
        &format!(
            "SELECT m.name, fk.id, fk.\"table\", fk.\"from\", fk.\"to\", fk.on_update, fk.on_delete
             FROM sqlite_schema m JOIN pragma_foreign_key_list(m.name) fk
             WHERE {OBJECTS} ORDER BY m.name, fk.id, fk.seq"
        ),
    )
    .await?
    {
        let table = text(&r, 0);
        let name = format!("{table}_fk{}", text(&r, 1));
        let list = foreign_keys.entry(table).or_default();
        if list.last().is_none_or(|k| k.name != name) {
            list.push(ForeignKey {
                name,
                columns: Vec::new(),
                ref_namespace: "main".into(),
                ref_table: text(&r, 2),
                ref_columns: Vec::new(),
                on_delete: FkAction::parse(&text(&r, 6)),
                on_update: FkAction::parse(&text(&r, 5)),
            });
        }
        if let Some(k) = list.last_mut() {
            k.columns.push(text(&r, 3));
            k.ref_columns.push(text(&r, 4));
        }
    }
    for keys in foreign_keys.values_mut() {
        for k in keys {
            if k.ref_columns.iter().any(String::is_empty) {
                k.ref_columns = columns
                    .get(&k.ref_table)
                    .map(|c| {
                        c.iter()
                            .filter(|c| c.primary_key)
                            .map(|c| c.name.clone())
                            .collect()
                    })
                    .unwrap_or_default();
            }
        }
    }
    Ok(foreign_keys)
}

pub(crate) async fn load(conn: &libsql::Connection) -> Result<Schema> {
    let mut columns = load_columns(conn).await?;
    let mut indexes = load_indexes(conn).await?;
    let mut foreign_keys = load_foreign_keys(conn, &columns).await?;
    let mut tables = Vec::new();
    let mut views = Vec::new();
    for r in q(
        conn,
        &format!("SELECT m.name, m.type FROM sqlite_schema m WHERE {OBJECTS} ORDER BY m.name"),
    )
    .await?
    {
        let name = text(&r, 0);
        let relation = Relation {
            columns: columns.remove(&name).unwrap_or_default(),
            indexes: indexes.remove(&name).unwrap_or_default(),
            foreign_keys: foreign_keys.remove(&name).unwrap_or_default(),
            estimated_rows: None,
            detail: None,
            name,
        };
        if text(&r, 1) == "view" {
            views.push(relation);
        } else {
            tables.push(relation);
        }
    }
    let triggers = q(
        conn,
        "SELECT name, tbl_name FROM sqlite_schema WHERE type = 'trigger' ORDER BY name",
    )
    .await?
    .iter()
    .map(|r| Object {
        name: text(r, 0),
        detail: Some(text(r, 1)),
    })
    .collect();
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
