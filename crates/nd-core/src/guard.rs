use std::future::Future;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use nd_driver::{Canceller, Chunk, Connection, DriverError, Outcome, Row};
use nd_store::{
    AuditEntry, ConfirmMode, Environment, Preferences, SafetyPrefs, UnlockMode, now_ms,
};
use serde::Serialize;
use tokio::sync::mpsc;

use crate::session::Session;
use crate::{Core, CoreError, Result};

pub const UNLOCK_FOR: Duration = Duration::from_mins(10);
pub(crate) const UNTIMED_FOR: Duration = Duration::from_hours(24 * 365 * 10);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, specta::Type)]
pub struct WriteAccess {
    pub guarded: bool,
    pub open: bool,
    pub mode: UnlockMode,
    #[specta(type = Option<f64>)]
    pub open_ms: Option<u64>,
}

pub fn statement_timeout(env: Environment) -> Option<Duration> {
    SafetyPrefs::default().timeout(env)
}

pub(crate) struct Busy<'a>(&'a Session);

impl Drop for Busy<'_> {
    fn drop(&mut self) {
        self.0.touch();
        self.0.busy.store(false, Ordering::Release);
    }
}

pub(crate) fn claim(session: &Session) -> Result<Busy<'_>> {
    if session.busy.swap(true, Ordering::AcqRel) {
        return Err(CoreError::Busy);
    }
    session.touch();
    Ok(Busy(session))
}

pub(crate) async fn collect(
    conn: &dyn Connection,
    sql: &str,
    params: Option<&[Option<String>]>,
) -> std::result::Result<(Vec<Row>, Outcome), DriverError> {
    let (tx, mut rx) = mpsc::channel(8);
    let gather = async {
        let mut rows = Vec::new();
        while let Some(chunk) = rx.recv().await {
            if let Chunk::Rows(r) = chunk {
                rows.extend(r);
            }
        }
        rows
    };
    let exec = async {
        match params {
            Some(p) => conn.execute_bound(sql, p, tx).await,
            None => conn.execute(sql, tx).await,
        }
    };
    let (outcome, rows) = tokio::join!(exec, gather);
    Ok((rows, outcome?))
}

pub(crate) async fn quiet(
    conn: &dyn Connection,
    sql: &str,
) -> std::result::Result<(), DriverError> {
    collect(conn, sql, None).await.map(|_| ())
}

pub(crate) async fn timed<T>(
    fut: impl Future<Output = std::result::Result<T, DriverError>>,
    limit: Option<Duration>,
    canceller: &Arc<dyn Canceller>,
) -> std::result::Result<T, DriverError> {
    let Some(limit) = limit else {
        return fut.await;
    };
    let fired = Arc::new(AtomicBool::new(false));
    let done = Arc::new(AtomicBool::new(false));
    let timer = {
        let (fired, done, canceller) = (fired.clone(), done.clone(), canceller.clone());
        tokio::spawn(async move {
            tokio::time::sleep(limit).await;
            if !done.swap(true, Ordering::AcqRel) {
                fired.store(true, Ordering::Release);
                canceller.cancel().await;
            }
        })
    };
    let out = fut.await;
    done.store(true, Ordering::Release);
    timer.abort();
    match out {
        Err(DriverError::Cancelled) if fired.load(Ordering::Acquire) => Err(DriverError::Timeout {
            seconds: u32::try_from(limit.as_secs()).unwrap_or(u32::MAX),
        }),
        other => other,
    }
}

fn who() -> String {
    std::env::var("USER")
        .or_else(|_| std::env::var("USERNAME"))
        .unwrap_or_default()
}

impl Session {
    pub(crate) fn environment(&self) -> Environment {
        self.profile
            .lock()
            .map_or(Environment::Production, |p| p.environment)
    }

    fn name_matches(&self, typed: &str) -> Result<bool> {
        let p = self.profile.lock().map_err(|_| CoreError::Poisoned)?;
        Ok(typed.trim() == p.name.trim())
    }

    pub(crate) fn guarded(&self) -> bool {
        self.environment() == Environment::Production
    }

    pub(crate) fn access(&self) -> Result<WriteAccess> {
        let until = *self
            .unlocked_until
            .lock()
            .map_err(|_| CoreError::Poisoned)?;
        let left = until.and_then(|u| u.checked_duration_since(Instant::now()));
        let untimed = self.untimed.load(Ordering::Acquire);
        Ok(WriteAccess {
            guarded: self.guarded(),
            open: left.is_some(),
            mode: UnlockMode::Timed,
            open_ms: left
                .filter(|_| !untimed)
                .map(|d| u64::try_from(d.as_millis()).unwrap_or(u64::MAX)),
        })
    }

    async fn set_locked(&self, locked: bool) -> Result<()> {
        self.set_locked_for(locked, Some(UNLOCK_FOR)).await
    }

    async fn set_locked_for(&self, locked: bool, open_for: Option<Duration>) -> Result<()> {
        let conn = self.conn.as_ref();
        if let Some(sql) = conn.dialect().and_then(|d| nd_sql::read_only(d, locked)) {
            if quiet(conn, sql).await.is_err() {
                let _ = quiet(conn, "ROLLBACK").await;
                self.ended();
                quiet(conn, sql).await?;
            }
        } else {
            conn.set_read_only(locked).await?;
        }
        self.server_read_only.store(locked, Ordering::Release);
        self.untimed
            .store(!locked && open_for.is_none(), Ordering::Release);
        *self
            .unlocked_until
            .lock()
            .map_err(|_| CoreError::Poisoned)? =
            (!locked).then(|| Instant::now() + open_for.unwrap_or(UNTIMED_FOR));
        Ok(())
    }
}

#[derive(Debug, Clone, Copy)]
pub(crate) enum Access<'a> {
    Read,
    Write { confirm: Option<&'a str> },
    Held,
}

impl Core {
    pub(crate) async fn admit(&self, session: &Session, access: Access<'_>) -> Result<()> {
        if !session.guarded() {
            if session.server_read_only.load(Ordering::Acquire) {
                session.set_locked(false).await?;
            }
            return Ok(());
        }
        let mode = self.preferences().safety.unlock_mode;
        let held = session.access()?.open;
        if mode == UnlockMode::Open && !held {
            session.set_locked_for(false, None).await?;
        } else if mode == UnlockMode::Timed && held && session.untimed.load(Ordering::Acquire) {
            session.set_locked(true).await?;
        }
        let expired = session
            .unlocked_until
            .lock()
            .map_err(|_| CoreError::Poisoned)?
            .is_some_and(|u| u <= Instant::now());
        let open = session.access()?.open;
        if expired || (!open && !session.server_read_only.load(Ordering::Acquire)) {
            session.set_locked(true).await?;
        }
        self.authorize(session, access)
    }

    fn access_of(&self, session: &Session) -> Result<WriteAccess> {
        let mode = self.preferences().safety.unlock_mode;
        let mut a = session.access()?;
        a.mode = mode;
        if a.guarded {
            match mode {
                UnlockMode::Open => a.open = true,
                UnlockMode::Timed if session.untimed.load(Ordering::Acquire) => {
                    a.open = false;
                }
                _ => {}
            }
        }
        Ok(a)
    }

    pub(crate) fn authorize(&self, session: &Session, access: Access<'_>) -> Result<()> {
        if !session.guarded() {
            return Ok(());
        }
        let confirm = match access {
            Access::Read => return Ok(()),
            Access::Held if self.access_of(session)?.open => return Ok(()),
            Access::Held => return Err(CoreError::ReadOnly),
            Access::Write { confirm } => confirm,
        };
        if !self.access_of(session)?.open {
            return Err(CoreError::ReadOnly);
        }
        let mode = self.preferences().safety.confirm_mode;
        match (mode, confirm) {
            (ConfirmMode::None, _) => Ok(()),
            (_, None) => Err(CoreError::NeedsConfirmation),
            (ConfirmMode::Typed, Some(typed)) if !session.name_matches(typed)? => {
                Err(CoreError::NameMismatch)
            }
            (_, Some(_)) => Ok(()),
        }
    }

    pub(crate) fn confirmed(&self, name: &str, confirm: Option<&str>) -> Result<()> {
        match (self.preferences().safety.confirm_mode, confirm) {
            (ConfirmMode::None, _) => Ok(()),
            (_, None) => Err(CoreError::NeedsConfirmation),
            (ConfirmMode::Typed, Some(typed)) if typed.trim() != name.trim() => {
                Err(CoreError::NameMismatch)
            }
            (_, Some(_)) => Ok(()),
        }
    }

    pub(crate) fn risk_prompt_off(&self, session: &Session) -> bool {
        session.guarded() && self.preferences().safety.confirm_mode == ConfirmMode::None
    }

    pub fn write_access(&self, session_id: &str) -> Result<WriteAccess> {
        self.access_of(&*self.session(session_id)?)
    }

    pub async fn unlock_writes(&self, session_id: &str) -> Result<WriteAccess> {
        let safety = self.preferences().safety;
        let open_for = (safety.unlock_mode == UnlockMode::Timed).then(|| safety.unlock_for());
        self.unlock_writes_in(session_id, open_for).await
    }

    pub fn preferences(&self) -> Preferences {
        self.store(nd_store::Store::preferences)
            .unwrap_or_else(|e| {
                tracing::warn!(error = %e, "preferences not read, using defaults");
                Preferences::default()
            })
    }

    pub fn set_preferences(
        &self,
        value: &Preferences,
        confirm: Option<&str>,
    ) -> Result<Preferences> {
        let before = self.store(nd_store::Store::preferences)?.safety;
        let after = value.safety;
        let looser = rank_unlock(after.unlock_mode) > rank_unlock(before.unlock_mode)
            || rank_confirm(after.confirm_mode) > rank_confirm(before.confirm_mode);
        if looser && confirm.is_none() {
            return Err(CoreError::NeedsConfirmation);
        }
        let saved = self.store(|s| s.set_preferences(value))?;
        if looser {
            let line = format!(
                "-- production safety: unlock {:?} -> {:?}, confirm {:?} -> {:?}",
                before.unlock_mode, after.unlock_mode, before.confirm_mode, after.confirm_mode
            )
            .to_lowercase();
            let ids = self
                .store(nd_store::Store::connections)?
                .into_iter()
                .filter(|c| c.environment == Environment::Production)
                .map(|c| c.id);
            for id in ids {
                self.audit_as(&id, String::new(), &line, None, None);
            }
        }
        Ok(saved)
    }

    pub(crate) fn timeout_for(&self, env: Environment) -> Option<Duration> {
        self.preferences().safety.timeout(env)
    }

    #[doc(hidden)]
    pub async fn unlock_writes_for(
        &self,
        session_id: &str,
        open_for: Duration,
    ) -> Result<WriteAccess> {
        self.unlock_writes_in(session_id, Some(open_for)).await
    }

    async fn unlock_writes_in(
        &self,
        session_id: &str,
        open_for: Option<Duration>,
    ) -> Result<WriteAccess> {
        let session = self.live(session_id).await?;
        if session.guarded() {
            let _busy = claim(&session)?;
            if session.tx_open() && session.tx_state().writes == 0 {
                self.finish(&session, false).await?;
            }
            session.set_locked_for(false, open_for).await?;
            self.audit(&session, "-- writes unlocked", None, None);
        }
        self.access_of(&session)
    }

    pub(crate) async fn relock_if_expired(&self, session: &Session) -> Result<bool> {
        let expired = session
            .unlocked_until
            .lock()
            .map_err(|_| CoreError::Poisoned)?
            .is_some_and(|u| u <= Instant::now());
        if expired && session.guarded() {
            session.set_locked(true).await?;
        }
        Ok(expired)
    }

    pub async fn lock_writes(&self, session_id: &str) -> Result<WriteAccess> {
        let session = self.live(session_id).await?;
        if session.guarded() && self.preferences().safety.unlock_mode != UnlockMode::Open {
            let _busy = claim(&session)?;
            session.set_locked(true).await?;
            self.audit(&session, "-- writes locked", None, None);
        }
        self.access_of(&session)
    }

    pub(crate) fn audit(
        &self,
        session: &Session,
        sql: &str,
        rows: Option<u64>,
        error: Option<String>,
    ) {
        self.audit_as(
            &session.connection_id,
            session.conn.server().database.clone(),
            sql,
            rows,
            error,
        );
    }

    pub(crate) fn dialect_of(&self, connection_id: &str) -> Option<nd_sql::Dialect> {
        let live = Self::lock(&self.sessions).ok().and_then(|m| {
            m.values()
                .find(|s| s.connection_id == connection_id)
                .map(|s| s.conn.dialect())
        });
        live.unwrap_or_else(|| {
            self.store(|s| s.connection(connection_id))
                .ok()
                .flatten()
                .and_then(|p| p.target.dialect())
        })
    }

    pub(crate) fn audit_as(
        &self,
        connection_id: &str,
        database: String,
        sql: &str,
        rows: Option<u64>,
        error: Option<String>,
    ) {
        let entry = AuditEntry {
            id: 0,
            connection_id: connection_id.to_owned(),
            database,
            at: now_ms(),
            who: who(),
            sql: nd_sql::redact_secrets(sql, self.dialect_of(connection_id)).into_owned(),
            rows: rows.and_then(|r| i64::try_from(r).ok()),
            error,
        };
        if let Err(e) = self.store(|s| s.add_audit(&entry)) {
            tracing::warn!(error = %e, "audit entry not recorded");
        }
    }
}

fn rank_unlock(m: UnlockMode) -> u8 {
    match m {
        UnlockMode::Timed => 0,
        UnlockMode::Session => 1,
        UnlockMode::Open => 2,
    }
}

fn rank_confirm(m: ConfirmMode) -> u8 {
    match m {
        ConfirmMode::Typed => 0,
        ConfirmMode::Modal => 1,
        ConfirmMode::None => 2,
    }
}
