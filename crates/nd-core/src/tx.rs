use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};

use nd_driver::{Chunk, Column, Connection, DriverError, Row};
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;

use crate::guard::{Access, claim, collect, quiet, timed};
use crate::session::Session;
use crate::{Core, CoreError, Result};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "snake_case")]
pub enum CommitMode {
    #[default]
    Auto,
    Manual,
    Smart,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct Tx {
    pub mode: CommitMode,
    pub began: Option<Instant>,
    pub writes: u32,
    pub rows: u64,
    pub failed: bool,
    pub dry_run: bool,
    pub confirmed: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
pub struct TxState {
    pub mode: CommitMode,
    pub open: bool,
    pub writes: u32,
    #[specta(type = f64)]
    pub rows: u64,
    #[specta(type = Option<f64>)]
    pub open_ms: Option<u64>,
    pub failed: bool,
    pub dry_run: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "snake_case")]
pub enum PreviewKind {
    Update,
    Delete,
}

#[derive(Debug, Clone, Serialize, specta::Type)]
pub struct RowPreview {
    pub kind: PreviewKind,
    pub columns: Vec<Column>,
    pub before: Vec<Row>,
    pub after: Vec<Row>,
    pub changed: Vec<u32>,
    pub truncated: bool,
    pub measured: bool,
}

pub(crate) const PREVIEW_ROWS: usize = 200;
const PREVIEW_LIMIT: Duration = Duration::from_secs(5);
const PREVIEW_SAVEPOINT: &str = "nd_preview";

impl Session {
    pub(crate) fn tx_open(&self) -> bool {
        self.in_transaction.load(Ordering::Acquire)
    }

    pub(crate) fn commit_mode(&self) -> CommitMode {
        self.tx.lock().map(|t| t.mode).unwrap_or_default()
    }

    pub(crate) fn tx_state(&self) -> TxState {
        let open = self.tx_open();
        let t = self.tx.lock().map(|t| t.clone()).unwrap_or_default();
        TxState {
            mode: t.mode,
            open,
            writes: if open { t.writes } else { 0 },
            rows: if open { t.rows } else { 0 },
            open_ms: t
                .began
                .filter(|_| open)
                .map(|b| u64::try_from(b.elapsed().as_millis()).unwrap_or(u64::MAX)),
            failed: open && t.failed,
            dry_run: open && t.dry_run,
        }
    }

    fn with_tx(&self, f: impl FnOnce(&mut Tx)) {
        if let Ok(mut t) = self.tx.lock() {
            f(&mut t);
        }
    }

    pub(crate) fn began(&self, dry_run: bool) {
        self.with_tx(|t| {
            *t = Tx {
                mode: t.mode,
                began: Some(Instant::now()),
                dry_run,
                confirmed: !dry_run,
                ..Tx::default()
            };
        });
        self.in_transaction.store(true, Ordering::Release);
    }

    pub(crate) fn wrote(&self, rows: Option<u64>, confirmed: bool) {
        self.with_tx(|t| {
            t.writes += 1;
            t.rows += rows.unwrap_or(0);
            t.confirmed &= confirmed;
        });
    }

    pub(crate) fn tx_failed(&self) {
        self.with_tx(|t| t.failed = true);
    }

    pub(crate) fn ended(&self) {
        self.in_transaction.store(false, Ordering::Release);
        self.with_tx(|t| {
            *t = Tx {
                mode: t.mode,
                ..Tx::default()
            }
        });
    }

    pub(crate) fn set_mode(&self, mode: CommitMode) {
        self.with_tx(|t| t.mode = mode);
    }
}

pub(crate) fn aborts_on_error(dialect: nd_sql::Dialect) -> bool {
    matches!(dialect, nd_sql::Dialect::Postgres | nd_sql::Dialect::Duckdb)
}

pub(crate) fn returning(sql: &str, dialect: nd_sql::Dialect) -> Option<String> {
    use nd_sql::Dialect as D;
    if !matches!(dialect, D::Postgres | D::Sqlite | D::Duckdb) {
        return None;
    }
    let upper = sql.to_ascii_uppercase();
    let insert = upper.trim_start().starts_with("INSERT");
    let has = upper
        .split(|c: char| !c.is_ascii_alphanumeric() && c != '_')
        .any(|w| w == "RETURNING");
    (insert && !has).then(|| format!("{sql}\nRETURNING *"))
}

async fn read(
    conn: &dyn Connection,
    sql: &str,
) -> std::result::Result<(Vec<Column>, Vec<Row>), DriverError> {
    let (tx, mut rx) = mpsc::channel(8);
    let gather = async {
        let (mut columns, mut rows) = (Vec::new(), Vec::new());
        while let Some(chunk) = rx.recv().await {
            match chunk {
                Chunk::Columns(c) => columns = c,
                Chunk::Rows(r) => rows.extend(r),
                Chunk::Documents(_) | Chunk::Progress(_) => {}
            }
        }
        (columns, rows)
    };
    let (outcome, got) = tokio::join!(conn.execute(sql, tx), gather);
    outcome.map(|_| got)
}

fn same_name(column: &str, assigned: &str) -> bool {
    column.eq_ignore_ascii_case(assigned)
}

impl Core {
    pub fn transaction(&self, session_id: &str) -> Result<TxState> {
        Ok(self.session(session_id)?.tx_state())
    }

    pub fn set_commit_mode(&self, session_id: &str, mode: CommitMode) -> Result<TxState> {
        let session = self.session(session_id)?;
        session.set_mode(mode);
        Ok(session.tx_state())
    }

    pub async fn commit(&self, session_id: &str, confirm: Option<&str>) -> Result<TxState> {
        let session = self.session(session_id)?;
        let _busy = claim(&session)?;
        if !session.tx_open() {
            return Ok(session.tx_state());
        }
        let t = session.tx.lock().map_err(|_| CoreError::Poisoned)?.clone();
        if t.failed {
            return Err(CoreError::TransactionFailed);
        }
        let access = if t.confirmed {
            Access::Held
        } else {
            Access::Write { confirm }
        };
        self.admit(&session, access).await?;
        self.finish(&session, true).await?;
        Ok(session.tx_state())
    }

    pub async fn rollback(&self, session_id: &str) -> Result<TxState> {
        let session = self.session(session_id)?;
        let _busy = claim(&session)?;
        if session.tx_open() {
            self.finish(&session, false).await?;
        }
        Ok(session.tx_state())
    }

    pub(crate) async fn begin(
        &self,
        session: &Session,
        dry_run: bool,
    ) -> std::result::Result<(), DriverError> {
        match session.conn.dialect() {
            Some(dialect) => quiet(session.conn.as_ref(), nd_sql::begin(dialect)).await?,
            None => session.conn.begin_native().await?,
        }
        session.began(dry_run);
        Ok(())
    }

    pub(crate) async fn finish(&self, session: &Session, commit: bool) -> Result<()> {
        let conn = session.conn.as_ref();
        let t = session.tx.lock().map_err(|_| CoreError::Poisoned)?.clone();
        let sql = if commit { "COMMIT" } else { "ROLLBACK" };
        let out = if conn.dialect().is_some() {
            let out = quiet(conn, sql).await;
            if out.is_err() && commit {
                let _ = quiet(conn, "ROLLBACK").await;
            }
            out
        } else {
            conn.end_native(commit).await
        };
        if t.writes > 0 {
            let rows = (commit && out.is_ok()).then_some(t.rows);
            self.audit(
                session,
                sql,
                rows,
                out.as_ref().err().map(ToString::to_string),
            );
        }
        session.ended();
        self.restore_read_only(session).await;
        out.map_err(Into::into)
    }

    pub(crate) async fn restore_read_only(&self, session: &Session) {
        let conn = session.conn.as_ref();
        if !session.guarded() || conn.dialect() != Some(nd_sql::Dialect::Postgres) {
            return;
        }
        let locked = session.server_read_only.load(Ordering::Acquire);
        if let Some(sql) = conn.dialect().and_then(|d| nd_sql::read_only(d, locked))
            && let Err(e) = quiet(conn, sql).await
        {
            tracing::warn!(error = %e, "read-only setting not restored");
        }
    }

    pub(crate) async fn preview(
        &self,
        session: &Session,
        sql: &str,
    ) -> std::result::Result<Option<RowPreview>, DriverError> {
        let Some((p, mut preview)) = self.estimate(session, sql).await else {
            return Ok(None);
        };
        if preview.kind == PreviewKind::Delete {
            preview.measured = true;
            return Ok(Some(preview));
        }
        match self.rehearse(session, sql, &p, &preview).await? {
            Some(after) => {
                preview.changed = changed_columns(&preview, &after);
                preview.after = after;
                preview.measured = true;
            }
            None if p.exact_only => return Ok(None),
            None => {}
        }
        Ok(Some(preview))
    }

    async fn estimate(
        &self,
        session: &Session,
        sql: &str,
    ) -> Option<(nd_sql::Preview, RowPreview)> {
        let conn = session.conn.as_ref();
        let dialect = conn.dialect()?;
        let limit = u32::try_from(PREVIEW_ROWS + 1).unwrap_or(u32::MAX);
        let p = nd_sql::preview_query(sql, dialect, limit)?;
        let guarded = dialect == nd_sql::Dialect::Postgres;
        if guarded
            && quiet(conn, &format!("SAVEPOINT {PREVIEW_SAVEPOINT}"))
                .await
                .is_err()
        {
            return None;
        }
        let canceller = conn.canceller();
        let got = timed(read(conn, &p.sql), Some(PREVIEW_LIMIT), &canceller).await;
        if guarded {
            if got.is_err() {
                let _ = quiet(conn, &format!("ROLLBACK TO SAVEPOINT {PREVIEW_SAVEPOINT}")).await;
            }
            let _ = quiet(conn, &format!("RELEASE SAVEPOINT {PREVIEW_SAVEPOINT}")).await;
        }
        let (mut columns, mut rows) = got
            .inspect_err(|e| tracing::debug!(error = %e, "dry run preview skipped"))
            .ok()?;
        let extra = p.assigned.len();
        let width = columns.len().checked_sub(extra)?;
        columns.truncate(width);
        let changed: Vec<u32> = p
            .assigned
            .iter()
            .map(|a| {
                columns
                    .iter()
                    .position(|c| same_name(&c.name, a))
                    .and_then(|i| u32::try_from(i).ok())
            })
            .collect::<Option<_>>()?;
        let truncated = rows.len() > PREVIEW_ROWS;
        rows.truncate(PREVIEW_ROWS);
        let mut before = Vec::with_capacity(rows.len());
        let mut after = Vec::new();
        for mut row in rows {
            let new = row.split_off(width.min(row.len()));
            if extra > 0 {
                let mut a = row.clone();
                for (slot, value) in changed.iter().zip(new) {
                    if let Some(cell) = a.get_mut(*slot as usize) {
                        *cell = value;
                    }
                }
                after.push(a);
            }
            before.push(row);
        }
        let preview = RowPreview {
            kind: if extra > 0 {
                PreviewKind::Update
            } else {
                PreviewKind::Delete
            },
            columns,
            before,
            after,
            changed,
            truncated,
            measured: false,
        };
        Some((p, preview))
    }

    async fn rehearse(
        &self,
        session: &Session,
        sql: &str,
        p: &nd_sql::Preview,
        preview: &RowPreview,
    ) -> std::result::Result<Option<Vec<Row>>, DriverError> {
        let conn = session.conn.as_ref();
        let Some(dialect) = conn.dialect() else {
            return Ok(None);
        };
        let marks = match dialect {
            nd_sql::Dialect::Postgres | nd_sql::Dialect::Mysql | nd_sql::Dialect::Sqlite => (
                format!("SAVEPOINT {PREVIEW_SAVEPOINT}"),
                format!("ROLLBACK TO SAVEPOINT {PREVIEW_SAVEPOINT}"),
                Some(format!("RELEASE SAVEPOINT {PREVIEW_SAVEPOINT}")),
            ),
            nd_sql::Dialect::Mssql => (
                format!("SAVE TRANSACTION {PREVIEW_SAVEPOINT}"),
                format!("ROLLBACK TRANSACTION {PREVIEW_SAVEPOINT}"),
                None,
            ),
            _ => return Ok(None),
        };
        let remote = matches!(
            session.target,
            nd_driver::Target::Libsql(nd_driver::LibsqlTarget::Remote { .. })
        );
        let Some((namespace, name)) = p.relation.clone().filter(|_| !remote) else {
            return Ok(None);
        };
        let Some(lookup) = keyed_read(conn, dialect, namespace, name, preview).await else {
            return Ok(None);
        };
        let (open, undo, release) = marks;
        if quiet(conn, &open).await.is_err() {
            return Ok(None);
        }
        let canceller = conn.canceller();
        let ran = timed(
            async {
                collect(conn, sql, None).await?;
                collect(conn, &lookup.sql, Some(&lookup.params)).await
            },
            Some(PREVIEW_LIMIT),
            &canceller,
        )
        .await;
        quiet(conn, &undo).await.inspect_err(|e| {
            tracing::error!(error = %e, "dry run rehearsal not rolled back");
        })?;
        if let Some(release) = release {
            let _ = quiet(conn, &release).await;
        }
        let Ok((rows, _)) = ran.inspect_err(|e| tracing::debug!(error = %e, "rehearsal skipped"))
        else {
            return Ok(None);
        };
        Ok(pair(&lookup.at, &preview.before, rows))
    }
}

struct KeyedRead {
    sql: String,
    params: Vec<Option<String>>,
    at: Vec<usize>,
}

async fn keyed_read(
    conn: &dyn Connection,
    dialect: nd_sql::Dialect,
    namespace: String,
    name: String,
    preview: &RowPreview,
) -> Option<KeyedRead> {
    let namespace = if namespace.is_empty() {
        let sql = match dialect {
            nd_sql::Dialect::Postgres => "SELECT current_schema()",
            nd_sql::Dialect::Mysql => "SELECT DATABASE()",
            nd_sql::Dialect::Mssql => "SELECT SCHEMA_NAME()",
            _ => "SELECT 'main'",
        };
        collect(conn, sql, None)
            .await
            .ok()?
            .0
            .first()?
            .first()?
            .clone()?
    } else {
        namespace
    };
    if dialect == nd_sql::Dialect::Mysql {
        let engine = collect(
            conn,
            "SELECT engine FROM information_schema.tables WHERE table_schema = ? AND table_name = ?",
            Some(&[Some(namespace.clone()), Some(name.clone())]),
        )
        .await
        .ok()?;
        let innodb = engine
            .0
            .first()?
            .first()?
            .as_deref()?
            .eq_ignore_ascii_case("InnoDB");
        if !innodb {
            return None;
        }
    }
    let relation = conn.relation(&namespace, &name).await.ok()??;
    let key: Vec<&str> = relation
        .columns
        .iter()
        .filter(|c| c.primary_key)
        .map(|c| c.name.as_str())
        .collect();
    if key.is_empty() {
        return None;
    }
    let at: Vec<usize> = key
        .iter()
        .map(|k| preview.columns.iter().position(|c| same_name(&c.name, k)))
        .collect::<Option<_>>()?;
    if at.iter().any(|i| {
        preview
            .changed
            .contains(&u32::try_from(*i).unwrap_or(u32::MAX))
    }) {
        return None;
    }
    let keys: Vec<Vec<nd_sql::Field>> = preview
        .before
        .iter()
        .map(|row| {
            key.iter()
                .zip(&at)
                .map(|(k, i)| nd_sql::Field {
                    column: (*k).to_owned(),
                    value: row.get(*i).cloned().flatten(),
                })
                .collect()
        })
        .collect();
    let types: Vec<nd_sql::ColumnType> = relation
        .columns
        .iter()
        .map(|c| nd_sql::ColumnType {
            name: c.name.clone(),
            type_name: c.type_name.clone(),
        })
        .collect();
    let table = nd_sql::TableRef { namespace, name };
    let bound = nd_sql::select_keys(dialect, &table, &types, &keys);
    Some(KeyedRead {
        sql: bound.sql,
        params: bound.params,
        at,
    })
}

fn pair(at: &[usize], before: &[Row], rows: Vec<Row>) -> Option<Vec<Row>> {
    let key = |row: &Row| -> Vec<Option<String>> {
        at.iter().map(|i| row.get(*i).cloned().flatten()).collect()
    };
    let mut by_key: std::collections::HashMap<Vec<Option<String>>, Row> =
        rows.into_iter().map(|r| (key(&r), r)).collect();
    before.iter().map(|b| by_key.remove(&key(b))).collect()
}

fn changed_columns(preview: &RowPreview, after: &[Row]) -> Vec<u32> {
    let mut out: Vec<u32> = preview.changed.clone();
    for (i, _) in preview.columns.iter().enumerate() {
        let Ok(slot) = u32::try_from(i) else { continue };
        if out.contains(&slot) {
            continue;
        }
        if preview
            .before
            .iter()
            .zip(after)
            .any(|(b, a)| b.get(i) != a.get(i))
        {
            out.push(slot);
        }
    }
    out.sort_unstable();
    out
}
