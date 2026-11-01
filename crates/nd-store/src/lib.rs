mod audit;
mod connections;
mod folders;
mod genprofiles;
mod library;
mod marks;
mod migrations;
mod prefs;
mod settings;
mod transfer;
mod vars;
mod visibility;
mod workspaces;

use std::path::Path;

use rusqlite::Connection;

pub use audit::AuditEntry;
pub use connections::{
    ConnectionProfile, Environment, HistoryEntry, SessionOptions, Template, now_ms,
};
pub use folders::{Folder, FolderMark, FolderTint, TreeNode};
pub use genprofiles::{GenProfiles, GenSaved};
pub use library::{NoteScope, ParamUse, SavedNote, Script, Snippet};
pub use marks::{Mark, MarkTarget, Marks};
pub use prefs::{
    ConfirmMode, DEFAULT_ROW_CAP, DateDisplay, DriverPrefs, EditorPrefs, NullDisplay,
    NumberDisplay, Preferences, ResultsPrefs, SafetyPrefs, UnlockMode,
};
pub use settings::{
    Appearance, ErLayout, Locale, Palette, SoundDetail, SoundTimbre, Style, TablePosition,
    ThemeMode, UiScale, Volume,
};
pub use transfer::{
    Applied, Conflict, ExportSelection, ImportChoice, ImportItem, ImportOutcome, ImportPreview,
    ItemKind, PACKAGE_VERSION, PLAIN_FORMAT, Package, PackedSecret, Resolution,
};
pub use vars::{VarError, expand, lookup_in, names, resolve, secret_like, valid_name};
pub use visibility::{PackedVisibility, Visibility};
pub use workspaces::{
    SavedDesign, SavedTab, SavedTable, TabView, Workspace, WorkspaceSession, Workspaces,
};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("database: {0}")]
    Sqlite(#[from] rusqlite::Error),
    #[error("serialization: {0}")]
    Json(#[from] serde_json::Error),
    #[error("a folder cannot move inside itself")]
    Cycle,
    #[error("not found: {0}")]
    Missing(String),
    #[error("store version {found} is newer than this build supports ({supported})")]
    TooNew { found: u32, supported: u32 },
    #[error("file: {0}")]
    Io(#[from] std::io::Error),
}

pub type Result<T> = std::result::Result<T, Error>;

pub struct Store {
    conn: Connection,
}

impl Store {
    pub fn open(path: &Path) -> Result<Self> {
        private_file(path)?;
        let store = Self::init(Connection::open(path)?)?;
        for suffix in ["-wal", "-shm"] {
            let mut side = path.as_os_str().to_owned();
            side.push(suffix);
            private_file(Path::new(&side))?;
        }
        Ok(store)
    }

    pub fn open_in_memory() -> Result<Self> {
        Self::init(Connection::open_in_memory()?)
    }

    fn init(mut conn: Connection) -> Result<Self> {
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "foreign_keys", true)?;
        migrations::apply(&mut conn)?;
        Ok(Self { conn })
    }
}

#[cfg(unix)]
pub fn private_file(path: &Path) -> std::io::Result<()> {
    use std::os::unix::fs::{OpenOptionsExt as _, PermissionsExt as _};
    match std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(path)
    {
        Ok(_) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
            std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))
        }
        Err(e) => Err(e),
    }
}

#[cfg(not(unix))]
pub fn private_file(_path: &Path) -> std::io::Result<()> {
    Ok(())
}
