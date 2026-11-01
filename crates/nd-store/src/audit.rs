use rusqlite::params;
use serde::{Deserialize, Serialize};

use crate::{Result, Store};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
pub struct AuditEntry {
    #[specta(type = f64)]
    pub id: i64,
    pub connection_id: String,
    #[serde(default)]
    pub database: String,
    #[specta(type = f64)]
    pub at: i64,
    pub who: String,
    pub sql: String,
    #[specta(type = Option<f64>)]
    pub rows: Option<i64>,
    pub error: Option<String>,
}

impl Store {
    pub fn add_audit(&self, e: &AuditEntry) -> Result<i64> {
        self.conn.execute(
            "INSERT INTO audit (connection_id, database, at, who, sql, rows, error) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![e.connection_id, e.database, e.at, e.who, e.sql, e.rows, e.error],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    pub fn audit(&self, connection_id: &str, limit: u32) -> Result<Vec<AuditEntry>> {
        let mut stmt = self.conn.prepare_cached(
            "SELECT id, connection_id, database, at, who, sql, rows, error FROM audit
             WHERE connection_id = ?1 ORDER BY at DESC, id DESC LIMIT ?2",
        )?;
        stmt.query_map(params![connection_id, limit], |r| {
            Ok(AuditEntry {
                id: r.get(0)?,
                connection_id: r.get(1)?,
                database: r.get(2)?,
                at: r.get(3)?,
                who: r.get(4)?,
                sql: r.get(5)?,
                rows: r.get(6)?,
                error: r.get(7)?,
            })
        })?
        .collect::<rusqlite::Result<_>>()
        .map_err(Into::into)
    }
}
