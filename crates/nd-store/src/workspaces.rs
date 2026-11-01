use std::collections::BTreeMap;

use rusqlite::{Transaction, params};
use serde::{Deserialize, Serialize};

use nd_sql::ParamValue;

use crate::folders::{Table, next_position};
use crate::{Result, Store};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "lowercase")]
pub enum TabView {
    #[default]
    Query,
    Data,
    Schema,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
pub struct SavedTable {
    pub namespace: String,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
pub struct SavedTab {
    pub connection_id: String,
    pub title: String,
    #[serde(default)]
    pub text: String,
    #[serde(default)]
    pub view: TabView,
    #[serde(default)]
    pub table: Option<SavedTable>,
    #[serde(default)]
    pub database: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub params: Vec<ParamValue>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub design: Option<SavedDesign>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trail: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
pub struct SavedDesign {
    pub namespace: String,
    pub engine: String,
    pub origin: Option<String>,
    pub draft: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, specta::Type)]
#[serde(default)]
pub struct WorkspaceSession {
    pub tabs: Vec<SavedTab>,
    pub active: Option<u32>,
    pub split: f64,
    pub history_open: bool,
    pub widths: BTreeMap<String, u32>,
}

impl Default for WorkspaceSession {
    fn default() -> Self {
        Self {
            tabs: Vec::new(),
            active: None,
            split: 0.45,
            history_open: false,
            widths: BTreeMap::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, specta::Type)]
pub struct Workspace {
    pub id: String,
    pub name: String,
    pub folders: Vec<String>,
    pub connections: Vec<String>,
    pub session: WorkspaceSession,
    #[serde(default)]
    pub vars: BTreeMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, specta::Type)]
pub struct Workspaces {
    pub active: Option<String>,
    pub all: WorkspaceSession,
    pub list: Vec<Workspace>,
}

#[derive(Default, Serialize, Deserialize)]
#[serde(default)]
struct Body {
    folders: Vec<String>,
    connections: Vec<String>,
    session: WorkspaceSession,
    vars: BTreeMap<String, String>,
}

const ACTIVE: &str = "workspace_active";
const ALL: &str = "workspace_all";

pub(crate) fn read(tx: &rusqlite::Connection) -> Result<Vec<Workspace>> {
    let mut stmt = tx.prepare_cached(
        "SELECT id, name, body FROM workspaces ORDER BY position, name COLLATE NOCASE",
    )?;
    let rows = stmt.query_map([], |r| {
        Ok((
            r.get::<_, String>(0)?,
            r.get::<_, String>(1)?,
            r.get::<_, String>(2)?,
        ))
    })?;
    rows.map(|row| {
        let (id, name, body) = row?;
        let b: Body = serde_json::from_str(&body).unwrap_or_default();
        Ok(Workspace {
            id,
            name,
            folders: b.folders,
            connections: b.connections,
            session: b.session,
            vars: b.vars,
        })
    })
    .collect()
}

pub(crate) fn write(tx: &rusqlite::Connection, w: &Workspace, position: i64) -> Result<()> {
    let body = serde_json::to_string(&Body {
        folders: w.folders.clone(),
        connections: w.connections.clone(),
        session: w.session.clone(),
        vars: w.vars.clone(),
    })?;
    tx.execute(
        "INSERT INTO workspaces (id, name, position, body) VALUES (?1, ?2, ?3, ?4)
         ON CONFLICT(id) DO UPDATE SET name = excluded.name, body = excluded.body",
        params![w.id, w.name, position, body],
    )?;
    Ok(())
}

pub(crate) fn replace_folder(
    tx: &Transaction<'_>,
    folder: &str,
    folders: &[String],
    connections: &[String],
) -> Result<()> {
    for mut w in read(tx)? {
        let before = w.folders.len();
        w.folders.retain(|f| f != folder);
        if w.folders.len() == before {
            continue;
        }
        for f in folders {
            if !w.folders.contains(f) {
                w.folders.push(f.clone());
            }
        }
        for c in connections {
            if !w.connections.contains(c) {
                w.connections.push(c.clone());
            }
        }
        write(tx, &w, 0)?;
    }
    Ok(())
}

impl Store {
    pub fn workspaces(&self) -> Result<Workspaces> {
        let list = read(&self.conn)?;
        let active = self
            .get::<Option<String>>(ACTIVE)?
            .flatten()
            .filter(|id| list.iter().any(|w| &w.id == id));
        Ok(Workspaces {
            active,
            all: self.get(ALL)?.unwrap_or_default(),
            list,
        })
    }

    pub fn save_workspace(&self, w: &Workspace) -> Result<()> {
        let tx = self.conn.unchecked_transaction()?;
        let position = next_position(&tx, Table::Workspaces, None)?;
        write(&tx, w, position)?;
        tx.commit()?;
        Ok(())
    }

    pub fn delete_workspace(&self, id: &str) -> Result<bool> {
        let gone = self
            .conn
            .execute("DELETE FROM workspaces WHERE id = ?1", [id])?
            > 0;
        if self.get::<Option<String>>(ACTIVE)?.flatten().as_deref() == Some(id) {
            self.put(ACTIVE, &None::<String>)?;
        }
        Ok(gone)
    }

    pub fn save_session(&self, id: Option<&str>, session: &WorkspaceSession) -> Result<()> {
        let Some(id) = id else {
            return self.put(ALL, session);
        };
        let tx = self.conn.unchecked_transaction()?;
        if let Some(mut w) = read(&tx)?.into_iter().find(|w| w.id == id) {
            w.session = session.clone();
            write(&tx, &w, 0)?;
        }
        tx.commit()?;
        Ok(())
    }

    pub fn set_active_workspace(&self, id: Option<&str>) -> Result<()> {
        self.put(ACTIVE, &id)
    }

    pub fn variables_for(
        &self,
        connection_id: Option<&str>,
        folder: Option<&str>,
    ) -> Result<BTreeMap<String, String>> {
        let all = self.workspaces()?;
        if let Some(active) = &all.active {
            return Ok(all
                .list
                .into_iter()
                .find(|w| &w.id == active)
                .map(|w| w.vars)
                .unwrap_or_default());
        }
        let id = connection_id.unwrap_or_default();
        let folders = self.folders()?;
        let mut chain = Vec::new();
        let mut at = match self.connection(id)? {
            Some(c) => c.folder,
            None => folder.map(str::to_owned),
        };
        while let Some(f) = at.take() {
            if chain.contains(&f) {
                break;
            }
            at = folders
                .iter()
                .find(|x| x.id == f)
                .and_then(|x| x.parent.clone());
            chain.push(f);
        }
        Ok(all
            .list
            .into_iter()
            .find(|w| {
                w.connections.iter().any(|c| c == id) || w.folders.iter().any(|f| chain.contains(f))
            })
            .map(|w| w.vars)
            .unwrap_or_default())
    }
}
