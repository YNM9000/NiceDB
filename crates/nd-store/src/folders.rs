use rusqlite::{Connection, OptionalExtension, Transaction, params};
use serde::{Deserialize, Serialize, de::DeserializeOwned};

use crate::{Error, Result, Store};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "lowercase")]
pub enum FolderTint {
    Ochre,
    Red,
    Green,
    Blue,
    Violet,
    Slate,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "lowercase")]
pub enum FolderMark {
    Star,
    Briefcase,
    Building,
    Flask,
    Server,
    Shield,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
pub struct Folder {
    pub id: String,
    pub parent: Option<String>,
    pub name: String,
    pub tint: Option<FolderTint>,
    pub mark: Option<FolderMark>,
    pub collapsed: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(tag = "kind", content = "id", rename_all = "lowercase")]
pub enum TreeNode {
    Folder(String),
    Connection(String),
}

pub(crate) fn enum_str<T: Serialize>(v: Option<T>) -> Option<String> {
    v.and_then(|v| serde_json::to_value(v).ok())
        .and_then(|v| v.as_str().map(str::to_owned))
}

pub(crate) fn enum_parse<T: DeserializeOwned>(s: Option<String>) -> Option<T> {
    s.and_then(|s| serde_json::from_value(serde_json::Value::String(s)).ok())
}

const SYNC_GROUPS: &str = "
WITH RECURSIVE root(id, top) AS (
    SELECT id, id FROM folders WHERE parent_id IS NULL
    UNION ALL
    SELECT f.id, root.top FROM folders f JOIN root ON f.parent_id = root.id
)
UPDATE connections SET group_name = coalesce(
    (SELECT t.name FROM root JOIN folders t ON t.id = root.top WHERE root.id = connections.folder_id), '')";

const DESCENDANTS: &str = "
WITH RECURSIVE sub(id) AS (
    SELECT ?1
    UNION ALL
    SELECT f.id FROM folders f JOIN sub ON f.parent_id = sub.id
)
SELECT 1 FROM sub WHERE id = ?2";

pub(crate) fn sync_groups(tx: &Connection) -> Result<()> {
    tx.execute(SYNC_GROUPS, [])?;
    Ok(())
}

fn siblings(tx: &Transaction<'_>, node: &TreeNode, parent: Option<&str>) -> Result<Vec<String>> {
    let sql = match node {
        TreeNode::Folder(_) => {
            "SELECT id FROM folders WHERE parent_id IS ?1 AND id <> ?2 ORDER BY position, name COLLATE NOCASE"
        }
        TreeNode::Connection(_) => {
            "SELECT id FROM connections WHERE folder_id IS ?1 AND id <> ?2 ORDER BY position, name COLLATE NOCASE"
        }
    };
    let id = match node {
        TreeNode::Folder(id) | TreeNode::Connection(id) => id,
    };
    let mut stmt = tx.prepare(sql)?;
    stmt.query_map(params![parent, id], |r| r.get(0))?
        .collect::<rusqlite::Result<_>>()
        .map_err(Into::into)
}

pub(crate) fn next_position(tx: &Connection, table: Table, parent: Option<&str>) -> Result<i64> {
    let sql = match table {
        Table::Folders => {
            "SELECT coalesce(max(position), 0) + 1 FROM folders WHERE parent_id IS ?1"
        }
        Table::Connections => {
            "SELECT coalesce(max(position), 0) + 1 FROM connections WHERE folder_id IS ?1"
        }
        Table::Workspaces => {
            "SELECT coalesce(max(position), 0) + 1 FROM workspaces WHERE ?1 IS NULL"
        }
    };
    Ok(tx.query_row(sql, [parent], |r| r.get(0))?)
}

pub(crate) fn insert_folder(tx: &Connection, f: &Folder) -> Result<()> {
    let position = next_position(tx, Table::Folders, f.parent.as_deref())?;
    tx.execute(
        "INSERT INTO folders (id, parent_id, name, tint, mark, collapsed, position) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        params![f.id, f.parent, f.name, enum_str(f.tint), enum_str(f.mark), f.collapsed, position],
    )?;
    Ok(())
}

#[derive(Clone, Copy)]
pub(crate) enum Table {
    Folders,
    Connections,
    Workspaces,
}

impl Store {
    pub fn folders(&self) -> Result<Vec<Folder>> {
        let mut stmt = self.conn.prepare_cached(
            "SELECT id, parent_id, name, tint, mark, collapsed FROM folders ORDER BY position, name COLLATE NOCASE",
        )?;
        stmt.query_map([], |r| {
            Ok(Folder {
                id: r.get(0)?,
                parent: r.get(1)?,
                name: r.get(2)?,
                tint: enum_parse(r.get(3)?),
                mark: enum_parse(r.get(4)?),
                collapsed: r.get(5)?,
            })
        })?
        .collect::<rusqlite::Result<_>>()
        .map_err(Into::into)
    }

    pub fn save_folder(&self, f: &Folder) -> Result<()> {
        let tx = self.conn.unchecked_transaction()?;
        let exists = tx
            .query_row("SELECT 1 FROM folders WHERE id = ?1", [&f.id], |_| Ok(()))
            .optional()?
            .is_some();
        if exists {
            tx.execute(
                "UPDATE folders SET name = ?2, tint = ?3, mark = ?4, collapsed = ?5 WHERE id = ?1",
                params![
                    f.id,
                    f.name,
                    enum_str(f.tint),
                    enum_str(f.mark),
                    f.collapsed
                ],
            )?;
        } else {
            insert_folder(&tx, f)?;
        }
        sync_groups(&tx)?;
        tx.commit()?;
        Ok(())
    }

    pub fn delete_folder(&self, id: &str) -> Result<bool> {
        let tx = self.conn.unchecked_transaction()?;
        let Some(parent) = tx
            .query_row("SELECT parent_id FROM folders WHERE id = ?1", [id], |r| {
                r.get::<_, Option<String>>(0)
            })
            .optional()?
        else {
            return Ok(false);
        };
        let children: Vec<String> = tx
            .prepare("SELECT id FROM folders WHERE parent_id = ?1 ORDER BY position")?
            .query_map([id], |r| r.get(0))?
            .collect::<rusqlite::Result<_>>()?;
        let members: Vec<String> = tx
            .prepare("SELECT id FROM connections WHERE folder_id = ?1 ORDER BY position")?
            .query_map([id], |r| r.get(0))?
            .collect::<rusqlite::Result<_>>()?;
        let first = next_position(&tx, Table::Folders, parent.as_deref())?;
        for (pos, child) in (first..).zip(&children) {
            tx.execute(
                "UPDATE folders SET parent_id = ?2, position = ?3 WHERE id = ?1",
                params![child, parent, pos],
            )?;
        }
        let first = next_position(&tx, Table::Connections, parent.as_deref())?;
        for (pos, c) in (first..).zip(&members) {
            tx.execute(
                "UPDATE connections SET folder_id = ?2, position = ?3 WHERE id = ?1",
                params![c, parent, pos],
            )?;
        }
        crate::workspaces::replace_folder(&tx, id, &children, &members)?;
        tx.execute("DELETE FROM folders WHERE id = ?1", [id])?;
        sync_groups(&tx)?;
        tx.commit()?;
        Ok(true)
    }

    pub fn move_node(&self, node: &TreeNode, parent: Option<&str>, index: usize) -> Result<()> {
        let tx = self.conn.unchecked_transaction()?;
        if let TreeNode::Folder(id) = node
            && let Some(p) = parent
            && tx
                .query_row(DESCENDANTS, params![id, p], |_| Ok(()))
                .optional()?
                .is_some()
        {
            return Err(Error::Cycle);
        }
        let mut order = siblings(&tx, node, parent)?;
        let (id, sql) = match node {
            TreeNode::Folder(id) => (
                id,
                "UPDATE folders SET parent_id = ?2, position = ?3 WHERE id = ?1",
            ),
            TreeNode::Connection(id) => (
                id,
                "UPDATE connections SET folder_id = ?2, position = ?3 WHERE id = ?1",
            ),
        };
        order.insert(index.min(order.len()), id.clone());
        let renumber = match node {
            TreeNode::Folder(_) => "UPDATE folders SET position = ?2 WHERE id = ?1",
            TreeNode::Connection(_) => "UPDATE connections SET position = ?2 WHERE id = ?1",
        };
        for (pos, other) in (1i64..).zip(&order) {
            if other == id {
                if tx.execute(sql, params![id, parent, pos])? == 0 {
                    return Err(Error::Missing(id.clone()));
                }
            } else {
                tx.execute(renumber, params![other, pos])?;
            }
        }
        sync_groups(&tx)?;
        tx.commit()?;
        Ok(())
    }
}
