use crate::session::Sql as _;
use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};

use nd_driver::{Column, DriverError, Row};
use nd_sql::{Browse, Change, ChangeSet, ColumnType, Field, Summary, TableRef};
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;

use crate::guard::{Access, claim, collect, quiet, timed};
use crate::session::Session;
use crate::tx::CommitMode;
use crate::{Core, CoreError, Result};

const COUNT_LIMIT: Duration = Duration::from_secs(2);
const PICK_LIMIT: Duration = Duration::from_secs(3);

#[derive(Debug, Clone, Serialize, specta::Type)]
pub struct Page {
    pub columns: Vec<Column>,
    pub rows: Vec<Row>,
    pub has_more: bool,
    pub next: Option<Vec<Option<String>>>,
    #[specta(type = Option<f64>)]
    pub total: Option<u64>,
    pub estimated: bool,
    pub sql: String,
    #[specta(type = f64)]
    pub elapsed_ms: u64,
}

#[derive(Debug, Clone, Serialize, specta::Type)]
pub struct ChangePreview {
    pub begin: String,
    pub statements: Vec<String>,
    pub commit: String,
}

#[derive(Debug, Clone, Serialize, specta::Type)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ApplyOutcome {
    Applied {
        #[specta(type = f64)]
        rows: u64,
        held: bool,
    },
    Conflict {
        index: u32,
    },
    Failed {
        index: u32,
        error: DriverError,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
pub struct IdentityColumn {
    pub name: String,
    pub identity: nd_sql::Identity,
}

fn identity_query(dialect: nd_sql::Dialect) -> Option<&'static str> {
    Some(match dialect {
        nd_sql::Dialect::Postgres => {
            "SELECT column_name::text, is_identity::text, identity_generation::text, column_default::text
             FROM information_schema.columns WHERE table_schema = $1 AND table_name = $2"
        }
        nd_sql::Dialect::Mysql => {
            "SELECT column_name, extra, NULL, NULL FROM information_schema.columns
             WHERE table_schema = ? AND table_name = ?"
        }
        nd_sql::Dialect::Mssql => {
            "SELECT c.name, 'YES', 'ALWAYS', NULL FROM sys.columns c
             JOIN sys.tables t ON t.object_id = c.object_id
             JOIN sys.schemas s ON s.schema_id = t.schema_id
             WHERE s.name = @P1 AND t.name = @P2 AND c.is_identity = 1"
        }
        nd_sql::Dialect::Duckdb => {
            "SELECT column_name, NULL, NULL, column_default FROM information_schema.columns
             WHERE table_schema = ? AND table_name = ?"
        }
        nd_sql::Dialect::Sqlite => {
            "SELECT name, CASE WHEN upper(type) = 'INTEGER' AND pk = 1
               AND (SELECT count(*) FROM pragma_table_info(?2) WHERE pk > 0) = 1 THEN 'rowid' END, NULL, NULL
             FROM pragma_table_info(?2) WHERE ?1 IS NOT NULL"
        }
        _ => return None,
    })
}

fn identity_of(row: &Row) -> Option<IdentityColumn> {
    let text = |i: usize| row.get(i).cloned().flatten().unwrap_or_default();
    let (flag, generation, default) = (text(1), text(2), text(3).to_ascii_lowercase());
    let identity = if flag.eq_ignore_ascii_case("YES") && generation.eq_ignore_ascii_case("ALWAYS")
    {
        nd_sql::Identity::Always
    } else if flag.eq_ignore_ascii_case("YES")
        || flag.to_ascii_lowercase().contains("auto_increment")
        || flag == "rowid"
        || default.starts_with("nextval(")
        || default.starts_with("unique_rowid(")
    {
        nd_sql::Identity::ByDefault
    } else {
        return None;
    };
    Some(IdentityColumn {
        name: text(0),
        identity,
    })
}

#[derive(Debug, Clone, Deserialize, specta::Type)]
pub struct KeyMatch {
    pub table: TableRef,
    pub types: Vec<ColumnType>,
    pub key: Vec<Field>,
}

#[derive(Debug, Clone, Deserialize, specta::Type)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum SummarySource {
    Query { sql: String },
    Table { browse: Box<Browse> },
}

fn ms(t: Instant) -> u64 {
    u64::try_from(t.elapsed().as_millis()).unwrap_or(u64::MAX)
}

fn index(i: usize) -> u32 {
    u32::try_from(i).unwrap_or(u32::MAX)
}

fn check(dialect: nd_sql::Dialect, set: &ChangeSet) -> Result<()> {
    let keyless = set.changes.iter().any(|c| match c {
        Change::Update { key, set, .. } => key.is_empty() || set.is_empty(),
        Change::Delete { key, .. } => key.is_empty(),
        Change::Insert { .. } => false,
    });
    if keyless {
        return Err(CoreError::NoKey);
    }
    match nd_sql::invalid_value(dialect, set) {
        Some(column) => Err(CoreError::InvalidValue(column)),
        None => Ok(()),
    }
}

fn next_page(browse: &Browse, last: Option<&Row>) -> Option<Vec<Option<String>>> {
    let (columns, _) = nd_sql::keyset(browse)?;
    let last = last?;
    columns
        .iter()
        .map(|c| {
            let i = browse.columns.iter().position(|x| x == c)?;
            last.get(i).cloned().flatten().map(Some)
        })
        .collect()
}

fn count_of(rows: &[Row]) -> Option<u64> {
    rows.first()
        .and_then(|r| r.first().cloned().flatten())
        .and_then(|v| v.parse().ok())
}

pub(crate) async fn count_rows(
    session: &Session,
    queries: &[(String, Vec<Option<String>>)],
) -> Vec<Option<u64>> {
    let conn = session.conn.as_ref();
    let Some(dialect) = conn.dialect() else {
        return vec![None; queries.len()];
    };
    let open = session.in_transaction.load(Ordering::Acquire);
    let (enter, leave): (&[&str], &[&str]) = match (dialect, open) {
        (nd_sql::Dialect::Postgres, false) => (&["BEGIN READ ONLY"], &["ROLLBACK"]),
        (nd_sql::Dialect::Postgres, true) => (
            &["SAVEPOINT nd_count"],
            &[
                "ROLLBACK TO SAVEPOINT nd_count",
                "RELEASE SAVEPOINT nd_count",
            ],
        ),
        (nd_sql::Dialect::Mysql, false) => (&["START TRANSACTION READ ONLY"], &["ROLLBACK"]),
        _ => (&[], &[]),
    };
    let mut out = vec![None; queries.len()];
    for sql in enter {
        if let Err(e) = quiet(conn, sql).await {
            tracing::debug!(error = %e, "row count skipped");
            return out;
        }
    }
    let canceller = conn.canceller();
    let deadline = Instant::now() + COUNT_LIMIT;
    for (slot, (sql, params)) in out.iter_mut().zip(queries) {
        let Some(left) = deadline.checked_duration_since(Instant::now()) else {
            break;
        };
        let params = Some(params.as_slice()).filter(|p| !p.is_empty());
        match timed(collect(conn, sql, params), Some(left), &canceller).await {
            Ok((rows, _)) => *slot = count_of(&rows),
            Err(e) => {
                tracing::debug!(error = %e, "row count skipped");
                break;
            }
        }
    }
    for sql in leave {
        if let Err(e) = quiet(conn, sql).await {
            tracing::warn!(error = %e, "leaving the row count failed");
        }
    }
    out
}

impl Core {
    pub async fn identity_columns(
        &self,
        session_id: &str,
        namespace: &str,
        name: &str,
    ) -> Result<Vec<IdentityColumn>> {
        let session = self.live(session_id).await?;
        let Some(sql) = session.conn.dialect().and_then(identity_query) else {
            return Ok(Vec::new());
        };
        let _busy = claim(&session)?;
        let params = [Some(namespace.to_owned()), Some(name.to_owned())];
        let canceller = session.conn.canceller();
        let rows = timed(
            collect(session.conn.as_ref(), sql, Some(&params)),
            Some(COUNT_LIMIT),
            &canceller,
        )
        .await
        .map(|(rows, _)| rows)
        .unwrap_or_default();
        Ok(rows.iter().filter_map(identity_of).collect())
    }

    pub async fn browse(&self, session_id: &str, browse: &Browse) -> Result<Page> {
        let session = self.live(session_id).await?;
        let limit = self.timeout_for(session.environment());
        self.page(&session, browse, limit, None, true).await
    }

    pub async fn pick_rows(&self, session_id: &str, browse: &Browse, job_id: &str) -> Result<Page> {
        let session = self.live(session_id).await?;
        let slot = self.job_start(job_id)?;
        slot.job.watch(session.conn.canceller());
        let page = self
            .page(&session, browse, Some(PICK_LIMIT), Some(&slot.job), false)
            .await;
        if slot.job.stopped() {
            return Err(CoreError::Driver(DriverError::Cancelled));
        }
        page
    }

    async fn page(
        &self,
        session: &Session,
        browse: &Browse,
        limit: Option<Duration>,
        job: Option<&crate::tools::Job>,
        count: bool,
    ) -> Result<Page> {
        let _busy = claim(session)?;
        let dialect = session.conn.sql()?;
        let filter_reads = browse
            .filter
            .as_deref()
            .is_none_or(|f| nd_sql::is_read(&format!("SELECT 1 WHERE {f}"), dialect));
        let access = if filter_reads {
            Access::Read
        } else {
            Access::Write { confirm: None }
        };
        self.admit(session, access).await?;
        let canceller = session.conn.canceller();
        let started = Instant::now();

        let mut probe = browse.clone();
        probe.limit = browse.limit.saturating_add(1);
        let select = nd_sql::select(dialect, &probe);
        let (tx, mut rx) = mpsc::channel(8);
        let gather = async {
            let (mut columns, mut rows) = (Vec::new(), Vec::new());
            while let Some(chunk) = rx.recv().await {
                match chunk {
                    nd_driver::Chunk::Columns(c) => columns = c,
                    nd_driver::Chunk::Rows(r) => rows.extend(r),
                    nd_driver::Chunk::Documents(_) | nd_driver::Chunk::Progress(_) => {}
                }
            }
            (columns, rows)
        };
        if job.is_some_and(crate::tools::Job::stopped) {
            return Err(CoreError::Driver(DriverError::Cancelled));
        }
        let exec = timed(
            session.conn.execute_bound(&select.sql, &select.params, tx),
            limit,
            &canceller,
        );
        let (outcome, (columns, mut rows)) = tokio::join!(exec, gather);
        if !filter_reads {
            let error = outcome.as_ref().err().map(ToString::to_string);
            self.audit(
                session,
                &select.sql,
                outcome.as_ref().ok().and_then(|o| o.rows_affected),
                error,
            );
        }
        outcome?;
        let limit = usize::try_from(browse.limit).unwrap_or(usize::MAX);
        let has_more = rows.len() > limit;
        rows.truncate(limit);
        let next = next_page(browse, rows.last()).filter(|_| has_more);

        let (total, estimated) = if browse.offset == 0 && !has_more {
            (Some(rows.len() as u64), false)
        } else if browse.offset == 0 && count {
            self.count(session, browse).await
        } else {
            (None, false)
        };
        Ok(Page {
            columns,
            rows,
            has_more,
            next,
            total,
            estimated,
            sql: select.preview,
            elapsed_ms: ms(started),
        })
    }

    async fn count(&self, session: &Session, browse: &Browse) -> (Option<u64>, bool) {
        let Some(dialect) = session.conn.dialect() else {
            return (None, false);
        };
        let canceller = session.conn.canceller();
        if let Some(q) = nd_sql::catalog_count(dialect, browse) {
            let r = timed(
                collect(session.conn.as_ref(), &q.sql, Some(&q.params)),
                Some(COUNT_LIMIT),
                &canceller,
            )
            .await;
            if let Some(n) = r.ok().and_then(|(rows, _)| count_of(&rows)) {
                return (Some(n), true);
            }
        }
        (self.exact_count(session, browse, dialect).await, false)
    }

    async fn exact_count(
        &self,
        session: &Session,
        browse: &Browse,
        dialect: nd_sql::Dialect,
    ) -> Option<u64> {
        let q = nd_sql::count(dialect, browse);
        let canceller = session.conn.canceller();
        let r = timed(
            collect(session.conn.as_ref(), &q.sql, Some(&q.params)),
            Some(COUNT_LIMIT),
            &canceller,
        )
        .await;
        match r {
            Ok((rows, _)) => count_of(&rows),
            Err(e) => {
                tracing::debug!(error = %e, "row count skipped");
                None
            }
        }
    }

    pub async fn count_matches(
        &self,
        session_id: &str,
        matches: &[KeyMatch],
    ) -> Result<Vec<Option<u64>>> {
        let session = self.live(session_id).await?;
        let _busy = claim(&session)?;
        self.admit(&session, Access::Read).await?;
        let dialect = session.conn.sql()?;
        let queries: Vec<_> = matches
            .iter()
            .map(|m| {
                let b = nd_sql::count_key(dialect, &m.table, &m.types, &m.key);
                (b.sql, b.params)
            })
            .collect();
        Ok(count_rows(&session, &queries).await)
    }

    pub async fn summarize(
        &self,
        session_id: &str,
        source: &SummarySource,
        summary: &Summary,
    ) -> Result<Vec<Row>> {
        let session = self.live(session_id).await?;
        let _busy = claim(&session)?;
        let dialect = session.conn.sql()?;
        if !nd_sql::summarizable(dialect) {
            return Err(CoreError::Unsortable(nd_sql::Unsortable::NotSelect));
        }
        let bound = match source {
            SummarySource::Query { sql } => nd_sql::Bound {
                sql: nd_sql::summary_query(sql, dialect, summary).map_err(CoreError::Unsortable)?,
                params: Vec::new(),
                preview: String::new(),
            },
            SummarySource::Table { browse } => {
                let reads = browse
                    .filter
                    .as_deref()
                    .is_none_or(|f| nd_sql::is_read(&format!("SELECT 1 WHERE {f}"), dialect));
                if !reads {
                    return Err(CoreError::Unsortable(nd_sql::Unsortable::Writes));
                }
                nd_sql::summary_browse(dialect, browse, summary)
            }
        };
        self.admit(&session, Access::Read).await?;
        let canceller = session.conn.canceller();
        let params = Some(bound.params.as_slice()).filter(|p| !p.is_empty());
        let (rows, _) = timed(
            collect(session.conn.as_ref(), &bound.sql, params),
            self.timeout_for(session.environment()),
            &canceller,
        )
        .await?;
        Ok(rows)
    }

    pub async fn database_counts(&self, session_id: &str) -> Result<Vec<(String, u64)>> {
        let session = self.live(session_id).await?;
        let Some(dialect) = session.conn.dialect() else {
            return Ok(Vec::new());
        };
        let cockroach = session
            .conn
            .server()
            .product
            .to_lowercase()
            .contains("cockroach");
        let sql = match dialect {
            nd_sql::Dialect::Mysql => {
                "SELECT TABLE_SCHEMA, COUNT(*) FROM information_schema.TABLES GROUP BY TABLE_SCHEMA"
            }
            nd_sql::Dialect::Clickhouse => {
                "SELECT database, toString(count()) FROM system.tables GROUP BY database"
            }
            nd_sql::Dialect::Postgres if cockroach => {
                "SELECT table_catalog, count(*)::text FROM \"\".information_schema.tables \
                 WHERE table_schema NOT IN ('pg_catalog', 'information_schema', 'crdb_internal', 'pg_extension') \
                 GROUP BY table_catalog"
            }
            _ => return Ok(Vec::new()),
        };
        let _busy = claim(&session)?;
        self.admit(&session, Access::Read).await?;
        let canceller = session.conn.canceller();
        let (rows, _) = timed(
            collect(session.conn.as_ref(), sql, None),
            Some(COUNT_LIMIT),
            &canceller,
        )
        .await?;
        Ok(rows
            .iter()
            .filter_map(|r| {
                let name = r.first().cloned().flatten()?;
                let n = r.get(1).cloned().flatten()?.parse().ok()?;
                Some((name, n))
            })
            .collect())
    }

    pub fn preview_changes(&self, session_id: &str, set: &ChangeSet) -> Result<ChangePreview> {
        let dialect = self.session(session_id)?.conn.sql()?;
        check(dialect, set)?;
        Ok(ChangePreview {
            begin: nd_sql::begin(dialect).to_owned(),
            statements: set
                .changes
                .iter()
                .map(|c| nd_sql::change(dialect, &set.table, &set.columns, c).preview)
                .collect(),
            commit: "COMMIT".into(),
        })
    }

    pub async fn estimate_changes(
        &self,
        session_id: &str,
        set: &ChangeSet,
    ) -> Result<Vec<Option<u64>>> {
        let session = self.live(session_id).await?;
        let _busy = claim(&session)?;
        let dialect = session.conn.sql()?;
        check(dialect, set)?;
        self.admit(&session, Access::Read).await?;
        let mut queries = Vec::new();
        let mut at = Vec::new();
        for c in &set.changes {
            at.push(
                nd_sql::count_change(dialect, &set.table, &set.columns, c).map(|b| {
                    queries.push((b.sql, b.params));
                    queries.len() - 1
                }),
            );
        }
        let counts = count_rows(&session, &queries).await;
        Ok(at
            .into_iter()
            .map(|i| i.and_then(|i| counts.get(i).copied().flatten()))
            .collect())
    }

    pub async fn apply_changes(
        &self,
        session_id: &str,
        set: &ChangeSet,
        confirm: Option<&str>,
    ) -> Result<ApplyOutcome> {
        let session = self.live(session_id).await?;
        check(session.conn.sql()?, set)?;
        let _busy = claim(&session)?;
        let dialect = session.conn.sql()?;
        let tx = session.tx_state();
        let join = tx.open || tx.mode != CommitMode::Auto;
        let mark = savepoint(dialect);
        if tx.dry_run || tx.failed || (tx.open && mark.is_none()) {
            return Err(CoreError::TransactionOpen);
        }
        self.admit(&session, Access::Write { confirm }).await?;
        let conn = session.conn.as_ref();
        let canceller = conn.canceller();
        let limit = self.timeout_for(session.environment());

        let mut script = Vec::new();
        if !tx.open {
            if join {
                self.begin(&session, false).await?;
            } else {
                quiet(conn, nd_sql::begin(dialect)).await?;
            }
            script.push(nd_sql::begin(dialect).to_owned());
        }
        if join && let Some([enter, ..]) = mark {
            quiet(conn, enter).await?;
        }
        let mut rows = 0u64;
        let mut outcome = None;
        for (i, change) in set.changes.iter().enumerate() {
            let b = nd_sql::change(dialect, &set.table, &set.columns, change);
            script.push(b.preview);
            match timed(collect(conn, &b.sql, Some(&b.params)), limit, &canceller).await {
                Ok((_, o)) if o.rows_affected == Some(1) => rows += 1,
                Ok(_) => {
                    outcome = Some(ApplyOutcome::Conflict { index: index(i) });
                    break;
                }
                Err(error) => {
                    outcome = Some(ApplyOutcome::Failed {
                        index: index(i),
                        error,
                    });
                    break;
                }
            }
        }
        let outcome = if join {
            self.settle_joined(&session, mark, outcome, rows).await
        } else {
            script.push("COMMIT".into());
            commit_alone(conn, outcome, rows, set.changes.len()).await
        };
        let (n, error) = match &outcome {
            ApplyOutcome::Applied { rows, .. } => (Some(*rows), None),
            ApplyOutcome::Conflict { index } => {
                (None, Some(format!("conflict at change {}", index + 1)))
            }
            ApplyOutcome::Failed { error, .. } => (None, Some(error.to_string())),
        };
        self.audit(&session, &script.join(";\n"), n, error);
        if let Err(e) = self.relock_if_expired(&session).await {
            tracing::warn!(error = %e, "writes not locked again");
        }
        Ok(outcome)
    }

    async fn settle_joined(
        &self,
        session: &Session,
        mark: Option<[&'static str; 3]>,
        outcome: Option<ApplyOutcome>,
        rows: u64,
    ) -> ApplyOutcome {
        let conn = session.conn.as_ref();
        match (outcome, mark) {
            (Some(failed), Some([_, undo, release])) => {
                for sql in [undo, release].into_iter().filter(|s| !s.is_empty()) {
                    if let Err(e) = quiet(conn, sql).await {
                        tracing::warn!(error = %e, "savepoint not undone");
                    }
                }
                failed
            }
            (Some(failed), None) => {
                if let Err(e) = self.finish(session, false).await {
                    tracing::warn!(error = %e, "rollback failed");
                }
                failed
            }
            (None, mark) => {
                if let Some([_, _, release]) = mark
                    && !release.is_empty()
                    && let Err(e) = quiet(conn, release).await
                {
                    tracing::warn!(error = %e, "savepoint not released");
                }
                for _ in 0..rows {
                    session.wrote(Some(1), true);
                }
                ApplyOutcome::Applied { rows, held: true }
            }
        }
    }
}

async fn commit_alone(
    conn: &dyn nd_driver::Connection,
    outcome: Option<ApplyOutcome>,
    rows: u64,
    changes: usize,
) -> ApplyOutcome {
    if let Some(failed) = outcome {
        if let Err(e) = quiet(conn, "ROLLBACK").await {
            tracing::warn!(error = %e, "rollback failed");
        }
        return failed;
    }
    match quiet(conn, "COMMIT").await {
        Ok(()) => ApplyOutcome::Applied { rows, held: false },
        Err(error) => {
            let _ = quiet(conn, "ROLLBACK").await;
            ApplyOutcome::Failed {
                index: index(changes),
                error,
            }
        }
    }
}

fn savepoint(dialect: nd_sql::Dialect) -> Option<[&'static str; 3]> {
    match dialect {
        nd_sql::Dialect::Duckdb => None,
        nd_sql::Dialect::Oracle => {
            Some(["SAVEPOINT nd_apply", "ROLLBACK TO SAVEPOINT nd_apply", ""])
        }
        nd_sql::Dialect::Mssql => Some([
            "SAVE TRANSACTION nd_apply",
            "ROLLBACK TRANSACTION nd_apply",
            "",
        ]),
        _ => Some([
            "SAVEPOINT nd_apply",
            "ROLLBACK TO SAVEPOINT nd_apply",
            "RELEASE SAVEPOINT nd_apply",
        ]),
    }
}
