mod activity;
mod backup;
mod clickhouse;
mod compare;
mod design;
mod generate;
mod mongo;
mod monitor;
mod rowcmp;
mod search;
mod seclab;
mod timescale;
mod transfer;
mod users;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use nd_driver::{Canceller, Connection, Dialect, DriverError, Target};
use serde::Serialize;

use crate::guard::quiet;
use crate::session::{Session, connect_target};
use crate::{Core, CoreError, Result};

pub use activity::{Activity, IndexUsage, KillMode, LockWait, ServerSession, Sizes, TableSize};
pub use backup::{
    Backed, BackupFormat, BackupRequest, BackupTool, RestoreRequest, ToolCheck, ToolFamily,
};
pub use clickhouse::{
    BackgroundMerge, ClickhouseAccess, PartitionSize, ProfileSetting, RowPolicy, SettingsProfile,
};
pub use compare::{CompareRequest, Comparison};
pub use generate::{
    AutoKind, FakeColumn, FakeRequest, GEN_ROWS_MAX, GenDone, GenFault, GenForeignKey,
    GenParentRef, GenPreview, GenProfile, GenWritten,
};
pub use monitor::{
    ConnectionCount, DatabaseSize, GapReason, Maintenance, MaintenanceDone, MaintenanceRequest,
    MaintenanceTable, Monitor, MonitorGap, MonitorPart, OpenTransaction, SlowQueries, SlowQuery,
    SlowSource,
};
pub use nd_diff::design::{
    DesignCheck, DesignColumn, DesignEngine, DesignForeignKey, DesignIndex, DesignNote,
    DesignNoteCode, DesignScript, DesignTrigger, TableDesign, design_script,
};
pub use nd_diff::rows::{
    CellChange, ColumnPair, ColumnPlan, RowCounts, RowDiff, RowSide, RowState,
};
pub use nd_diff::{DiffItem, DiffState, Note as DiffNote, ObjectKind};
pub use nd_transfer::{FileFormat, Inferred, Unit};
pub(crate) use rowcmp::KeptRows;
pub use rowcmp::{
    ROW_BUDGET_MB, RowCompareEvent, RowCompareRequest, RowCompared, RowProbe, RowSource,
};
pub use search::{
    ColumnHits, SEARCH_ROWS_MAX, SearchDone, SearchEvent, SearchPlanned, SearchTable, TableHits,
    ValueSearch,
};
pub use seclab::{
    SecCategory, SecFinding, SecSeverity, SecSkip, SecSubject, SecVerdict, SecurityReport,
};
pub use timescale::HypertableSize;
pub use transfer::{
    ColumnMap, DataExport, DataExportSource, DataExported, DataImport, DataImportSample,
    DataImportSource, DataImportTarget, DataImported, NewColumn, OnRowError, RowFault,
    SampleColumn,
};
pub use users::{Grant, Principal, Principals, Quota, QuotaCap, QuotaLimit, RoleAttribute};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Engine {
    Postgres,
    Cockroach,
    Mysql,
    Mariadb,
    Mssql,
    Sqlite,
    Libsql,
    Duckdb,
    Clickhouse,
    Mongo,
    Oracle,
    Firebird,
    Hana,
    Db2,
    Odbc,
}

impl Engine {
    pub(crate) fn of(conn: &dyn Connection) -> Self {
        let product = &conn.server().product;
        if conn.capabilities().language == nd_driver::Language::Mql {
            return Self::Mongo;
        }
        match conn.dialect().unwrap_or(Dialect::Postgres) {
            Dialect::Postgres if product == "CockroachDB" => Self::Cockroach,
            Dialect::Firebird => Self::Firebird,
            Dialect::Hana => Self::Hana,
            Dialect::Db2 => Self::Db2,
            Dialect::Ansi | Dialect::AnsiBacktick => Self::Odbc,
            Dialect::Postgres
            | Dialect::Cql
            | Dialect::Partiql
            | Dialect::Opensearch
            | Dialect::Elasticsearch => Self::Postgres,
            Dialect::Mysql if product.contains("MariaDB") => Self::Mariadb,
            Dialect::Mysql => Self::Mysql,
            Dialect::Mssql => Self::Mssql,
            Dialect::Sqlite if product == "libSQL" => Self::Libsql,
            Dialect::Sqlite => Self::Sqlite,
            Dialect::Duckdb => Self::Duckdb,
            Dialect::Clickhouse => Self::Clickhouse,
            Dialect::Oracle => Self::Oracle,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ToolFault {
    Unsupported,
    Missing {
        tool: String,
    },
    Failed {
        tool: String,
        message: String,
    },
    NotRead,
    WrongExtension,
    File {
        message: String,
    },
    Empty,
    KeyMissing {
        column: String,
    },
    DuplicateKey {
        side: RowSide,
        key: nd_driver::Row,
    },
    Production,
    TooLarge {
        #[specta(type = f64)]
        rows: u64,
        limit_mb: u32,
    },
}

impl From<ToolFault> for CoreError {
    fn from(f: ToolFault) -> Self {
        Self::Tool(f)
    }
}

impl From<nd_transfer::Error> for CoreError {
    fn from(e: nd_transfer::Error) -> Self {
        Self::Tool(ToolFault::File {
            message: e.to_string(),
        })
    }
}

#[derive(Debug, Clone, Serialize, specta::Type)]
pub struct Progress {
    #[specta(type = f64)]
    pub done: u64,
    #[specta(type = Option<f64>)]
    pub total: Option<u64>,
    pub unit: nd_transfer::Unit,
    #[specta(type = f64)]
    pub rows: u64,
    #[specta(type = f64)]
    pub bytes: u64,
    pub stage: Option<String>,
}

pub(crate) struct Pace(Option<Instant>);

impl Pace {
    pub fn new() -> Self {
        Self(None)
    }

    pub fn due(&mut self) -> bool {
        let now = Instant::now();
        if self
            .0
            .is_some_and(|t| now.duration_since(t) < Duration::from_millis(120))
        {
            return false;
        }
        self.0 = Some(now);
        true
    }
}

pub(crate) struct Side {
    pub conn: Box<dyn Connection>,
}

#[derive(Default)]
pub(crate) struct Job {
    stop: AtomicBool,
    wake: tokio::sync::Notify,
    canceller: Mutex<Option<Arc<dyn Canceller>>>,
}

impl Job {
    pub fn stopped(&self) -> bool {
        self.stop.load(Ordering::Acquire)
    }

    pub fn watch(&self, canceller: Arc<dyn Canceller>) {
        if let Ok(mut c) = self.canceller.lock() {
            *c = Some(canceller);
        }
    }

    pub async fn cancelled(&self) {
        let woken = self.wake.notified();
        if self.stopped() {
            return;
        }
        woken.await;
    }
}

pub(crate) struct JobSlot<'a> {
    core: &'a Core,
    id: String,
    pub job: Arc<Job>,
}

impl Drop for JobSlot<'_> {
    fn drop(&mut self) {
        if let Ok(mut jobs) = self.core.jobs.lock() {
            jobs.remove(&self.id);
        }
    }
}

pub(crate) fn ms(t: Instant) -> u64 {
    u64::try_from(t.elapsed().as_millis()).unwrap_or(u64::MAX)
}

pub(crate) fn tail(text: &str, lines: usize) -> String {
    let kept: Vec<&str> = text.lines().filter(|l| !l.trim().is_empty()).collect();
    kept[kept.len().saturating_sub(lines)..].join("\n")
}

impl Core {
    pub(crate) fn job_start(&self, id: &str) -> Result<JobSlot<'_>> {
        let job = Arc::new(Job::default());
        Self::lock(&self.jobs)?.insert(id.to_owned(), job.clone());
        Ok(JobSlot {
            core: self,
            id: id.to_owned(),
            job,
        })
    }

    pub async fn cancel_job(&self, id: &str) -> Result<()> {
        let job = Self::lock(&self.jobs)?.get(id).cloned();
        let Some(job) = job else {
            return Ok(());
        };
        job.stop.store(true, Ordering::Release);
        job.wake.notify_waiters();
        let canceller = job.canceller.lock().ok().and_then(|c| c.clone());
        if let Some(c) = canceller {
            c.cancel().await;
        }
        Ok(())
    }

    pub(crate) async fn dial_tool(&self, session: &Session) -> Result<Side> {
        if session.caps.dialect().is_none() && session.caps.language != nd_driver::Language::Mql {
            return Err(ToolFault::Unsupported.into());
        }
        if matches!(&session.target, Target::Duckdb(t) if t.path.trim().is_empty() || t.path.trim() == ":memory:")
        {
            return Err(ToolFault::Unsupported.into());
        }
        let creds = self.creds(&session.connection_id, &session.target).await?;
        let conn = connect_target(&session.target, &creds, session.tunnel.as_deref()).await?;
        if session.guarded()
            && let Some(sql) = conn.dialect().and_then(|d| nd_sql::read_only(d, true))
        {
            quiet(conn.as_ref(), sql).await?;
        }
        crate::session::initialize(conn.as_ref(), &session.options().init).await?;
        Ok(Side { conn })
    }

    pub(crate) async fn side<'a>(&self, session: &'a Session) -> Result<SideGuard<'a>> {
        let mut guard = session.side.lock().await;
        if guard.is_none() {
            *guard = Some(self.dial_tool(session).await?);
        }
        Ok(SideGuard {
            guard,
            broken: AtomicBool::new(false),
        })
    }
}

pub(crate) struct SideGuard<'a> {
    guard: tokio::sync::MutexGuard<'a, Option<Side>>,
    broken: AtomicBool,
}

impl Drop for SideGuard<'_> {
    fn drop(&mut self) {
        if self.broken.load(Ordering::Acquire) {
            *self.guard = None;
        }
    }
}

impl SideGuard<'_> {
    pub fn conn(&self) -> Result<&dyn Connection> {
        self.guard
            .as_ref()
            .map(|s| s.conn.as_ref())
            .ok_or_else(|| CoreError::NotFound("tool connection".into()))
    }

    pub fn check<T>(&self, r: std::result::Result<T, DriverError>) -> Result<T> {
        if matches!(r, Err(DriverError::Connect { .. } | DriverError::Io { .. })) {
            self.broken.store(true, Ordering::Release);
        }
        Ok(r?)
    }
}
