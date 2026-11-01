use std::collections::BTreeMap;
use std::time::{SystemTime, UNIX_EPOCH};

use nd_driver::Target;
use rusqlite::{OptionalExtension, params};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::folders::{Table, next_position, sync_groups};
use crate::{Result, Store};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "lowercase")]
pub enum Environment {
    Local,
    Development,
    Staging,
    Production,
}

impl Environment {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Local => "local",
            Self::Development => "development",
            Self::Staging => "staging",
            Self::Production => "production",
        }
    }

    fn parse(s: &str) -> Self {
        match s {
            "local" => Self::Local,
            "development" => Self::Development,
            "staging" => Self::Staging,
            _ => Self::Production,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(default)]
pub struct SessionOptions {
    pub init: String,
    pub keepalive_secs: u32,
    pub reconnect: bool,
    pub idle_minutes: u32,
    pub row_limit: Option<u32>,
}

impl Default for SessionOptions {
    fn default() -> Self {
        Self {
            init: String::new(),
            keepalive_secs: 0,
            reconnect: true,
            idle_minutes: 0,
            row_limit: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, specta::Type)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum Template {
    #[default]
    None,
    Model,
    Follows {
        id: String,
        overrides: Vec<String>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
pub struct ConnectionProfile {
    pub id: String,
    pub name: String,
    pub folder: Option<String>,
    pub environment: Environment,
    pub color: Option<String>,
    pub target: Target,
    #[serde(default)]
    pub session: SessionOptions,
    #[serde(default)]
    pub template: Template,
    #[serde(default)]
    pub raw_fields: BTreeMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
pub struct HistoryEntry {
    #[specta(type = f64)]
    pub id: i64,
    pub connection_id: String,
    pub sql: String,
    #[specta(type = f64)]
    pub started_at: i64,
    #[specta(type = f64)]
    pub elapsed_ms: i64,
    #[specta(type = Option<f64>)]
    pub rows: Option<i64>,
    pub error: Option<String>,
}

pub fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| i64::try_from(d.as_millis()).unwrap_or(i64::MAX))
}

pub(crate) fn upsert(tx: &rusqlite::Connection, p: &ConnectionProfile) -> Result<()> {
    let position = next_position(tx, Table::Connections, p.folder.as_deref())?;
    tx.execute(
        "INSERT INTO connections (id, name, folder_id, environment, color, target, session, position, created_at, updated_at, template, raw_fields)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?9, ?10, ?11)
         ON CONFLICT(id) DO UPDATE SET name = excluded.name,
           position = CASE WHEN folder_id IS excluded.folder_id THEN position ELSE excluded.position END,
           folder_id = excluded.folder_id, environment = excluded.environment, color = excluded.color,
           target = excluded.target, session = excluded.session, updated_at = excluded.updated_at,
           template = excluded.template, raw_fields = excluded.raw_fields",
        params![
            p.id,
            p.name,
            p.folder,
            p.environment.as_str(),
            p.color,
            serde_json::to_string(&p.target)?,
            serde_json::to_string(&p.session)?,
            position,
            now_ms(),
            serde_json::to_string(&p.template)?,
            serde_json::to_string(&p.raw_fields)?
        ],
    )?;
    Ok(())
}

fn read_all(conn: &rusqlite::Connection) -> Result<Vec<ConnectionProfile>> {
    let mut stmt = conn.prepare_cached(
        "SELECT id, name, folder_id, environment, color, target, session, template, raw_fields FROM connections ORDER BY position, name COLLATE NOCASE",
    )?;
    let rows = stmt.query_map([], |r| {
        Ok((
            (
                r.get::<_, String>(0)?,
                r.get(1)?,
                r.get(2)?,
                r.get::<_, String>(3)?,
                r.get(4)?,
            ),
            (
                r.get::<_, String>(5)?,
                r.get::<_, String>(6)?,
                r.get::<_, String>(7)?,
                r.get::<_, String>(8)?,
            ),
        ))
    })?;
    rows.map(|row| {
        let ((id, name, folder, env, color), (target, session, template, raw)) = row?;
        Ok(ConnectionProfile {
            id,
            name,
            folder,
            environment: Environment::parse(&env),
            color,
            target: serde_json::from_str(&target)?,
            session: serde_json::from_str(&session)?,
            template: serde_json::from_str(&template).unwrap_or_default(),
            raw_fields: serde_json::from_str(&raw).unwrap_or_default(),
        })
    })
    .collect()
}

fn merge_fields(model: &Value, own: &Value, keys: impl Iterator<Item = String>) -> Value {
    let mut out = model.clone();
    if let (Value::Object(out_map), Value::Object(own_map)) = (&mut out, own) {
        for k in keys.filter(|k| k != "driver") {
            match own_map.get(&k) {
                Some(v) => {
                    out_map.insert(k, v.clone());
                }
                None => {
                    out_map.remove(&k);
                }
            }
        }
    }
    out
}

fn follow(
    model: &ConnectionProfile,
    own: &ConnectionProfile,
    overrides: &[String],
) -> Result<ConnectionProfile> {
    let top = |path: &str| path.split('.').next().unwrap_or_default().to_owned();
    let mine = |k: &str| overrides.iter().any(|o| o == k);
    let model_t = serde_json::to_value(&model.target)?;
    let own_t = serde_json::to_value(&own.target)?;
    let same_driver = model_t.get("driver") == own_t.get("driver");
    let target_keys = overrides
        .iter()
        .filter(|o| !o.starts_with("session.") && same_driver)
        .cloned();
    let target = merge_fields(&model_t, &own_t, target_keys);
    let session_keys = overrides
        .iter()
        .filter_map(|o| o.strip_prefix("session.").map(str::to_owned));
    let session = merge_fields(
        &serde_json::to_value(&model.session)?,
        &serde_json::to_value(&own.session)?,
        session_keys,
    );
    let mut raw: BTreeMap<String, String> = model
        .raw_fields
        .iter()
        .filter(|(k, _)| !(same_driver && mine(&top(k))))
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect();
    if same_driver {
        raw.extend(
            own.raw_fields
                .iter()
                .filter(|(k, _)| mine(&top(k)))
                .map(|(k, v)| (k.clone(), v.clone())),
        );
    }
    let mut out = own.clone();
    out.target = serde_json::from_value(target)?;
    out.session = serde_json::from_value(session)?;
    out.raw_fields = raw;
    Ok(out)
}

impl Store {
    pub fn connections(&self) -> Result<Vec<ConnectionProfile>> {
        read_all(&self.conn)
    }

    pub fn connection(&self, id: &str) -> Result<Option<ConnectionProfile>> {
        Ok(self.connections()?.into_iter().find(|c| c.id == id))
    }

    pub fn save_connection(&self, p: &ConnectionProfile) -> Result<()> {
        let tx = self.conn.unchecked_transaction()?;
        let all = read_all(&tx)?;
        let mut p = p.clone();
        if let Template::Follows { id, overrides } = &p.template {
            p = match all
                .iter()
                .find(|m| &m.id == id && m.template == Template::Model)
            {
                Some(model) if model.id != p.id => follow(model, &p, overrides)?,
                _ => ConnectionProfile {
                    template: Template::None,
                    ..p
                },
            };
        }
        upsert(&tx, &p)?;
        for f in all
            .iter()
            .filter(|f| matches!(&f.template, Template::Follows { id, .. } if id == &p.id))
        {
            let next = match &f.template {
                Template::Follows { overrides, .. } if p.template == Template::Model => {
                    follow(&p, f, overrides)?
                }
                _ => ConnectionProfile {
                    template: Template::None,
                    ..f.clone()
                },
            };
            upsert(&tx, &next)?;
        }
        sync_groups(&tx)?;
        tx.commit()?;
        Ok(())
    }

    pub fn delete_connection(&self, id: &str) -> Result<bool> {
        let tx = self.conn.unchecked_transaction()?;
        for f in read_all(&tx)?
            .into_iter()
            .filter(|f| matches!(&f.template, Template::Follows { id: m, .. } if m == id))
        {
            upsert(
                &tx,
                &ConnectionProfile {
                    template: Template::None,
                    ..f
                },
            )?;
        }
        let gone = tx.execute("DELETE FROM connections WHERE id = ?1", [id])? > 0;
        tx.commit()?;
        Ok(gone)
    }

    pub fn add_history(&self, e: &HistoryEntry) -> Result<i64> {
        self.conn.execute(
            "INSERT INTO history (connection_id, sql, started_at, elapsed_ms, rows, error) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![e.connection_id, e.sql, e.started_at, e.elapsed_ms, e.rows, e.error],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    pub fn history(
        &self,
        connection_id: Option<&str>,
        query: &str,
        limit: u32,
    ) -> Result<Vec<HistoryEntry>> {
        let pattern = format!(
            "%{}%",
            query
                .replace('\\', "\\\\")
                .replace('%', "\\%")
                .replace('_', "\\_")
        );
        let mut stmt = self.conn.prepare_cached(
            "SELECT id, connection_id, sql, started_at, elapsed_ms, rows, error FROM history
             WHERE (?1 IS NULL OR connection_id = ?1) AND sql LIKE ?2 ESCAPE '\\'
             ORDER BY started_at DESC, id DESC LIMIT ?3",
        )?;
        stmt.query_map(params![connection_id, pattern, limit], |r| {
            Ok(HistoryEntry {
                id: r.get(0)?,
                connection_id: r.get(1)?,
                sql: r.get(2)?,
                started_at: r.get(3)?,
                elapsed_ms: r.get(4)?,
                rows: r.get(5)?,
                error: r.get(6)?,
            })
        })?
        .collect::<rusqlite::Result<_>>()
        .map_err(Into::into)
    }

    pub fn connection_exists(&self, id: &str) -> Result<bool> {
        Ok(self
            .conn
            .query_row("SELECT 1 FROM connections WHERE id = ?1", [id], |_| Ok(()))
            .optional()?
            .is_some())
    }
}
