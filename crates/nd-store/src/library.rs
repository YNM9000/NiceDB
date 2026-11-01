use rusqlite::{OptionalExtension, Row, params};
use serde::{Deserialize, Serialize};

use nd_sql::{ParamKind, ParamValue};

use crate::{Result, Store, now_ms};

const KEPT_VALUES: i64 = 12;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
pub struct ParamUse {
    pub name: String,
    pub kind: ParamKind,
    pub value: String,
    #[specta(type = f64)]
    pub used_at: i64,
    pub here: bool,
}

fn kind_name(kind: ParamKind) -> String {
    serde_json::to_value(kind)
        .ok()
        .and_then(|v| v.as_str().map(str::to_owned))
        .unwrap_or_default()
}

fn kind_of(name: &str) -> ParamKind {
    serde_json::from_value(serde_json::Value::String(name.to_owned())).unwrap_or(ParamKind::Text)
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PackedParam {
    pub query: String,
    pub name: String,
    pub kind: ParamKind,
    pub value: String,
    pub used_at: i64,
}

pub(crate) fn insert_param(conn: &rusqlite::Connection, p: &PackedParam) -> Result<bool> {
    let added = conn.execute(
        "INSERT INTO param_values (query, name, kind, value, used_at) VALUES (?1, ?2, ?3, ?4, ?5)
         ON CONFLICT(query, name, kind, value) DO UPDATE SET used_at = MAX(used_at, excluded.used_at)
         WHERE excluded.used_at > used_at",
        params![p.query, p.name, kind_name(p.kind), p.value, p.used_at],
    )?;
    Ok(added > 0)
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
pub struct Script {
    pub id: String,
    pub name: String,
    pub folder: String,
    pub tags: Vec<String>,
    pub connection_id: Option<String>,
    pub sql: String,
    #[serde(default)]
    pub params: Vec<ParamValue>,
    #[specta(type = f64)]
    pub created_at: i64,
    #[specta(type = f64)]
    pub updated_at: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
pub struct Snippet {
    pub id: String,
    pub name: String,
    pub trigger: String,
    pub description: String,
    pub body: String,
    pub dialect: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "lowercase")]
pub enum NoteScope {
    Connection,
    Workspace,
}

impl NoteScope {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Connection => "connection",
            Self::Workspace => "workspace",
        }
    }

    fn parse(s: &str) -> Self {
        if s == "workspace" {
            Self::Workspace
        } else {
            Self::Connection
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
pub struct SavedNote {
    pub scope: NoteScope,
    pub scope_id: String,
    pub body: String,
    #[specta(type = f64)]
    pub updated_at: i64,
}

fn script_from(r: &Row<'_>) -> rusqlite::Result<Script> {
    let tags: String = r.get(3)?;
    let params: String = r.get(8)?;
    Ok(Script {
        id: r.get(0)?,
        name: r.get(1)?,
        folder: r.get(2)?,
        tags: serde_json::from_str(&tags).unwrap_or_default(),
        connection_id: r.get(4)?,
        sql: r.get(5)?,
        params: serde_json::from_str(&params).unwrap_or_default(),
        created_at: r.get(6)?,
        updated_at: r.get(7)?,
    })
}

const SCRIPT_COLUMNS: &str =
    "id, name, folder, tags, connection_id, sql, created_at, updated_at, params";

pub(crate) fn upsert_script(conn: &rusqlite::Connection, s: &Script) -> Result<()> {
    conn.execute(
        "INSERT INTO scripts (id, name, folder, tags, connection_id, sql, created_at, updated_at, params)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
         ON CONFLICT(id) DO UPDATE SET name = excluded.name, folder = excluded.folder,
            tags = excluded.tags, connection_id = excluded.connection_id, sql = excluded.sql,
            updated_at = excluded.updated_at, params = excluded.params",
        params![
            s.id,
            s.name,
            s.folder,
            serde_json::to_string(&s.tags)?,
            s.connection_id,
            s.sql,
            s.created_at,
            s.updated_at,
            serde_json::to_string(&s.params)?,
        ],
    )?;
    Ok(())
}

pub(crate) fn upsert_snippet(conn: &rusqlite::Connection, s: &Snippet) -> Result<()> {
    conn.execute(
        "INSERT INTO snippets (id, name, trigger, description, body, dialect, position)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, (SELECT COALESCE(MAX(position), -1) + 1 FROM snippets))
         ON CONFLICT(id) DO UPDATE SET name = excluded.name, trigger = excluded.trigger,
            description = excluded.description, body = excluded.body, dialect = excluded.dialect",
        params![s.id, s.name, s.trigger, s.description, s.body, s.dialect],
    )?;
    Ok(())
}

pub(crate) fn upsert_note(conn: &rusqlite::Connection, n: &SavedNote) -> Result<()> {
    conn.execute(
        "INSERT INTO notes (scope, scope_id, body, updated_at) VALUES (?1, ?2, ?3, ?4)
         ON CONFLICT(scope, scope_id) DO UPDATE SET body = excluded.body, updated_at = excluded.updated_at",
        params![n.scope.as_str(), n.scope_id, n.body, n.updated_at],
    )?;
    Ok(())
}

impl Store {
    pub fn scripts(&self) -> Result<Vec<Script>> {
        let mut stmt = self.conn.prepare_cached(&format!(
            "SELECT {SCRIPT_COLUMNS} FROM scripts ORDER BY folder COLLATE NOCASE, name COLLATE NOCASE"
        ))?;
        let rows = stmt.query_map([], script_from)?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    pub fn script(&self, id: &str) -> Result<Option<Script>> {
        Ok(self
            .conn
            .query_row(
                &format!("SELECT {SCRIPT_COLUMNS} FROM scripts WHERE id = ?1"),
                [id],
                script_from,
            )
            .optional()?)
    }

    pub fn save_script(&self, s: &Script) -> Result<Script> {
        let created = self.script(&s.id)?.map_or_else(
            || {
                if s.created_at > 0 {
                    s.created_at
                } else {
                    now_ms()
                }
            },
            |old| old.created_at,
        );
        let saved = Script {
            created_at: created,
            updated_at: now_ms(),
            ..s.clone()
        };
        upsert_script(&self.conn, &saved)?;
        Ok(saved)
    }

    pub fn delete_script(&self, id: &str) -> Result<()> {
        self.conn
            .execute("DELETE FROM scripts WHERE id = ?1", [id])?;
        Ok(())
    }

    pub fn remember_params(&self, query: &str, values: &[ParamValue]) -> Result<()> {
        let now = now_ms();
        for v in values {
            self.conn.execute(
                "INSERT INTO param_values (query, name, kind, value, used_at) VALUES (?1, ?2, ?3, ?4, ?5)
                 ON CONFLICT(query, name, kind, value) DO UPDATE SET used_at = excluded.used_at",
                params![query, v.name, kind_name(v.kind), v.value, now],
            )?;
            self.conn.execute(
                "DELETE FROM param_values WHERE query = ?1 AND name = ?2 AND rowid NOT IN (
                    SELECT rowid FROM param_values WHERE query = ?1 AND name = ?2
                    ORDER BY used_at DESC LIMIT ?3)",
                params![query, v.name, KEPT_VALUES],
            )?;
        }
        Ok(())
    }

    pub fn param_values(&self) -> Result<Vec<PackedParam>> {
        let mut stmt = self.conn.prepare_cached(
            "SELECT query, name, kind, value, used_at FROM param_values ORDER BY used_at, rowid",
        )?;
        let rows = stmt.query_map([], |r| {
            let kind: String = r.get(2)?;
            Ok(PackedParam {
                query: r.get(0)?,
                name: r.get(1)?,
                kind: kind_of(&kind),
                value: r.get(3)?,
                used_at: r.get(4)?,
            })
        })?;
        Ok(rows.collect::<std::result::Result<_, _>>()?)
    }

    pub fn param_history(&self, query: &str, names: &[String]) -> Result<Vec<ParamUse>> {
        let mut stmt = self.conn.prepare_cached(
            "SELECT name, kind, value, MAX(used_at), MAX(query = ?1) FROM param_values
             WHERE name = ?2 GROUP BY kind, value
             ORDER BY MAX(query = ?1) DESC, MAX(used_at) DESC, MAX(rowid) DESC LIMIT ?3",
        )?;
        let mut out = Vec::new();
        for name in names {
            let rows = stmt.query_map(params![query, name, KEPT_VALUES], |r| {
                let kind: String = r.get(1)?;
                Ok(ParamUse {
                    name: r.get(0)?,
                    kind: kind_of(&kind),
                    value: r.get(2)?,
                    used_at: r.get(3)?,
                    here: r.get(4)?,
                })
            })?;
            for row in rows {
                out.push(row?);
            }
        }
        Ok(out)
    }

    pub fn snippets(&self) -> Result<Vec<Snippet>> {
        let mut stmt = self.conn.prepare_cached(
            "SELECT id, name, trigger, description, body, dialect FROM snippets ORDER BY position, name COLLATE NOCASE",
        )?;
        let rows = stmt.query_map([], |r| {
            Ok(Snippet {
                id: r.get(0)?,
                name: r.get(1)?,
                trigger: r.get(2)?,
                description: r.get(3)?,
                body: r.get(4)?,
                dialect: r.get(5)?,
            })
        })?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    pub fn save_snippet(&self, s: &Snippet) -> Result<()> {
        upsert_snippet(&self.conn, s)
    }

    pub fn delete_snippet(&self, id: &str) -> Result<()> {
        self.conn
            .execute("DELETE FROM snippets WHERE id = ?1", [id])?;
        Ok(())
    }

    pub fn notes(&self) -> Result<Vec<SavedNote>> {
        let mut stmt = self.conn.prepare_cached(
            "SELECT scope, scope_id, body, updated_at FROM notes ORDER BY updated_at DESC",
        )?;
        let rows = stmt.query_map([], |r| {
            let scope: String = r.get(0)?;
            Ok(SavedNote {
                scope: NoteScope::parse(&scope),
                scope_id: r.get(1)?,
                body: r.get(2)?,
                updated_at: r.get(3)?,
            })
        })?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    pub fn set_note(
        &self,
        scope: NoteScope,
        scope_id: &str,
        body: &str,
    ) -> Result<Option<SavedNote>> {
        if body.trim().is_empty() {
            self.conn.execute(
                "DELETE FROM notes WHERE scope = ?1 AND scope_id = ?2",
                params![scope.as_str(), scope_id],
            )?;
            return Ok(None);
        }
        let note = SavedNote {
            scope,
            scope_id: scope_id.to_owned(),
            body: body.to_owned(),
            updated_at: now_ms(),
        };
        upsert_note(&self.conn, &note)?;
        Ok(Some(note))
    }
}
