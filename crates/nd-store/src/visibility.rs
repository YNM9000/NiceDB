use std::collections::BTreeMap;

use rusqlite::{OptionalExtension, params};
use serde::{Deserialize, Serialize};

use crate::{Result, Store};

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(default)]
pub struct Visibility {
    pub databases: BTreeMap<String, bool>,
    pub schemas: BTreeMap<String, BTreeMap<String, bool>>,
}

impl Visibility {
    pub fn is_default(&self) -> bool {
        self.databases.is_empty() && self.schemas.values().all(BTreeMap::is_empty)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PackedVisibility {
    pub connection_id: String,
    pub visibility: Visibility,
}

pub(crate) fn read(conn: &rusqlite::Connection, connection_id: &str) -> Result<Visibility> {
    let body: Option<String> = conn
        .query_row(
            "SELECT body FROM visibility WHERE connection_id = ?1",
            [connection_id],
            |r| r.get(0),
        )
        .optional()?;
    Ok(body
        .and_then(|b| serde_json::from_str(&b).ok())
        .unwrap_or_default())
}

pub(crate) fn write(
    conn: &rusqlite::Connection,
    connection_id: &str,
    visibility: &Visibility,
) -> Result<()> {
    if visibility.is_default() {
        conn.execute(
            "DELETE FROM visibility WHERE connection_id = ?1",
            [connection_id],
        )?;
        return Ok(());
    }
    conn.execute(
        "INSERT INTO visibility (connection_id, body) VALUES (?1, ?2)
         ON CONFLICT(connection_id) DO UPDATE SET body = excluded.body",
        params![connection_id, serde_json::to_string(visibility)?],
    )?;
    Ok(())
}

impl Store {
    pub fn visibility(&self, connection_id: &str) -> Result<Visibility> {
        read(&self.conn, connection_id)
    }

    pub fn set_visibility(&self, connection_id: &str, visibility: &Visibility) -> Result<()> {
        write(&self.conn, connection_id, visibility)
    }
}
