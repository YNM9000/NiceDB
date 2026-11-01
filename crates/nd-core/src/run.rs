use crate::session::Sql as _;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

use nd_driver::{Canceller, Chunk, Column, DriverError, Position, Row};
use nd_sql::{Risk, Statement};
use nd_store::{HistoryEntry, now_ms};
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;

use crate::guard::{Access, claim, timed};
use crate::offsets::{byte_to_utf16, utf16_to_byte};
use crate::params::{Prepared, prepare, query_key};
use crate::session::Session;
use crate::tx::CommitMode;
use crate::{Core, CoreError, Result};

pub const MAX_ROWS: u64 = 2_000_000;

#[derive(Debug, Clone, Deserialize, specta::Type)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum RunScope {
    Script,
    Selection { from: u32, to: u32 },
    Statement { cursor: u32 },
}

#[derive(Debug, Clone, Deserialize, specta::Type)]
pub struct RunRequest {
    pub run_id: String,
    pub session_id: String,
    pub text: String,
    pub scope: RunScope,
    #[serde(default)]
    pub confirm: RunConfirm,
    #[serde(default)]
    pub sort: Vec<nd_sql::SortKey>,
    #[serde(default)]
    pub all_rows: bool,
    #[serde(default)]
    pub dry_run: bool,
    #[serde(default)]
    pub params: Option<Vec<nd_sql::ParamValue>>,
    #[serde(default)]
    pub continue_on_error: bool,
}

#[derive(Debug, Clone, Default, Deserialize, specta::Type)]
pub struct RunConfirm {
    pub risks: bool,
    pub name: Option<String>,
}

#[derive(Debug, Clone, Serialize, specta::Type)]
pub struct RiskFound {
    pub index: u32,
    pub from: u32,
    pub to: u32,
    pub sql: String,
    pub risk: nd_sql::Risk,
    #[specta(type = Option<f64>)]
    pub rows: Option<u64>,
}

#[derive(Debug, Clone, Serialize, specta::Type)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum RunEvent {
    Statement {
        index: u32,
        from: u32,
        to: u32,
        sql: String,
        unsortable: Option<nd_sql::Unsortable>,
    },
    Columns {
        index: u32,
        columns: Vec<Column>,
    },
    Rows {
        index: u32,
        rows: Vec<Row>,
    },
    Progress {
        index: u32,
        progress: nd_driver::QueryProgress,
    },
    Documents {
        index: u32,
        documents: Vec<String>,
    },
    Done {
        index: u32,
        #[specta(type = f64)]
        elapsed_ms: u64,
        #[specta(type = f64)]
        row_count: u64,
        #[specta(type = Option<f64>)]
        rows_affected: Option<u64>,
        command: Option<String>,
        truncated: bool,
        #[specta(type = Option<f64>)]
        row_limit: Option<u64>,
    },
    Failed {
        index: u32,
        #[specta(type = f64)]
        elapsed_ms: u64,
        error: DriverError,
        position: Option<Position>,
        at: Option<u32>,
    },
    Relocked,
    Reconnected {
        index: u32,
        write: bool,
        transaction_lost: bool,
    },
    Lost {
        index: u32,
        write: bool,
        error: CoreError,
    },
    Preview {
        index: u32,
        preview: crate::RowPreview,
    },
    ImplicitCommit {
        index: u32,
    },
    TransactionLost,
    Transaction {
        state: crate::TxState,
    },
    Finished {
        #[specta(type = f64)]
        elapsed_ms: u64,
        cancelled: bool,
    },
}

struct Job<'a> {
    text: &'a str,
    st: &'a Statement,
    shown: &'a str,
    base: &'a nd_sql::Filled,
    sql: &'a str,
    params: &'a [Option<String>],
    index: u32,
    all_rows: bool,
}

impl<'a> Job<'a> {
    fn of(
        plan: &Plan<'a>,
        filled: &'a Prepared,
        st: &'a Statement,
        index: u32,
        returned: Option<&'a str>,
    ) -> Self {
        let exec = filled.exec();
        let (base, sql, params) = match returned {
            Some(r) => (&filled.display, r, &[][..]),
            None => (
                exec,
                plan.wrapped.unwrap_or(&exec.sql),
                exec.params.as_slice(),
            ),
        };
        Job {
            text: plan.text,
            st,
            shown: &filled.display.sql,
            base,
            sql,
            params,
            index,
            all_rows: plan.all_rows,
        }
    }

    fn source_at(&self, error: &DriverError, shift: usize) -> Option<usize> {
        let DriverError::Query {
            offset: Some(o), ..
        } = error
        else {
            return None;
        };
        (*o as usize)
            .checked_sub(shift)
            .map(|o| self.base.source_offset(o))
            .filter(|o| *o <= self.st.range.len())
            .map(|o| self.st.range.start + o)
    }
}

struct Plan<'a> {
    text: &'a str,
    prepared: &'a [Prepared],
    dialect: nd_sql::Dialect,
    dry: bool,
    mode: CommitMode,
    access: Access<'a>,
    wrapped: Option<&'a str>,
    all_rows: bool,
    continue_on_error: bool,
}

enum Next {
    Go,
    Stop,
    Dropped,
}

pub(crate) fn ms_since(t: Instant) -> u64 {
    ms(t)
}

pub(crate) struct RunHandle {
    pub(crate) cancelled: Arc<AtomicBool>,
    pub(crate) canceller: Arc<dyn Canceller>,
}

pub(crate) fn select(text: &str, dialect: nd_sql::Dialect, scope: &RunScope) -> Vec<Statement> {
    match *scope {
        RunScope::Script => nd_sql::split(text, dialect),
        RunScope::Selection { from, to } => {
            let (a, b) = (
                utf16_to_byte(text, from.min(to)),
                utf16_to_byte(text, from.max(to)),
            );
            nd_sql::split(&text[a..b], dialect)
                .into_iter()
                .map(|s| Statement {
                    range: s.range.start + a..s.range.end + a,
                })
                .collect()
        }
        RunScope::Statement { cursor } => {
            let all = nd_sql::split(text, dialect);
            nd_sql::statement_at(&all, utf16_to_byte(text, cursor))
                .cloned()
                .into_iter()
                .collect()
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TxControl {
    Begin,
    Commit,
    Rollback,
}

fn transaction_change(sql: &str, dialect: nd_sql::Dialect) -> Option<TxControl> {
    let mut words = sql.split_whitespace().map(str::to_ascii_uppercase);
    let first = words.next()?;
    let second = words.next();
    if dialect == nd_sql::Dialect::Oracle {
        return match (first.trim_end_matches(';'), second.as_deref()) {
            ("SET", Some("TRANSACTION")) => Some(TxControl::Begin),
            ("ROLLBACK", Some("TO" | "WORK")) if sql.to_ascii_uppercase().contains(" TO ") => None,
            ("COMMIT", _) => Some(TxControl::Commit),
            ("ROLLBACK", _) => Some(TxControl::Rollback),
            _ => None,
        };
    }
    match (first.trim_end_matches(';'), second) {
        ("BEGIN", Some(w)) if matches!(w.as_str(), "TRY" | "CATCH") => None,
        ("BEGIN", _) | ("START", Some(_)) => Some(TxControl::Begin),
        ("ROLLBACK", Some(w)) if w == "TO" => None,
        ("COMMIT" | "END", _) => Some(TxControl::Commit),
        ("ROLLBACK" | "ABORT", _) => Some(TxControl::Rollback),
        _ => None,
    }
}

fn writes(sql: &str, dialect: nd_sql::Dialect) -> bool {
    !nd_sql::is_read(sql, dialect) && transaction_change(sql, dialect) != Some(TxControl::Rollback)
}

fn check_dry(dialect: nd_sql::Dialect, prepared: &[Prepared]) -> Result<()> {
    for (i, p) in prepared.iter().enumerate() {
        let sql = p.display.sql.as_str();
        if transaction_change(sql, dialect).is_some()
            || nd_sql::transaction_fit(sql, dialect) != nd_sql::TxFit::Held
        {
            return Err(CoreError::NotDryRunnable {
                index: u32::try_from(i).unwrap_or(u32::MAX),
                sql: sql.to_owned(),
            });
        }
    }
    Ok(())
}

fn risks(
    text: &str,
    dialect: nd_sql::Dialect,
    statements: &[Statement],
    prepared: &[Prepared],
) -> Vec<RiskFound> {
    statements
        .iter()
        .zip(prepared)
        .enumerate()
        .filter_map(|(i, (st, p))| {
            let sql = p.display.sql.as_str();
            nd_sql::risk(sql, dialect).map(|risk| RiskFound {
                index: u32::try_from(i).unwrap_or(u32::MAX),
                from: byte_to_utf16(text, st.range.start),
                to: byte_to_utf16(text, st.range.end),
                sql: sql.to_owned(),
                risk,
                rows: None,
            })
        })
        .collect()
}

fn statement_event(
    text: &str,
    dialect: nd_sql::Dialect,
    st: &Statement,
    sql: &str,
    index: u32,
) -> RunEvent {
    RunEvent::Statement {
        index,
        from: byte_to_utf16(text, st.range.start),
        to: byte_to_utf16(text, st.range.end),
        sql: sql.to_owned(),
        unsortable: nd_sql::sortable(sql, dialect).err(),
    }
}

fn sorted(
    dialect: nd_sql::Dialect,
    prepared: &[Prepared],
    keys: &[nd_sql::SortKey],
) -> Result<Option<String>> {
    if keys.is_empty() {
        return Ok(None);
    }
    let [p] = prepared else {
        return Err(CoreError::Unsortable(nd_sql::Unsortable::Multiple));
    };
    nd_sql::sorted(&p.exec().sql, dialect, keys)
        .map(Some)
        .map_err(CoreError::Unsortable)
}

async fn count_affected(session: &Session, risks: &mut [RiskFound]) {
    let Some(dialect) = session.conn.dialect() else {
        return;
    };
    let counted: Vec<(usize, String)> = risks
        .iter()
        .enumerate()
        .filter(|(_, r)| matches!(r.risk, Risk::UpdateWithoutWhere | Risk::DeleteWithoutWhere))
        .filter_map(|(i, r)| nd_sql::count_query(&r.sql, dialect).map(|q| (i, q)))
        .collect();
    let queries: Vec<_> = counted
        .iter()
        .map(|(_, q)| (q.clone(), Vec::new()))
        .collect();
    let counts = crate::data::count_rows(session, &queries).await;
    for ((i, _), n) in counted.iter().zip(counts) {
        if let Some(r) = risks.get_mut(*i) {
            r.rows = n;
        }
    }
}

async fn execute(
    session: &Session,
    sql: &str,
    params: &[Option<String>],
    out: mpsc::Sender<Chunk>,
) -> std::result::Result<nd_driver::Outcome, DriverError> {
    if params.is_empty() {
        session.conn.execute(sql, out).await
    } else {
        session.conn.execute_bound(sql, params, out).await
    }
}

fn ms(t: Instant) -> u64 {
    u64::try_from(t.elapsed().as_millis()).unwrap_or(u64::MAX)
}

impl Core {
    pub async fn run(&self, req: RunRequest, emit: impl Fn(RunEvent) + Send + Sync) -> Result<()> {
        let session = self.live(&req.session_id).await?;
        if session.conn.dialect().is_none() {
            return self.run_native(&session, &req, &emit).await;
        }
        let _busy = claim(&session)?;
        if session.tx_lost.swap(false, Ordering::AcqRel) {
            emit(RunEvent::TransactionLost);
        }
        let dialect = session.conn.sql()?;
        let text = req.text.as_str();
        let statements = select(text, dialect, &req.scope);
        let (prepared, used) =
            prepare(text, dialect, &statements, req.params.as_deref()).map_err(CoreError::Param)?;
        let wrapped = sorted(dialect, &prepared, &req.sort)?;
        let any_write = prepared.iter().any(|p| writes(&p.display.sql, dialect));
        let dry = req.dry_run && any_write;
        self.screen(&session, &req, &statements, &prepared, dry)
            .await?;
        self.remember_params(&query_key(text, &statements), &used);
        let confirm = req.confirm.name.as_deref();
        let write_access = if dry {
            Access::Held
        } else {
            Access::Write { confirm }
        };
        self.admit(
            &session,
            if any_write {
                write_access
            } else {
                Access::Read
            },
        )
        .await?;
        let was_open = session.access()?.open_ms.is_some();
        let stop = Arc::new(AtomicBool::new(false));
        let canceller = session.conn.canceller();
        Self::lock(&self.runs)?.insert(
            req.run_id.clone(),
            RunHandle {
                cancelled: stop.clone(),
                canceller: canceller.clone(),
            },
        );

        let plan = Plan {
            text,
            prepared: &prepared,
            dialect,
            dry,
            mode: session.commit_mode(),
            access: write_access,
            wrapped: wrapped.as_deref(),
            all_rows: req.all_rows,
            continue_on_error: req.continue_on_error && !dry,
        };
        let started = Instant::now();
        let dropped = self
            .steps(
                &req.session_id,
                &session,
                &plan,
                &statements,
                &stop,
                &canceller,
                &emit,
            )
            .await;
        if dry && !dropped && stop.load(Ordering::Acquire) && session.tx_open() {
            self.undo_dry(&session).await;
        }

        Self::lock(&self.runs)?.remove(&req.run_id);
        if let Err(e) = self.relock_if_expired(&session).await {
            tracing::warn!(error = %e, "writes not locked again");
        }
        if was_open && session.access()?.open_ms.is_none() {
            emit(RunEvent::Relocked);
        }
        if let Ok(now) = self.session(&req.session_id) {
            emit(RunEvent::Transaction {
                state: now.tx_state(),
            });
        }
        emit(RunEvent::Finished {
            elapsed_ms: ms(started),
            cancelled: stop.load(Ordering::Acquire),
        });
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    async fn steps(
        &self,
        id: &str,
        session: &Session,
        plan: &Plan<'_>,
        statements: &[Statement],
        stop: &AtomicBool,
        canceller: &Arc<dyn Canceller>,
        emit: &(impl Fn(RunEvent) + Send + Sync),
    ) -> bool {
        for (i, st) in statements.iter().enumerate() {
            if stop.load(Ordering::Acquire) {
                break;
            }
            let index = u32::try_from(i).unwrap_or(u32::MAX);
            match self
                .step(id, session, plan, st, index, canceller, emit)
                .await
            {
                Next::Go => {}
                Next::Stop => break,
                Next::Dropped => return true,
            }
        }
        false
    }

    async fn screen(
        &self,
        session: &Session,
        req: &RunRequest,
        statements: &[Statement],
        prepared: &[Prepared],
        dry: bool,
    ) -> Result<()> {
        let (text, dialect) = (req.text.as_str(), session.conn.sql()?);
        if dry {
            if session.tx_open() {
                return Err(CoreError::TransactionOpen);
            }
            return check_dry(dialect, prepared);
        }
        let mut risks = risks(text, dialect, statements, prepared);
        if !risks.is_empty() && !req.confirm.risks && !self.risk_prompt_off(session) {
            self.admit(session, Access::Read).await?;
            count_affected(session, &mut risks).await;
            return Err(CoreError::Risky(risks));
        }
        Ok(())
    }

    pub(crate) async fn undo_dry(&self, session: &Session) {
        if let Err(e) = self.finish(session, false).await {
            tracing::warn!(error = %e, "dry run not rolled back");
        }
    }

    #[allow(clippy::too_many_arguments)]
    async fn step(
        &self,
        id: &str,
        session: &Session,
        plan: &Plan<'_>,
        st: &Statement,
        index: u32,
        canceller: &Arc<dyn Canceller>,
        emit: &(impl Fn(RunEvent) + Send + Sync),
    ) -> Next {
        let Plan {
            text,
            dialect,
            dry,
            mode,
            ..
        } = *plan;
        let filled = &plan.prepared[index as usize];
        let sql = filled.display.sql.as_str();
        let control = transaction_change(sql, dialect);
        let write = writes(sql, dialect);
        let fit = nd_sql::transaction_fit(sql, dialect);
        let refused = |error: DriverError| {
            emit(statement_event(text, dialect, st, sql, index));
            emit(RunEvent::Failed {
                index,
                elapsed_ms: 0,
                error,
                position: None,
                at: None,
            });
            Next::Stop
        };
        if write && let Err(e) = self.admit(session, plan.access).await {
            return refused(DriverError::query(e.to_string()));
        }
        let opens = dry || mode == CommitMode::Manual || (mode == CommitMode::Smart && write);
        if opens
            && control.is_none()
            && fit == nd_sql::TxFit::Held
            && !session.tx_open()
            && let Err(error) = self.begin(session, dry).await
        {
            return refused(error);
        }
        let preview = if dry && write {
            match self.preview(session, sql).await {
                Ok(p) => p,
                Err(error) => {
                    self.undo_dry(session).await;
                    return refused(error);
                }
            }
        } else {
            None
        };
        let returned = (dry && write)
            .then(|| crate::tx::returning(sql, dialect))
            .flatten();
        let held = session.tx_open();
        let job = Job::of(plan, filled, st, index, returned.as_deref());
        match self.run_one(session, job, canceller, emit).await {
            Err(error) => {
                if session.gone(&error).await {
                    emit(self.recover(id, session, index, write).await);
                    return Next::Dropped;
                }
                if dry && session.tx_open() {
                    self.undo_dry(session).await;
                } else if session.tx_open() && crate::tx::aborts_on_error(dialect) {
                    session.tx_failed();
                    return Next::Stop;
                }
                if plan.continue_on_error {
                    Next::Go
                } else {
                    Next::Stop
                }
            }
            Ok(rows) => {
                if let Some(preview) = preview {
                    emit(RunEvent::Preview { index, preview });
                }
                match control {
                    Some(TxControl::Begin) => session.began(false),
                    Some(TxControl::Commit | TxControl::Rollback) => {
                        session.ended();
                        self.restore_read_only(session).await;
                    }
                    None if held && fit == nd_sql::TxFit::Commits => {
                        session.ended();
                        emit(RunEvent::ImplicitCommit { index });
                    }
                    None if held && write => session.wrote(rows, !dry),
                    None => {}
                }
                Next::Go
            }
        }
    }

    pub(crate) async fn recover(
        &self,
        id: &str,
        session: &Session,
        index: u32,
        write: bool,
    ) -> RunEvent {
        let transaction_lost = session.tx_open();
        session.mark_lost();
        if !session.options().reconnect {
            return RunEvent::Lost {
                index,
                write,
                error: CoreError::Driver(DriverError::Io {
                    message: "connection lost".into(),
                }),
            };
        }
        match self.reconnect(id).await {
            Ok(_) => RunEvent::Reconnected {
                index,
                write,
                transaction_lost,
            },
            Err(error) => RunEvent::Lost {
                index,
                write,
                error,
            },
        }
    }

    fn row_cap(&self, session: &Session, sql: &str, all_rows: bool) -> (u64, bool) {
        let global = u64::from(self.preferences().results.row_cap);
        if all_rows {
            return (global, false);
        }
        match session.options().row_limit {
            Some(limit)
                if u64::from(limit) < global
                    && !session
                        .conn
                        .dialect()
                        .is_some_and(|d| nd_sql::has_limit(sql, d)) =>
            {
                (u64::from(limit.max(1)), true)
            }
            _ => (global, false),
        }
    }

    async fn run_one(
        &self,
        session: &Session,
        job: Job<'_>,
        canceller: &Arc<dyn Canceller>,
        emit: &(impl Fn(RunEvent) + Send + Sync),
    ) -> std::result::Result<Option<u64>, DriverError> {
        let Job {
            text,
            st,
            shown,
            base,
            sql,
            params,
            index,
            all_rows,
        } = job;
        let Some(dialect) = session.conn.dialect() else {
            return Err(DriverError::query("this engine does not speak SQL"));
        };
        emit(statement_event(text, dialect, st, shown, index));
        let shift = sql.find(base.sql.as_str()).unwrap_or(0);
        let logged = if base.changed() { shown } else { sql };

        let t = Instant::now();
        let (row_cap, own_limit) = self.row_cap(session, shown, all_rows);
        let (tx, mut rx) = mpsc::channel::<Chunk>(8);
        let forward = async {
            let mut count = 0u64;
            let mut truncated = false;
            while let Some(chunk) = rx.recv().await {
                match chunk {
                    Chunk::Columns(columns) => emit(RunEvent::Columns { index, columns }),
                    Chunk::Rows(mut rows) if !truncated => {
                        let room =
                            usize::try_from(row_cap.saturating_sub(count)).unwrap_or(usize::MAX);
                        if rows.len() >= room {
                            rows.truncate(room);
                            truncated = true;
                            canceller.cancel().await;
                        }
                        count += rows.len() as u64;
                        emit(RunEvent::Rows { index, rows });
                    }
                    Chunk::Progress(progress) => emit(RunEvent::Progress { index, progress }),
                    Chunk::Rows(_) | Chunk::Documents(_) => {}
                }
            }
            (count, truncated)
        };
        let exec = timed(
            execute(session, sql, params, tx),
            self.timeout_for(session.environment()),
            canceller,
        );
        let (outcome, (row_count, truncated)) = tokio::join!(exec, forward);
        let elapsed_ms = ms(t);
        self.audit_write(session, logged, &outcome);

        let finished = match outcome {
            Ok(o) => Ok((o.rows_affected, o.command)),
            Err(DriverError::Cancelled) if truncated => Ok((None, None)),
            Err(e) => Err(e),
        };
        match finished {
            Ok((rows_affected, command)) => {
                emit(RunEvent::Done {
                    index,
                    elapsed_ms,
                    row_count,
                    rows_affected,
                    command,
                    truncated,
                    row_limit: (truncated && own_limit).then_some(row_cap),
                });
                self.record(
                    &session.connection_id,
                    logged,
                    elapsed_ms,
                    Some(rows_affected.unwrap_or(row_count)),
                    None,
                );
                Ok(rows_affected)
            }
            Err(error) => {
                let abs = job.source_at(&error, shift);
                let message = Some(error.to_string());
                self.record(&session.connection_id, logged, elapsed_ms, None, message);
                emit(RunEvent::Failed {
                    index,
                    elapsed_ms,
                    position: abs.map(|b| nd_sql::position_at(text, b)),
                    at: abs.map(|b| byte_to_utf16(text, b)),
                    error: error.clone(),
                });
                Err(error)
            }
        }
    }

    fn audit_write(
        &self,
        session: &Session,
        sql: &str,
        outcome: &std::result::Result<nd_driver::Outcome, DriverError>,
    ) {
        if session
            .conn
            .dialect()
            .is_some_and(|d| nd_sql::is_read(sql, d))
        {
            return;
        }
        match outcome {
            Ok(o) => self.audit(session, sql, o.rows_affected, None),
            Err(e) => self.audit(session, sql, None, Some(e.to_string())),
        }
    }

    pub async fn cancel(&self, run_id: &str) -> Result<()> {
        let canceller = {
            let runs = Self::lock(&self.runs)?;
            let Some(run) = runs.get(run_id) else {
                return Ok(());
            };
            run.cancelled.store(true, Ordering::Release);
            run.canceller.clone()
        };
        canceller.cancel().await;
        Ok(())
    }

    pub(crate) fn record(
        &self,
        connection_id: &str,
        sql: &str,
        elapsed_ms: u64,
        rows: Option<u64>,
        error: Option<String>,
    ) {
        let entry = HistoryEntry {
            id: 0,
            connection_id: connection_id.to_owned(),
            sql: nd_sql::redact_secrets(sql, self.dialect_of(connection_id)).into_owned(),
            started_at: now_ms() - i64::try_from(elapsed_ms).unwrap_or(0),
            elapsed_ms: i64::try_from(elapsed_ms).unwrap_or(i64::MAX),
            rows: rows.and_then(|r| i64::try_from(r).ok()),
            error,
        };
        if let Err(e) = self.store(|s| s.add_history(&entry)) {
            tracing::warn!(error = %e, "history not recorded");
        }
    }
}
