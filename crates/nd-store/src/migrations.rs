use rusqlite::Connection;

use crate::{Error, Result};

const MIGRATIONS: &[&str] = &[
    include_str!("../migrations/0001_settings.sql"),
    include_str!("../migrations/0002_connections.sql"),
    include_str!("../migrations/0003_audit.sql"),
    include_str!("../migrations/0004_folders.sql"),
    include_str!("../migrations/0005_workspaces.sql"),
    include_str!("../migrations/0006_databases.sql"),
    include_str!("../migrations/0007_library.sql"),
    include_str!("../migrations/0008_session.sql"),
    include_str!("../migrations/0009_params.sql"),
    include_str!("../migrations/0010_templates.sql"),
];

pub(crate) fn apply(conn: &mut Connection) -> Result<()> {
    let supported = u32::try_from(MIGRATIONS.len()).expect("migration count fits in u32");
    let current: u32 = conn.pragma_query_value(None, "user_version", |r| r.get(0))?;
    if current > supported {
        return Err(Error::TooNew {
            found: current,
            supported,
        });
    }
    for (version, sql) in (current + 1..).zip(&MIGRATIONS[current as usize..]) {
        let tx = conn.transaction()?;
        tx.execute_batch(sql)?;
        tx.pragma_update(None, "user_version", version)?;
        tx.commit()?;
    }
    Ok(())
}
