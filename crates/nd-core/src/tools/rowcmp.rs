use crate::session::Sql as _;
use std::sync::Arc;
use std::time::Instant;

use nd_diff::rows::{ColumnPlan, Differ, RowCounts, RowDiff, RowError, RowSide};
use nd_driver::{Chunk, Column, Connection, Dialect, DriverError};
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;

use super::transfer::{qualified, single_read};
use super::{Job, Pace, ToolFault, ms};
use crate::guard::Access;
use crate::{Core, CoreError, Result};

pub const ROW_BUDGET_MB: u32 = 512;
const KEEP_SAME: usize = 50_000;
const KEPT: usize = 4;

#[derive(Debug, Clone, Deserialize, specta::Type)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum RowSource {
    Query {
        session_id: String,
        sql: String,
    },
    Table {
        session_id: String,
        namespace: String,
        name: String,
    },
}

impl RowSource {
    fn session_id(&self) -> &str {
        match self {
            Self::Query { session_id, .. } | Self::Table { session_id, .. } => session_id,
        }
    }
}

#[derive(Debug, Clone, Deserialize, specta::Type)]
pub struct RowCompareRequest {
    pub job_id: String,
    pub a: RowSource,
    pub b: RowSource,
    pub key: Vec<String>,
}

#[derive(Debug, Clone, Serialize, specta::Type)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum RowCompareEvent {
    Plan {
        plan: ColumnPlan,
    },
    Rows {
        rows: Vec<RowDiff>,
    },
    Progress {
        #[specta(type = f64)]
        a_rows: u64,
        #[specta(type = f64)]
        b_rows: u64,
        #[specta(type = f64)]
        waiting: u64,
    },
}

#[derive(Debug, Clone, Serialize, specta::Type)]
pub struct RowCompared {
    pub plan: ColumnPlan,
    pub counts: RowCounts,
    #[specta(type = f64)]
    pub a_rows: u64,
    #[specta(type = f64)]
    pub b_rows: u64,
    pub same_cut: bool,
    pub cancelled: bool,
    #[specta(type = f64)]
    pub elapsed_ms: u64,
    pub a_dialect: Dialect,
    pub b_dialect: Dialect,
}

#[derive(Debug, Clone, Serialize, specta::Type)]
pub struct RowProbe {
    pub columns: Vec<Column>,
    pub dialect: Dialect,
}

pub(crate) struct KeptRows {
    seq: u64,
    dialect: Dialect,
    plan: ColumnPlan,
    key: Vec<usize>,
    rows: Vec<RowDiff>,
}

impl From<RowError> for CoreError {
    fn from(e: RowError) -> Self {
        Self::Tool(match e {
            RowError::NoKey => ToolFault::Empty,
            RowError::KeyMissing(column) => ToolFault::KeyMissing { column },
            RowError::DuplicateKey { side, key } => ToolFault::DuplicateKey { side, key },
            RowError::TooLarge { rows } => ToolFault::TooLarge {
                rows,
                limit_mb: ROW_BUDGET_MB,
            },
        })
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Probe {
    Throwaway,
    Reused,
}

async fn columns_of(conn: &dyn Connection, sql: &str, probe: Probe) -> Result<Vec<Column>> {
    let (tx, mut rx) = mpsc::channel::<Chunk>(2);
    let canceller = conn.canceller();
    let wait = async {
        let mut found = None;
        while let Some(chunk) = rx.recv().await {
            if let (Chunk::Columns(c), None) = (chunk, &found) {
                found = Some(c);
                if probe == Probe::Throwaway {
                    canceller.cancel().await;
                    break;
                }
            }
        }
        drop(rx);
        found
    };
    let (outcome, found) = tokio::join!(conn.execute(sql, tx), wait);
    match (found, outcome) {
        (Some(c), Ok(_)) if !c.is_empty() => Ok(c),
        (Some(c), _) if !c.is_empty() && probe == Probe::Throwaway => Ok(c),
        (_, Err(e)) if !matches!(e, DriverError::Cancelled) => Err(e.into()),
        _ => Err(ToolFault::NotRead.into()),
    }
}

fn source_sql(source: &RowSource, dialect: Dialect, order: Option<&[String]>) -> Result<String> {
    match source {
        RowSource::Query { sql, .. } => single_read(sql, dialect),
        RowSource::Table {
            namespace, name, ..
        } => {
            let table = qualified(dialect, namespace, name);
            Ok(match order {
                None => format!("SELECT * FROM {table} WHERE 1 = 0"),
                Some([]) => format!("SELECT * FROM {table}"),
                Some(keys) => {
                    let by = keys
                        .iter()
                        .map(|k| nd_sql::ident(dialect, k))
                        .collect::<Vec<_>>()
                        .join(", ");
                    format!("SELECT * FROM {table} ORDER BY {by}")
                }
            })
        }
    }
}

fn order_for(columns: &[Column], key: &[String]) -> Vec<String> {
    let found: Vec<String> = key
        .iter()
        .filter_map(|k| {
            columns
                .iter()
                .find(|c| c.name == *k)
                .or_else(|| columns.iter().find(|c| c.name.eq_ignore_ascii_case(k)))
                .map(|c| c.name.clone())
        })
        .collect();
    if found.len() == key.len() {
        found
    } else {
        Vec::new()
    }
}

struct Feed {
    differ: Differ,
    sent: usize,
    a_rows: u64,
    b_rows: u64,
}

async fn first_columns(rx: &mut mpsc::Receiver<Chunk>) -> Option<Vec<Column>> {
    while let Some(chunk) = rx.recv().await {
        if let Chunk::Columns(c) = chunk {
            return Some(c);
        }
    }
    None
}

impl Feed {
    fn take(
        &mut self,
        side: RowSide,
        rows: &[nd_driver::Row],
    ) -> std::result::Result<(), RowError> {
        for r in rows {
            self.differ.push(side, r)?;
        }
        match side {
            RowSide::A => self.a_rows += rows.len() as u64,
            RowSide::B => self.b_rows += rows.len() as u64,
        }
        Ok(())
    }

    fn flush(&mut self, events: &(dyn Fn(RowCompareEvent) + Send + Sync)) {
        let rows = &self.differ.rows()[self.sent..];
        if !rows.is_empty() {
            events(RowCompareEvent::Rows {
                rows: rows.to_vec(),
            });
            self.sent = self.differ.rows().len();
        }
        events(RowCompareEvent::Progress {
            a_rows: self.a_rows,
            b_rows: self.b_rows,
            waiting: self.differ.waiting() as u64,
        });
    }
}

enum Stop {
    Done,
    Cancelled,
    Failed(CoreError),
}

async fn consume(
    mut ra: mpsc::Receiver<Chunk>,
    mut rb: mpsc::Receiver<Chunk>,
    key: &[String],
    job: &Job,
    events: &(dyn Fn(RowCompareEvent) + Send + Sync),
) -> (Option<Feed>, Stop) {
    let cols = tokio::select! {
        biased;
        () = job.cancelled() => return (None, Stop::Cancelled),
        c = async { tokio::join!(first_columns(&mut ra), first_columns(&mut rb)) } => c,
    };
    let (Some(ca), Some(cb)) = cols else {
        return (None, Stop::Failed(ToolFault::NotRead.into()));
    };
    let budget = ROW_BUDGET_MB as usize * 1024 * 1024;
    let differ = match Differ::new(&ca, &cb, key, budget, KEEP_SAME) {
        Ok(d) => d,
        Err(e) => return (None, Stop::Failed(e.into())),
    };
    events(RowCompareEvent::Plan {
        plan: differ.plan().clone(),
    });
    let mut feed = Feed {
        differ,
        sent: 0,
        a_rows: 0,
        b_rows: 0,
    };
    let (mut a_open, mut b_open) = (true, true);
    let mut pace = Pace::new();
    while a_open || b_open {
        let got = tokio::select! {
            biased;
            () = job.cancelled() => return (Some(feed), Stop::Cancelled),
            c = ra.recv(), if a_open => (RowSide::A, c),
            c = rb.recv(), if b_open => (RowSide::B, c),
        };
        match got {
            (RowSide::A, None) => a_open = false,
            (RowSide::B, None) => b_open = false,
            (side, Some(Chunk::Rows(rows))) => {
                if let Err(e) = feed.take(side, &rows) {
                    return (Some(feed), Stop::Failed(e.into()));
                }
            }
            (_, Some(Chunk::Columns(_) | Chunk::Documents(_) | Chunk::Progress(_))) => {}
        }
        if pace.due() {
            feed.flush(events);
        }
    }
    (Some(feed), Stop::Done)
}

impl Core {
    pub async fn row_probe(&self, source: RowSource) -> Result<RowProbe> {
        let session = self.session(source.session_id())?;
        self.authorize(&session, Access::Read)?;
        let side = self.dial_tool(&session).await?;
        let dialect = side.conn.sql()?;
        let sql = source_sql(&source, dialect, None)?;
        Ok(RowProbe {
            columns: columns_of(side.conn.as_ref(), &sql, Probe::Throwaway).await?,
            dialect,
        })
    }

    pub async fn compare_rows(
        &self,
        req: RowCompareRequest,
        events: impl Fn(RowCompareEvent) + Send + Sync,
    ) -> Result<RowCompared> {
        if req.key.is_empty() {
            return Err(ToolFault::Empty.into());
        }
        let sa = self.session(req.a.session_id())?;
        let sb = self.session(req.b.session_id())?;
        self.authorize(&sa, Access::Read)?;
        self.authorize(&sb, Access::Read)?;
        let slot = self.job_start(&req.job_id)?;
        let started = Instant::now();
        let a = self.dial_tool(&sa).await?;
        let b = self.dial_tool(&sb).await?;
        let (ca, cb) = (a.conn.as_ref(), b.conn.as_ref());
        let (da, db) = (ca.sql()?, cb.sql()?);
        let mut sql = Vec::new();
        for (source, conn) in [(&req.a, ca), (&req.b, cb)] {
            let dialect = conn.sql()?;
            let order = match source {
                RowSource::Table { .. } => {
                    let cols = columns_of(conn, &source_sql(source, dialect, None)?, Probe::Reused)
                        .await?;
                    Some(order_for(&cols, &req.key))
                }
                RowSource::Query { .. } => None,
            };
            sql.push(source_sql(
                source,
                dialect,
                Some(order.as_deref().unwrap_or(&[])),
            )?);
        }
        let (ta, ra) = mpsc::channel::<Chunk>(4);
        let (tb, rb) = mpsc::channel::<Chunk>(4);
        let work = async {
            let out = consume(ra, rb, &req.key, &slot.job, &events).await;
            if !matches!(out.1, Stop::Done) {
                ca.canceller().cancel().await;
                cb.canceller().cancel().await;
            }
            out
        };
        let (ea, eb, (feed, stop)) =
            tokio::join!(ca.execute(&sql[0], ta), cb.execute(&sql[1], tb), work);
        let stop = match stop {
            Stop::Done => match (ea, eb) {
                (Err(e), _) | (_, Err(e)) => Stop::Failed(e.into()),
                _ => Stop::Done,
            },
            s => s,
        };
        let cancelled = match stop {
            Stop::Failed(e) => return Err(e),
            Stop::Cancelled => true,
            Stop::Done => false,
        };
        let Some(mut feed) = feed else {
            return Ok(RowCompared {
                plan: ColumnPlan::default(),
                counts: RowCounts::default(),
                a_rows: 0,
                b_rows: 0,
                same_cut: false,
                cancelled: true,
                elapsed_ms: ms(started),
                a_dialect: da,
                b_dialect: db,
            });
        };
        if !cancelled {
            feed.differ.finish();
        }
        feed.flush(&events);
        let counts = feed.differ.counts();
        let same_cut = feed.differ.same_cut();
        let (a_rows, b_rows) = (feed.a_rows, feed.b_rows);
        let (plan, key, rows) = feed.differ.into_parts();
        let done = RowCompared {
            plan: plan.clone(),
            counts,
            a_rows,
            b_rows,
            same_cut,
            cancelled,
            elapsed_ms: ms(started),
            a_dialect: da,
            b_dialect: db,
        };
        if !cancelled {
            self.keep_rows(&req.job_id, da, plan, key, rows)?;
        }
        Ok(done)
    }

    fn keep_rows(
        &self,
        id: &str,
        dialect: Dialect,
        plan: ColumnPlan,
        key: Vec<usize>,
        rows: Vec<RowDiff>,
    ) -> Result<()> {
        let mut kept = Self::lock(&self.kept_rows)?;
        let seq = kept.values().map(|k| k.seq).max().unwrap_or(0) + 1;
        let rows = rows
            .into_iter()
            .filter(|r| r.state != nd_diff::rows::RowState::Same)
            .collect();
        kept.insert(
            id.to_owned(),
            Arc::new(KeptRows {
                seq,
                dialect,
                plan,
                key,
                rows,
            }),
        );
        while kept.len() > KEPT {
            let oldest = kept
                .iter()
                .min_by_key(|(_, k)| k.seq)
                .map(|(id, _)| id.clone());
            if let Some(id) = oldest {
                kept.remove(&id);
            }
        }
        Ok(())
    }

    pub fn row_script(&self, compare_id: &str, namespace: &str, name: &str) -> Result<Vec<String>> {
        if name.trim().is_empty() {
            return Err(ToolFault::Empty.into());
        }
        let kept = Self::lock(&self.kept_rows)?
            .get(compare_id)
            .cloned()
            .ok_or_else(|| CoreError::NotFound("comparison".into()))?;
        let table = qualified(kept.dialect, namespace, name);
        Ok(nd_diff::rows::script(
            kept.dialect,
            &table,
            &kept.plan,
            &kept.key,
            &kept.rows,
        ))
    }

    pub fn row_forget(&self, compare_id: &str) -> Result<()> {
        Self::lock(&self.kept_rows)?.remove(compare_id);
        Ok(())
    }
}
