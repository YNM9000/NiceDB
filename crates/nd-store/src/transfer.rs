use std::collections::{HashMap, HashSet};

use rusqlite::{Connection, OptionalExtension, params};
use serde::{Deserialize, Serialize};
use zeroize::Zeroize;

use crate::folders::{Table, enum_str, insert_folder, next_position, sync_groups};
use crate::genprofiles::KEY as GEN_PROFILES;
use crate::library::{PackedParam, insert_param, upsert_note, upsert_script, upsert_snippet};
use crate::visibility::{read as read_visibility, write as write_visibility};
use crate::workspaces::{read as read_workspaces, write as write_workspace};
use crate::{
    Appearance, ConnectionProfile, Folder, GenSaved, HistoryEntry, MarkTarget, Marks, NoteScope,
    PackedVisibility, Result, SavedNote, Script, Snippet, Store, Workspace,
};

pub const PACKAGE_VERSION: u32 = 1;
pub const PLAIN_FORMAT: &str = "nicedb-connections";

#[derive(Clone, Default, Serialize, Deserialize)]
pub struct PackedSecret {
    pub connection_id: String,
    pub password: Option<String>,
    pub ssh: Option<String>,
    #[serde(default, skip_serializing_if = "std::collections::BTreeMap::is_empty")]
    pub extra: std::collections::BTreeMap<String, String>,
}

impl std::fmt::Debug for PackedSecret {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let masked = |v: Option<&String>| v.map(|_| "***");
        f.debug_struct("PackedSecret")
            .field("connection_id", &self.connection_id)
            .field("password", &masked(self.password.as_ref()))
            .field("ssh", &masked(self.ssh.as_ref()))
            .field("extra", &self.extra.keys().collect::<Vec<_>>())
            .finish()
    }
}

impl Drop for PackedSecret {
    fn drop(&mut self) {
        self.password.zeroize();
        self.ssh.zeroize();
        for v in self.extra.values_mut() {
            v.zeroize();
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Package {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub format: Option<String>,
    pub version: u32,
    pub created_at: i64,
    pub folders: Vec<Folder>,
    pub connections: Vec<ConnectionProfile>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub secrets: Vec<PackedSecret>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub visibility: Vec<PackedVisibility>,
    pub workspaces: Vec<Workspace>,
    pub history: Vec<HistoryEntry>,
    pub settings: Option<Appearance>,
    pub scripts: Vec<Script>,
    pub snippets: Vec<Snippet>,
    pub notes: Vec<SavedNote>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub param_values: Vec<PackedParam>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub gen_profiles: Vec<GenSaved>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub marks: Option<Marks>,
}

#[derive(Debug, Clone, Default, Deserialize, specta::Type)]
#[allow(clippy::struct_excessive_bools)]
pub struct ExportSelection {
    pub folders: Vec<String>,
    pub connections: Vec<String>,
    pub workspaces: Vec<String>,
    pub history: bool,
    pub settings: bool,
    pub secrets: bool,
    pub library: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "lowercase")]
pub enum ItemKind {
    Folder,
    Connection,
    Workspace,
    Settings,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(tag = "reason", rename_all = "snake_case")]
pub enum Conflict {
    SameId { existing: String },
    SameName { existing: String },
    Replaces,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
pub struct ImportItem {
    pub kind: ItemKind,
    pub id: String,
    pub name: String,
    pub path: Vec<String>,
    pub conflict: Option<Conflict>,
    pub secret: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, specta::Type)]
pub struct ImportPreview {
    pub sealed: bool,
    #[specta(type = f64)]
    pub created_at: i64,
    pub items: Vec<ImportItem>,
    pub history: u32,
    pub scripts: u32,
    pub snippets: u32,
    pub notes: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "snake_case")]
pub enum Resolution {
    #[default]
    KeepBoth,
    Replace,
    Skip,
}

#[derive(Debug, Clone, Deserialize, specta::Type)]
pub struct ImportChoice {
    pub kind: ItemKind,
    pub id: String,
    pub resolution: Resolution,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, specta::Type)]
pub struct ImportOutcome {
    pub added: u32,
    pub replaced: u32,
    pub skipped: u32,
    pub history: u32,
    pub settings: bool,
    pub scripts: u32,
    pub snippets: u32,
    pub notes: u32,
    pub params: u32,
    pub profiles: u32,
    pub marks: u32,
}

#[derive(Debug, Default)]
pub struct Applied {
    pub outcome: ImportOutcome,
    pub connections: HashMap<String, String>,
}

fn mark_connection(t: &MarkTarget) -> Option<&str> {
    match t {
        MarkTarget::Table { connection_id, .. } | MarkTarget::Query { connection_id, .. } => {
            Some(connection_id)
        }
        MarkTarget::Item { .. } => None,
    }
}

fn depth(folders: &[Folder], f: &Folder) -> usize {
    let by: HashMap<&str, &Folder> = folders.iter().map(|f| (f.id.as_str(), f)).collect();
    let mut n = 0;
    let mut at = f.parent.as_deref();
    while let Some(p) = at.and_then(|p| by.get(p)) {
        n += 1;
        if n > folders.len() {
            break;
        }
        at = p.parent.as_deref();
    }
    n
}

fn top_down(folders: &[Folder]) -> Vec<&Folder> {
    let mut out: Vec<&Folder> = folders.iter().collect();
    out.sort_by_key(|f| depth(folders, f));
    out
}

fn path<'a>(folders: &'a [Folder], mut at: Option<&'a str>) -> Vec<String> {
    let by: HashMap<&str, &Folder> = folders.iter().map(|f| (f.id.as_str(), f)).collect();
    let mut out = Vec::new();
    while let Some(f) = at.and_then(|p| by.get(p)) {
        out.push(f.name.clone());
        if out.len() > folders.len() {
            break;
        }
        at = f.parent.as_deref();
    }
    out.reverse();
    out
}

fn same(a: &str, b: &str) -> bool {
    a.trim().to_lowercase() == b.trim().to_lowercase()
}

fn unique(name: &str, taken: impl Fn(&str) -> bool) -> String {
    if !taken(name) {
        return name.to_owned();
    }
    (2..10_000)
        .map(|n| format!("{name} ({n})"))
        .find(|c| !taken(c))
        .unwrap_or_else(|| name.to_owned())
}

fn new_id() -> String {
    uuid::Uuid::new_v4().to_string()
}

fn folder_conflict(existing: &[Folder], f: &Folder, parent: Option<&str>) -> Option<Conflict> {
    if let Some(e) = existing.iter().find(|e| e.id == f.id) {
        return Some(Conflict::SameId {
            existing: e.id.clone(),
        });
    }
    existing
        .iter()
        .find(|e| e.parent.as_deref() == parent && same(&e.name, &f.name))
        .map(|e| Conflict::SameName {
            existing: e.id.clone(),
        })
}

fn connection_conflict(existing: &[ConnectionProfile], c: &ConnectionProfile) -> Option<Conflict> {
    if let Some(e) = existing.iter().find(|e| e.id == c.id) {
        return Some(Conflict::SameId {
            existing: e.id.clone(),
        });
    }
    existing
        .iter()
        .find(|e| same(&e.name, &c.name))
        .map(|e| Conflict::SameName {
            existing: e.id.clone(),
        })
}

fn workspace_conflict(existing: &[Workspace], w: &Workspace) -> Option<Conflict> {
    if let Some(e) = existing.iter().find(|e| e.id == w.id) {
        return Some(Conflict::SameId {
            existing: e.id.clone(),
        });
    }
    existing
        .iter()
        .find(|e| same(&e.name, &w.name))
        .map(|e| Conflict::SameName {
            existing: e.id.clone(),
        })
}

fn existing_of(c: &Conflict) -> Option<&str> {
    match c {
        Conflict::SameId { existing } | Conflict::SameName { existing } => Some(existing),
        Conflict::Replaces => None,
    }
}

fn match_folders(existing: &[Folder], package: &[Folder]) -> HashMap<String, Option<Conflict>> {
    let mut landed: HashMap<String, String> = HashMap::new();
    let mut out = HashMap::new();
    for f in top_down(package) {
        let parent = f
            .parent
            .as_ref()
            .and_then(|p| landed.get(p))
            .map(String::as_str);
        let c = folder_conflict(existing, f, parent);
        if let Some(e) = c.as_ref().and_then(existing_of) {
            landed.insert(f.id.clone(), e.to_owned());
        }
        out.insert(f.id.clone(), c);
    }
    out
}

impl Store {
    pub fn snapshot(&self, sel: &ExportSelection) -> Result<Package> {
        let folders = self.folders()?;
        let mut keep: HashSet<&str> = folders
            .iter()
            .map(|f| f.id.as_str())
            .filter(|id| sel.folders.iter().any(|s| s == id))
            .collect();
        loop {
            let before = keep.len();
            for f in &folders {
                if f.parent.as_deref().is_some_and(|p| keep.contains(p)) {
                    keep.insert(&f.id);
                }
            }
            if keep.len() == before {
                break;
            }
        }
        let connections: Vec<ConnectionProfile> = self
            .connections()?
            .into_iter()
            .filter(|c| {
                sel.connections.contains(&c.id)
                    || c.folder.as_deref().is_some_and(|f| keep.contains(f))
            })
            .collect();
        let by: HashMap<&str, &Folder> = folders.iter().map(|f| (f.id.as_str(), f)).collect();
        let mut wanted: HashSet<&str> = keep.clone();
        let starts = connections
            .iter()
            .filter_map(|c| c.folder.as_deref())
            .chain(keep.iter().copied())
            .collect::<Vec<_>>();
        for start in starts {
            let mut at = by.get(start).and_then(|f| f.parent.as_deref());
            while let Some(p) = at {
                if !wanted.insert(p) {
                    break;
                }
                at = by.get(p).and_then(|f| f.parent.as_deref());
            }
            if by.contains_key(start) {
                wanted.insert(start);
            }
        }
        let picked: Vec<Folder> = folders
            .iter()
            .filter(|f| wanted.contains(f.id.as_str()))
            .cloned()
            .collect();
        let folders = top_down(&picked).into_iter().cloned().collect();
        let history = if sel.history {
            self.history_of(&connections)?
        } else {
            Vec::new()
        };
        let workspaces: Vec<Workspace> = read_workspaces(&self.conn)?
            .into_iter()
            .filter(|w| sel.workspaces.contains(&w.id))
            .collect();
        let (scripts, snippets, notes) = sel
            .library
            .then(|| self.library_snapshot(&connections, &workspaces))
            .transpose()?
            .unwrap_or_default();
        let (param_values, gen_profiles, marks) = sel
            .library
            .then(|| self.extras_snapshot(&connections))
            .transpose()?
            .unwrap_or_default();
        let mut visibility = Vec::new();
        for c in &connections {
            let v = read_visibility(&self.conn, &c.id)?;
            if !v.is_default() {
                visibility.push(PackedVisibility {
                    connection_id: c.id.clone(),
                    visibility: v,
                });
            }
        }
        Ok(Package {
            format: None,
            version: PACKAGE_VERSION,
            created_at: crate::now_ms(),
            folders,
            connections,
            secrets: Vec::new(),
            visibility,
            workspaces,
            history,
            settings: sel.settings.then(|| self.appearance()).transpose()?,
            scripts,
            snippets,
            notes,
            param_values,
            gen_profiles,
            marks,
        })
    }

    fn history_of(&self, connections: &[ConnectionProfile]) -> Result<Vec<HistoryEntry>> {
        let mut out = Vec::new();
        for c in connections {
            out.extend(self.history(Some(&c.id), "", u32::MAX)?);
        }
        Ok(out)
    }

    fn extras_snapshot(
        &self,
        connections: &[ConnectionProfile],
    ) -> Result<(Vec<PackedParam>, Vec<GenSaved>, Option<Marks>)> {
        let kept = |id: &str| connections.iter().any(|c| c.id == id);
        let profiles = self
            .gen_profiles()?
            .profiles
            .into_iter()
            .filter(|p| kept(&p.connection_id))
            .collect();
        let mut marks = self.marks()?;
        marks
            .marks
            .retain(|m| mark_connection(&m.target).is_none_or(kept));
        let marks = (!marks.marks.is_empty() || !marks.recent.is_empty()).then_some(marks);
        Ok((self.param_values()?, profiles, marks))
    }

    fn library_snapshot(
        &self,
        connections: &[ConnectionProfile],
        workspaces: &[Workspace],
    ) -> Result<(Vec<Script>, Vec<Snippet>, Vec<SavedNote>)> {
        let scripts = self
            .scripts()?
            .into_iter()
            .map(|mut s| {
                if s.connection_id
                    .as_ref()
                    .is_some_and(|id| !connections.iter().any(|c| &c.id == id))
                {
                    s.connection_id = None;
                }
                s
            })
            .collect();
        let notes = self
            .notes()?
            .into_iter()
            .filter(|n| match n.scope {
                NoteScope::Connection => connections.iter().any(|c| c.id == n.scope_id),
                NoteScope::Workspace => workspaces.iter().any(|w| w.id == n.scope_id),
            })
            .collect();
        Ok((scripts, self.snippets()?, notes))
    }

    pub fn preview_import(&self, p: &Package) -> Result<ImportPreview> {
        let folders = self.folders()?;
        let connections = self.connections()?;
        let workspaces = read_workspaces(&self.conn)?;
        let landed = match_folders(&folders, &p.folders);
        let mut items = Vec::new();
        for f in top_down(&p.folders) {
            items.push(ImportItem {
                kind: crate::ItemKind::Folder,
                id: f.id.clone(),
                name: f.name.clone(),
                path: path(&p.folders, f.parent.as_deref()),
                conflict: landed.get(&f.id).cloned().flatten(),
                secret: false,
            });
        }
        for c in &p.connections {
            items.push(ImportItem {
                kind: ItemKind::Connection,
                id: c.id.clone(),
                name: c.name.clone(),
                path: path(&p.folders, c.folder.as_deref()),
                conflict: connection_conflict(&connections, c),
                secret: p
                    .secrets
                    .iter()
                    .any(|s| s.connection_id == c.id && (s.password.is_some() || s.ssh.is_some())),
            });
        }
        for w in &p.workspaces {
            items.push(ImportItem {
                kind: ItemKind::Workspace,
                id: w.id.clone(),
                name: w.name.clone(),
                path: Vec::new(),
                conflict: workspace_conflict(&workspaces, w),
                secret: false,
            });
        }
        if p.settings.is_some() {
            items.push(ImportItem {
                kind: ItemKind::Settings,
                id: "settings".into(),
                name: String::new(),
                path: Vec::new(),
                conflict: Some(Conflict::Replaces),
                secret: false,
            });
        }
        Ok(ImportPreview {
            sealed: false,
            created_at: p.created_at,
            items,
            history: u32::try_from(p.history.len()).unwrap_or(u32::MAX),
            scripts: u32::try_from(p.scripts.len()).unwrap_or(u32::MAX),
            snippets: u32::try_from(p.snippets.len()).unwrap_or(u32::MAX),
            notes: u32::try_from(p.notes.len()).unwrap_or(u32::MAX),
        })
    }

    pub fn apply_import(&self, p: &Package, choices: &[ImportChoice]) -> Result<Applied> {
        let tx = self.conn.unchecked_transaction()?;
        let mut run = Importer {
            store: self,
            tx: &tx,
            choices,
            out: Applied::default(),
            folders: HashMap::new(),
            connections: HashMap::new(),
            workspaces: HashMap::new(),
        };
        run.folders(&p.folders)?;
        run.connections(&p.connections)?;
        run.visibility(&p.visibility)?;
        for w in &p.workspaces {
            run.workspace(w)?;
        }
        run.history(&p.history)?;
        run.library(p)?;
        run.extras(p)?;
        if let Some(a) = &p.settings {
            if run.choice(ItemKind::Settings, "settings") == Some(Resolution::Replace) {
                self.set_appearance(&Appearance {
                    setup_done: self.appearance()?.setup_done,
                    ..*a
                })?;
                run.out.outcome.settings = true;
            } else {
                run.out.outcome.skipped += 1;
            }
        }
        let out = run.out;
        sync_groups(&tx)?;
        tx.commit()?;
        Ok(out)
    }
}

struct Importer<'a> {
    store: &'a Store,
    tx: &'a Connection,
    choices: &'a [ImportChoice],
    out: Applied,
    folders: HashMap<String, String>,
    connections: HashMap<String, String>,
    workspaces: HashMap<String, String>,
}

impl Importer<'_> {
    fn choice(&self, kind: ItemKind, id: &str) -> Option<Resolution> {
        self.choices
            .iter()
            .find(|c| c.kind == kind && c.id == id)
            .map(|c| c.resolution)
    }

    fn count(&mut self, conflict: bool, resolution: Resolution) {
        if conflict && resolution == Resolution::Replace {
            self.out.outcome.replaced += 1;
        } else {
            self.out.outcome.added += 1;
        }
    }

    fn known(&self, table: Table, id: &str) -> Result<bool> {
        let sql = match table {
            Table::Folders => "SELECT 1 FROM folders WHERE id = ?1",
            Table::Connections => "SELECT 1 FROM connections WHERE id = ?1",
            Table::Workspaces => "SELECT 1 FROM workspaces WHERE id = ?1",
        };
        Ok(self
            .tx
            .query_row(sql, [id], |_| Ok(()))
            .optional()?
            .is_some())
    }

    fn folders(&mut self, folders: &[Folder]) -> Result<()> {
        for f in top_down(folders) {
            let existing = self.store.folders()?;
            let parent = f.parent.as_ref().and_then(|x| self.folders.get(x)).cloned();
            let conflict = folder_conflict(&existing, f, parent.as_deref());
            let resolution = self.choice(ItemKind::Folder, &f.id).unwrap_or_default();
            let landed = match (conflict.as_ref().and_then(existing_of), resolution) {
                (None, _) => {
                    insert_folder(
                        self.tx,
                        &Folder {
                            parent,
                            ..f.clone()
                        },
                    )?;
                    f.id.clone()
                }
                (Some(e), Resolution::Skip) => {
                    self.out.outcome.skipped += 1;
                    self.folders.insert(f.id.clone(), e.to_owned());
                    continue;
                }
                (Some(e), Resolution::Replace) => {
                    self.tx.execute(
                        "UPDATE folders SET name = ?2, tint = ?3, mark = ?4 WHERE id = ?1",
                        params![e, f.name, enum_str(f.tint), enum_str(f.mark)],
                    )?;
                    e.to_owned()
                }
                (Some(_), Resolution::KeepBoth) => {
                    let id = new_id();
                    let name = unique(&f.name, |n| {
                        existing
                            .iter()
                            .any(|e| e.parent == parent && same(&e.name, n))
                    });
                    insert_folder(
                        self.tx,
                        &Folder {
                            id: id.clone(),
                            parent,
                            name,
                            ..f.clone()
                        },
                    )?;
                    id
                }
            };
            self.count(conflict.is_some(), resolution);
            self.folders.insert(f.id.clone(), landed);
        }
        Ok(())
    }

    fn connections(&mut self, connections: &[ConnectionProfile]) -> Result<()> {
        let mut followers = Vec::new();
        for c in connections {
            let existing = self.store.connections()?;
            let folder = c.folder.as_ref().and_then(|x| self.folders.get(x)).cloned();
            let conflict = connection_conflict(&existing, c);
            let resolution = self.choice(ItemKind::Connection, &c.id).unwrap_or_default();
            let (id, name) = match (conflict.as_ref().and_then(existing_of), resolution) {
                (None, _) => (c.id.clone(), c.name.clone()),
                (Some(e), Resolution::Skip) => {
                    self.out.outcome.skipped += 1;
                    self.connections.insert(c.id.clone(), e.to_owned());
                    continue;
                }
                (Some(e), Resolution::Replace) => (e.to_owned(), c.name.clone()),
                (Some(_), Resolution::KeepBoth) => (
                    new_id(),
                    unique(&c.name, |n| existing.iter().any(|e| same(&e.name, n))),
                ),
            };
            self.count(conflict.is_some(), resolution);
            let environment = existing
                .iter()
                .find(|e| e.id == id && e.environment == crate::Environment::Production)
                .map_or(c.environment, |e| e.environment);
            let landed = ConnectionProfile {
                id: id.clone(),
                name,
                folder,
                environment,
                ..c.clone()
            };
            crate::connections::upsert(self.tx, &landed)?;
            if matches!(landed.template, crate::Template::Follows { .. }) {
                followers.push(landed);
            }
            self.connections.insert(c.id.clone(), id.clone());
            self.out.connections.insert(c.id.clone(), id);
        }
        for mut f in followers {
            if let crate::Template::Follows { id, .. } = &mut f.template
                && let Some(landed) = self.connections.get(id.as_str())
                && landed != id
            {
                landed.clone_into(id);
                crate::connections::upsert(self.tx, &f)?;
            }
        }
        Ok(())
    }

    fn visibility(&self, packed: &[PackedVisibility]) -> Result<()> {
        for p in packed {
            if let Some(id) = self.out.connections.get(&p.connection_id) {
                write_visibility(self.tx, id, &p.visibility)?;
            }
        }
        Ok(())
    }

    fn workspace(&mut self, w: &Workspace) -> Result<()> {
        let existing = read_workspaces(self.tx)?;
        let conflict = workspace_conflict(&existing, w);
        let resolution = self.choice(ItemKind::Workspace, &w.id).unwrap_or_default();
        let (id, name) = match (conflict.as_ref().and_then(existing_of), resolution) {
            (None, _) => (w.id.clone(), w.name.clone()),
            (Some(e), Resolution::Skip) => {
                self.out.outcome.skipped += 1;
                self.workspaces.insert(w.id.clone(), e.to_owned());
                return Ok(());
            }
            (Some(e), Resolution::Replace) => (e.to_owned(), w.name.clone()),
            (Some(_), Resolution::KeepBoth) => (
                new_id(),
                unique(&w.name, |n| existing.iter().any(|e| same(&e.name, n))),
            ),
        };
        self.workspaces.insert(w.id.clone(), id.clone());
        let mut folders = Vec::new();
        for f in &w.folders {
            let f = self.folders.get(f).unwrap_or(f);
            if self.known(Table::Folders, f)? && !folders.contains(f) {
                folders.push(f.clone());
            }
        }
        let mut connections = Vec::new();
        for c in &w.connections {
            let c = self.connections.get(c).unwrap_or(c);
            if self.known(Table::Connections, c)? && !connections.contains(c) {
                connections.push(c.clone());
            }
        }
        let mut session = w.session.clone();
        let active = session
            .active
            .and_then(|i| session.tabs.get(i as usize))
            .cloned();
        let mut tabs = Vec::new();
        for mut t in std::mem::take(&mut session.tabs) {
            if let Some(c) = self.connections.get(&t.connection_id) {
                t.connection_id.clone_from(c);
            }
            if self.known(Table::Connections, &t.connection_id)? {
                tabs.push(t);
            }
        }
        session.active = active
            .and_then(|a| tabs.iter().position(|t| t.title == a.title))
            .and_then(|i| u32::try_from(i).ok());
        session.tabs = tabs;
        self.count(conflict.is_some(), resolution);
        let position = next_position(self.tx, Table::Workspaces, None)?;
        write_workspace(
            self.tx,
            &Workspace {
                id,
                name,
                folders,
                connections,
                session,
                vars: w.vars.clone(),
            },
            position,
        )
    }

    fn known_connection(&self, id: &str) -> Result<Option<String>> {
        let id = self.connections.get(id).map_or(id, String::as_str);
        Ok(self.known(Table::Connections, id)?.then(|| id.to_owned()))
    }

    fn library(&mut self, p: &Package) -> Result<()> {
        for s in &p.scripts {
            if self.known_row("scripts", &s.id)? {
                continue;
            }
            let connection_id = match &s.connection_id {
                Some(c) => self.known_connection(c)?,
                None => None,
            };
            upsert_script(
                self.tx,
                &Script {
                    connection_id,
                    ..s.clone()
                },
            )?;
            self.out.outcome.scripts += 1;
        }
        for s in &p.snippets {
            if self.known_row("snippets", &s.id)? {
                continue;
            }
            upsert_snippet(self.tx, s)?;
            self.out.outcome.snippets += 1;
        }
        for n in &p.notes {
            let scope_id = match n.scope {
                NoteScope::Connection => self.known_connection(&n.scope_id)?,
                NoteScope::Workspace => {
                    let id = self
                        .workspaces
                        .get(&n.scope_id)
                        .map_or(n.scope_id.as_str(), String::as_str);
                    self.known(Table::Workspaces, id)?.then(|| id.to_owned())
                }
            };
            let Some(scope_id) = scope_id else { continue };
            let taken = self
                .tx
                .query_row(
                    "SELECT 1 FROM notes WHERE scope = ?1 AND scope_id = ?2",
                    params![n.scope.as_str(), scope_id],
                    |_| Ok(()),
                )
                .optional()?
                .is_some();
            if !taken {
                upsert_note(
                    self.tx,
                    &SavedNote {
                        scope_id,
                        ..n.clone()
                    },
                )?;
                self.out.outcome.notes += 1;
            }
        }
        Ok(())
    }

    fn extras(&mut self, p: &Package) -> Result<()> {
        for v in &p.param_values {
            if insert_param(self.tx, v)? {
                self.out.outcome.params += 1;
            }
        }
        if !p.gen_profiles.is_empty() {
            let mut all = self.store.gen_profiles()?;
            for g in &p.gen_profiles {
                let Some(connection_id) = self.known_connection(&g.connection_id)? else {
                    continue;
                };
                if all.profiles.iter().any(|x| x.id == g.id) {
                    continue;
                }
                all.profiles.push(GenSaved {
                    connection_id,
                    ..g.clone()
                });
                self.out.outcome.profiles += 1;
            }
            self.store.put(GEN_PROFILES, &all)?;
        }
        if let Some(marks) = &p.marks {
            let mut all = self.store.marks()?;
            for m in &marks.marks {
                if all.marks.iter().any(|x| x.id == m.id) {
                    continue;
                }
                let mut m = m.clone();
                match &mut m.target {
                    MarkTarget::Table { connection_id, .. }
                    | MarkTarget::Query { connection_id, .. } => {
                        let Some(here) = self.known_connection(connection_id)? else {
                            continue;
                        };
                        *connection_id = here;
                    }
                    MarkTarget::Item { .. } => {}
                }
                all.marks.push(m);
                self.out.outcome.marks += 1;
            }
            for r in &marks.recent {
                if !all.recent.contains(r) {
                    all.recent.push(r.clone());
                }
            }
            self.store.set_marks(&all)?;
        }
        Ok(())
    }

    fn known_row(&self, table: &str, id: &str) -> Result<bool> {
        Ok(self
            .tx
            .query_row(
                &format!("SELECT 1 FROM {table} WHERE id = ?1"),
                [id],
                |_| Ok(()),
            )
            .optional()?
            .is_some())
    }

    fn history(&mut self, history: &[HistoryEntry]) -> Result<()> {
        for h in history {
            let Some(id) = self.out.connections.get(&h.connection_id) else {
                continue;
            };
            let seen = self
                .tx
                .query_row(
                    "SELECT 1 FROM history WHERE connection_id = ?1 AND started_at = ?2 AND sql = ?3",
                    params![id, h.started_at, h.sql],
                    |_| Ok(()),
                )
                .optional()?
                .is_some();
            if !seen {
                self.tx.execute(
                    "INSERT INTO history (connection_id, sql, started_at, elapsed_ms, rows, error) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                    params![id, h.sql, h.started_at, h.elapsed_ms, h.rows, h.error],
                )?;
                self.out.outcome.history += 1;
            }
        }
        Ok(())
    }
}
