use std::ops::Range;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

use nd_driver::{Chunk, CommandClass, DocumentWrite, DriverError, Outcome};
use tokio::sync::mpsc;

use crate::guard::{Access, claim, timed};
use crate::offsets::{byte_to_utf16, utf16_to_byte};
use crate::run::{RiskFound, RunEvent, RunHandle, RunRequest, RunScope, ms_since};
use crate::session::Session;
use crate::tx::CommitMode;
use crate::{Core, CoreError, Result};

struct Cmd<'a> {
    text: &'a str,
    range: Range<usize>,
    index: u32,
    write: bool,
}

pub(crate) fn select(session: &Session, text: &str, scope: &RunScope) -> Vec<Range<usize>> {
    match *scope {
        RunScope::Script => session.conn.commands(text),
        RunScope::Selection { from, to } => {
            let (a, b) = (
                utf16_to_byte(text, from.min(to)),
                utf16_to_byte(text, from.max(to)),
            );
            session
                .conn
                .commands(&text[a..b])
                .into_iter()
                .map(|r| r.start + a..r.end + a)
                .collect()
        }
        RunScope::Statement { cursor } => {
            let at = utf16_to_byte(text, cursor);
            let all = session.conn.commands(text);
            let hit = all
                .iter()
                .find(|r| r.start <= at && at <= r.end)
                .or_else(|| all.iter().rev().find(|r| r.end <= at))
                .or_else(|| all.first())
                .cloned();
            hit.into_iter().collect()
        }
    }
}

impl Core {
    pub(crate) async fn run_native(
        &self,
        session: &Arc<Session>,
        req: &RunRequest,
        emit: &(impl Fn(RunEvent) + Send + Sync),
    ) -> Result<()> {
        if req.dry_run && !session.caps.transactions {
            return Err(CoreError::Driver(DriverError::query(
                "a dry run needs transactions, and this server has none",
            )));
        }
        let _busy = claim(session)?;
        let text = req.text.as_str();
        let commands = select(session, text, &req.scope);
        let classes: Vec<CommandClass> = commands
            .iter()
            .map(|r| session.conn.classify(&text[r.clone()]))
            .collect();
        let risks = risks_of(text, &commands, &classes);
        if !risks.is_empty() && !req.confirm.risks && !self.risk_prompt_off(session) {
            self.admit(session, Access::Read).await?;
            return Err(CoreError::Risky(risks));
        }
        let any_write = classes.iter().any(|c| c.write);
        let access = Access::Write {
            confirm: req.confirm.name.as_deref(),
        };
        self.admit(session, if any_write { access } else { Access::Read })
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
        let started = Instant::now();
        for (i, (range, class)) in commands.iter().zip(&classes).enumerate() {
            if stop.load(Ordering::Acquire) {
                break;
            }
            let index = u32::try_from(i).unwrap_or(u32::MAX);
            let command = &text[range.clone()];
            emit(RunEvent::Statement {
                index,
                from: byte_to_utf16(text, range.start),
                to: byte_to_utf16(text, range.end),
                sql: command.to_owned(),
                unsortable: None,
            });
            if let Err(error) = self.open_for(session, class.write, req.dry_run).await {
                emit(failure(text, range, index, 0, error));
                break;
            }
            let held = session.tx_open();
            match self
                .run_command(
                    session,
                    Cmd {
                        text,
                        range: range.clone(),
                        index,
                        write: class.write,
                    },
                    req.all_rows,
                    emit,
                )
                .await
            {
                Ok(rows) => {
                    if held && class.write {
                        session.wrote(rows, !req.dry_run);
                    }
                }
                Err(error) => {
                    self.failed_in_tx(session, held, req.dry_run).await;
                    if session.gone(&error).await {
                        emit(
                            self.recover(&req.session_id, session, index, class.write)
                                .await,
                        );
                        break;
                    }
                    if !req.continue_on_error {
                        break;
                    }
                }
            }
        }
        Self::lock(&self.runs)?.remove(&req.run_id);
        if let Err(e) = self.relock_if_expired(session).await {
            tracing::warn!(error = %e, "writes not locked again");
        }
        if was_open && session.access()?.open_ms.is_none() {
            emit(RunEvent::Relocked);
        }
        emit(RunEvent::Finished {
            elapsed_ms: ms_since(started),
            cancelled: stop.load(Ordering::Acquire),
        });
        Ok(())
    }

    async fn failed_in_tx(&self, session: &Session, held: bool, dry: bool) {
        if held && session.tx_open() {
            if dry {
                self.undo_dry(session).await;
            } else {
                session.tx_failed();
            }
        }
    }

    async fn open_for(
        &self,
        session: &Session,
        write: bool,
        dry: bool,
    ) -> std::result::Result<(), DriverError> {
        let mode = session.commit_mode();
        let opens = dry
            || (session.caps.transactions
                && (mode == CommitMode::Manual || (mode == CommitMode::Smart && write)));
        if opens && !session.tx_open() {
            self.begin(session, dry).await?;
        }
        Ok(())
    }

    async fn run_command(
        &self,
        session: &Session,
        cmd: Cmd<'_>,
        all_rows: bool,
        emit: &(impl Fn(RunEvent) + Send + Sync),
    ) -> std::result::Result<Option<u64>, DriverError> {
        let Cmd {
            text,
            range,
            index,
            write,
        } = cmd;
        let command = &text[range.clone()];
        let kept = hide_passwords(command);
        let canceller = session.conn.canceller();
        let (cap, own_limit) = self.native_cap(session, all_rows);
        let t = Instant::now();
        let (tx, mut rx) = mpsc::channel::<Chunk>(8);
        let forward = async {
            let mut count = 0u64;
            let mut truncated = false;
            while let Some(chunk) = rx.recv().await {
                if truncated {
                    continue;
                }
                let room = usize::try_from(cap.saturating_sub(count)).unwrap_or(usize::MAX);
                match chunk {
                    Chunk::Columns(columns) => emit(RunEvent::Columns { index, columns }),
                    Chunk::Rows(mut rows) => {
                        if rows.len() >= room {
                            rows.truncate(room);
                            truncated = true;
                            canceller.cancel().await;
                        }
                        count += rows.len() as u64;
                        emit(RunEvent::Rows { index, rows });
                    }
                    Chunk::Documents(mut documents) => {
                        if documents.len() >= room {
                            documents.truncate(room);
                            truncated = true;
                            canceller.cancel().await;
                        }
                        count += documents.len() as u64;
                        emit(RunEvent::Documents { index, documents });
                    }
                    Chunk::Progress(progress) => emit(RunEvent::Progress { index, progress }),
                }
            }
            (count, truncated)
        };
        let exec = timed(
            session.conn.execute(command, tx),
            self.timeout_for(session.environment()),
            &canceller,
        );
        let (outcome, (row_count, truncated)) = tokio::join!(exec, forward);
        let elapsed_ms = ms_since(t);
        if write {
            match &outcome {
                Ok(o) => self.audit(session, &kept, o.rows_affected, None),
                Err(e) => self.audit(session, &kept, None, Some(e.to_string())),
            }
        }
        let outcome = match outcome {
            Err(DriverError::Cancelled) if truncated => Ok(Outcome::default()),
            other => other,
        };
        match outcome {
            Ok(o) => {
                emit(RunEvent::Done {
                    index,
                    elapsed_ms,
                    row_count,
                    rows_affected: o.rows_affected,
                    command: o.command,
                    truncated,
                    row_limit: (truncated && own_limit).then_some(cap),
                });
                self.record(
                    &session.connection_id,
                    &kept,
                    elapsed_ms,
                    Some(o.rows_affected.unwrap_or(row_count)),
                    None,
                );
                Ok(o.rows_affected)
            }
            Err(error) => {
                self.record(
                    &session.connection_id,
                    &kept,
                    elapsed_ms,
                    None,
                    Some(error.to_string()),
                );
                emit(failure(text, &range, index, elapsed_ms, error.clone()));
                Err(error)
            }
        }
    }

    fn native_cap(&self, session: &Session, all_rows: bool) -> (u64, bool) {
        let global = u64::from(self.preferences().results.row_cap);
        match session.options().row_limit {
            Some(limit) if !all_rows && u64::from(limit) < global => {
                (u64::from(limit.max(1)), true)
            }
            _ => (global, false),
        }
    }

    pub async fn write_documents(
        &self,
        session_id: &str,
        write: &DocumentWrite,
        confirm: Option<&str>,
    ) -> Result<Outcome> {
        let session = self.live(session_id).await?;
        if !session.caps.editable {
            return Err(CoreError::Driver(DriverError::query(
                "documents of this engine cannot be edited",
            )));
        }
        let _busy = claim(&session)?;
        self.admit(&session, Access::Write { confirm }).await?;
        let shown = session.conn.describe_documents(write);
        let out = session.conn.write_documents(write).await;
        match &out {
            Ok(o) => self.audit(&session, &shown, o.rows_affected, None),
            Err(e) => self.audit(&session, &shown, None, Some(e.to_string())),
        }
        Ok(out?)
    }
}

fn hide_passwords(command: &str) -> String {
    let mut out = String::with_capacity(command.len());
    let mut rest = command;
    while let Some(at) = rest.find("\"pwd\"") {
        let (head, tail) = rest.split_at(at + 5);
        out.push_str(head);
        let after = tail.trim_start();
        let Some(value) = after.strip_prefix(':').map(str::trim_start) else {
            rest = tail;
            continue;
        };
        let Some(body) = value.strip_prefix('"') else {
            rest = tail;
            continue;
        };
        let mut end = None;
        let mut escaped = false;
        for (i, c) in body.char_indices() {
            match c {
                '\\' if !escaped => escaped = true,
                '"' if !escaped => {
                    end = Some(i);
                    break;
                }
                _ => escaped = false,
            }
        }
        let Some(end) = end else {
            rest = tail;
            continue;
        };
        out.push_str(": \"••••\"");
        rest = &body[end + 1..];
    }
    out.push_str(rest);
    out
}

fn risks_of(text: &str, commands: &[Range<usize>], classes: &[CommandClass]) -> Vec<RiskFound> {
    commands
        .iter()
        .zip(classes)
        .enumerate()
        .filter_map(|(i, (r, c))| {
            c.risk.map(|risk| RiskFound {
                index: u32::try_from(i).unwrap_or(u32::MAX),
                from: byte_to_utf16(text, r.start),
                to: byte_to_utf16(text, r.end),
                sql: text[r.clone()].to_owned(),
                risk,
                rows: None,
            })
        })
        .collect()
}

fn failure(
    text: &str,
    range: &Range<usize>,
    index: u32,
    elapsed_ms: u64,
    error: DriverError,
) -> RunEvent {
    let at = match &error {
        DriverError::Query {
            offset: Some(o), ..
        } => Some((range.start + *o as usize).min(range.end)),
        _ => None,
    };
    RunEvent::Failed {
        index,
        elapsed_ms,
        error,
        position: at.map(|b| nd_sql::position_at(text, b)),
        at: at.map(|b| byte_to_utf16(text, b)),
    }
}
