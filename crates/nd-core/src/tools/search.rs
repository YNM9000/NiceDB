use crate::session::Sql as _;
use std::time::{Duration, Instant};

use nd_driver::{Chunk, Connection, DriverError, Row};
use nd_sql::{Bound, ColumnType, SearchSpec, TableRef};
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;

use super::{Job, ms};
use crate::guard::{Access, quiet};
use crate::{Core, Result};

pub const SEARCH_ROWS_MAX: u32 = 500;

#[derive(Debug, Clone, Deserialize, specta::Type)]
pub struct SearchTable {
    pub table: TableRef,
    pub columns: Vec<ColumnType>,
    pub key: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, specta::Type)]
pub struct ValueSearch {
    pub job_id: String,
    pub session_id: String,
    pub spec: SearchSpec,
    pub tables: Vec<SearchTable>,
    pub row_limit: u32,
    pub budget_secs: u32,
}

#[derive(Debug, Clone, Serialize, specta::Type)]
pub struct SearchPlanned {
    pub columns: Vec<Vec<String>>,
    pub sample: Option<String>,
}

#[derive(Debug, Clone, Serialize, specta::Type)]
pub struct ColumnHits {
    pub column: String,
    #[specta(type = f64)]
    pub count: u64,
}

#[derive(Debug, Clone, Serialize, specta::Type)]
pub struct TableHits {
    pub namespace: String,
    pub name: String,
    #[specta(type = f64)]
    pub total: u64,
    pub columns: Vec<ColumnHits>,
    pub shown: Vec<String>,
    pub key: Vec<String>,
    pub rows: Vec<Row>,
    #[specta(type = f64)]
    pub elapsed_ms: u64,
}

#[derive(Debug, Clone, Serialize, specta::Type)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum SearchEvent {
    Table {
        index: u32,
        of: u32,
        name: String,
    },
    Hits {
        hits: TableHits,
    },
    Failed {
        namespace: String,
        name: String,
        message: String,
    },
}

#[derive(Debug, Clone, Serialize, specta::Type)]
pub struct SearchDone {
    pub searched: u32,
    pub of: u32,
    pub with_hits: u32,
    #[specta(type = f64)]
    pub rows: u64,
    pub failed: u32,
    pub cancelled: bool,
    pub out_of_time: bool,
    #[specta(type = f64)]
    pub elapsed_ms: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Halt {
    Cancelled,
    OutOfTime,
}

enum Fetch {
    Rows(Vec<Row>),
    Halt(Halt),
    Failed(DriverError),
}

async fn fetch(
    conn: &dyn Connection,
    b: &Bound,
    job: &Job,
    deadline: tokio::time::Instant,
) -> Fetch {
    let (tx, mut rx) = mpsc::channel::<Chunk>(4);
    let collect = async {
        let mut rows = Vec::new();
        while let Some(chunk) = rx.recv().await {
            if let Chunk::Rows(r) = chunk {
                rows.extend(r);
            }
        }
        rows
    };
    let work = async { tokio::join!(conn.execute_bound(&b.sql, &b.params, tx), collect) };
    let halt = tokio::select! {
        biased;
        () = job.cancelled() => Halt::Cancelled,
        () = tokio::time::sleep_until(deadline) => Halt::OutOfTime,
        (out, rows) = work => return match out {
            Ok(_) => Fetch::Rows(rows),
            Err(e) => Fetch::Failed(e),
        },
    };
    conn.canceller().cancel().await;
    Fetch::Halt(halt)
}

fn searched(dialect: nd_sql::Dialect, spec: &SearchSpec, t: &SearchTable) -> Vec<ColumnType> {
    t.columns
        .iter()
        .filter(|c| nd_sql::searchable(dialect, spec, &c.type_name))
        .cloned()
        .collect()
}

fn counted(row: Option<&Row>, at: usize) -> u64 {
    row.and_then(|r| r.get(at))
        .and_then(Option::as_deref)
        .and_then(|v| v.trim().parse::<f64>().ok())
        .map_or(0, |v| {
            #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
            let n = v.max(0.0) as u64;
            n
        })
}

enum Outcome {
    Nothing,
    Hits(TableHits, Option<Halt>),
    Halt(Halt),
    Failed(DriverError),
    Broken(DriverError),
}

fn failure(t: &SearchTable, e: &DriverError) -> SearchEvent {
    SearchEvent::Failed {
        namespace: t.table.namespace.clone(),
        name: t.table.name.clone(),
        message: e.to_string(),
    }
}

struct Probe<'a> {
    conn: &'a dyn Connection,
    spec: &'a SearchSpec,
    job: &'a Job,
    deadline: tokio::time::Instant,
    limit: u32,
}

impl Probe<'_> {
    async fn table(&self, t: &SearchTable, cols: &[ColumnType]) -> Outcome {
        let at = Instant::now();
        let Ok(dialect) = self.conn.sql() else {
            return Outcome::Failed(crate::session::NotSql.into());
        };
        let count = nd_sql::search_count(dialect, &t.table, cols, self.spec);
        let row = match fetch(self.conn, &count, self.job, self.deadline).await {
            Fetch::Rows(rows) => rows.into_iter().next(),
            Fetch::Halt(h) => return Outcome::Halt(h),
            Fetch::Failed(e @ (DriverError::Connect { .. } | DriverError::Io { .. })) => {
                return Outcome::Broken(e);
            }
            Fetch::Failed(e) => return Outcome::Failed(e),
        };
        let total = counted(row.as_ref(), 0);
        if total == 0 {
            return Outcome::Nothing;
        }
        let columns: Vec<ColumnHits> = cols
            .iter()
            .enumerate()
            .map(|(j, c)| ColumnHits {
                column: c.name.clone(),
                count: counted(row.as_ref(), j + 1),
            })
            .filter(|h| h.count > 0)
            .collect();
        let key: Vec<String> = t
            .key
            .iter()
            .filter(|k| t.columns.iter().any(|c| &c.name == *k))
            .cloned()
            .collect();
        let mut shown = key.clone();
        for h in &columns {
            if !shown.contains(&h.column) {
                shown.push(h.column.clone());
            }
        }
        let hit: Vec<ColumnType> = cols
            .iter()
            .filter(|c| columns.iter().any(|h| h.column == c.name))
            .cloned()
            .collect();
        let select = nd_sql::search_rows(dialect, &t.table, &shown, &hit, self.spec, self.limit);
        let (rows, halt) = match fetch(self.conn, &select, self.job, self.deadline).await {
            Fetch::Rows(rows) => (rows, None),
            Fetch::Halt(h) => (Vec::new(), Some(h)),
            Fetch::Failed(_) => (Vec::new(), None),
        };
        let hits = TableHits {
            namespace: t.table.namespace.clone(),
            name: t.table.name.clone(),
            total,
            columns,
            shown,
            key,
            rows,
            elapsed_ms: ms(at),
        };
        Outcome::Hits(hits, halt)
    }
}

impl Core {
    pub fn search_plan(
        &self,
        session_id: &str,
        spec: &SearchSpec,
        tables: &[SearchTable],
    ) -> Result<SearchPlanned> {
        let dialect = self.session(session_id)?.conn.sql()?;
        let mut sample = None;
        let columns = tables
            .iter()
            .map(|t| {
                let cols = searched(dialect, spec, t);
                if sample.is_none() && !cols.is_empty() {
                    sample = Some(nd_sql::search_count(dialect, &t.table, &cols, spec).preview);
                }
                cols.into_iter().map(|c| c.name).collect()
            })
            .collect();
        Ok(SearchPlanned { columns, sample })
    }

    pub async fn search_value(
        &self,
        req: ValueSearch,
        events: impl Fn(SearchEvent) + Send + Sync,
    ) -> Result<SearchDone> {
        let session = self.session(&req.session_id)?;
        self.authorize(&session, Access::Read)?;
        let slot = self.job_start(&req.job_id)?;
        let started = Instant::now();
        let deadline =
            tokio::time::Instant::now() + Duration::from_secs(u64::from(req.budget_secs.max(1)));
        let side = self.dial_tool(&session).await?;
        let conn = side.conn.as_ref();
        let dialect = conn.sql()?;
        if !session.guarded()
            && let Some(sql) = nd_sql::read_only(dialect, true)
        {
            quiet(conn, sql).await?;
        }
        let limit = req.row_limit.clamp(1, SEARCH_ROWS_MAX);
        let plan: Vec<(&SearchTable, Vec<ColumnType>)> = req
            .tables
            .iter()
            .map(|t| (t, searched(dialect, &req.spec, t)))
            .filter(|(_, c)| !c.is_empty())
            .collect();
        let of = u32::try_from(plan.len()).unwrap_or(u32::MAX);
        let mut done = SearchDone {
            searched: 0,
            of,
            with_hits: 0,
            rows: 0,
            failed: 0,
            cancelled: false,
            out_of_time: false,
            elapsed_ms: 0,
        };
        for (i, (t, cols)) in plan.iter().enumerate() {
            if slot.job.stopped() {
                done.cancelled = true;
                break;
            }
            events(SearchEvent::Table {
                index: u32::try_from(i).unwrap_or(u32::MAX),
                of,
                name: t.table.name.clone(),
            });
            let probe = Probe {
                conn,
                spec: &req.spec,
                job: &slot.job,
                deadline,
                limit,
            };
            let (hits, halt) = match probe.table(t, cols).await {
                Outcome::Nothing => (None, None),
                Outcome::Hits(hits, halt) => (Some(hits), halt),
                Outcome::Halt(h) => (None, Some(h)),
                Outcome::Broken(e) => return Err(e.into()),
                Outcome::Failed(e) => {
                    done.failed += 1;
                    events(failure(t, &e));
                    (None, None)
                }
            };
            if halt.is_none() || hits.is_some() {
                done.searched += 1;
            }
            if let Some(hits) = hits {
                done.with_hits += 1;
                done.rows += hits.total;
                events(SearchEvent::Hits { hits });
            }
            if let Some(h) = halt {
                done.cancelled = h == Halt::Cancelled;
                done.out_of_time = h == Halt::OutOfTime;
                break;
            }
        }
        done.elapsed_ms = ms(started);
        Ok(done)
    }
}
