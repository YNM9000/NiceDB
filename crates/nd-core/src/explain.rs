use crate::session::Sql as _;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use nd_driver::{Canceller, Chunk, Connection, Dialect, DriverError, Row};
use nd_plan::{Catalog, Engine, Finding, Format, Plan};
use nd_store::{Environment, now_ms};
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;

use crate::guard::{Access, claim, quiet, timed};
use crate::offsets::byte_to_utf16;
use crate::run::{RunHandle, RunScope, select};
use crate::session::Session;
use crate::{Core, CoreError, Result};

const CATALOG_LIMIT: Duration = Duration::from_secs(3);
const SAVEPOINT: &str = "nd_explain";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, specta::Type)]
#[serde(rename_all = "snake_case")]
pub enum ExplainMode {
    Analyze,
    Estimate,
}

#[derive(Debug, Clone, Deserialize, specta::Type)]
pub struct ExplainRequest {
    pub run_id: String,
    pub session_id: String,
    pub text: String,
    pub scope: RunScope,
    pub mode: ExplainMode,
    #[serde(default)]
    pub params: Option<Vec<nd_sql::ParamValue>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "snake_case")]
pub enum EstimateReason {
    Requested,
    ProductionWrite,
    NoSafeRollback,
    EngineCannotMeasure,
    NoSavepoint,
    NoPrivilege,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, specta::Type)]
#[serde(tag = "kind", content = "reason", rename_all = "snake_case")]
pub enum Execution {
    Measured,
    MeasuredRolledBack,
    Estimated(EstimateReason),
}

#[derive(Debug, Clone, Serialize, specta::Type)]
pub struct PlanReport {
    pub plan: Plan,
    pub findings: Vec<Finding>,
    pub execution: Execution,
    pub unreadable: Option<String>,
    pub sql: String,
    pub from: u32,
    pub to: u32,
    pub dialect: Option<Dialect>,
    #[specta(type = f64)]
    pub elapsed_ms: u64,
    #[specta(type = f64)]
    pub at: i64,
}

fn engine(conn: &dyn Connection) -> Option<Engine> {
    let product = conn.server().product.as_str();
    match conn.dialect()? {
        Dialect::Postgres if product == "CockroachDB" => Some(Engine::Cockroach),
        Dialect::Postgres => Some(Engine::Postgres),
        Dialect::Sqlite => Some(Engine::Sqlite),
        Dialect::Mysql if product == "MariaDB" => Some(Engine::Mariadb),
        Dialect::Mysql if product == "TiDB" => Some(Engine::Tidb),
        Dialect::Mysql => Some(Engine::Mysql),
        Dialect::Mssql => Some(Engine::Mssql),
        Dialect::Duckdb => Some(Engine::Duckdb),
        Dialect::Clickhouse => Some(Engine::Clickhouse),
        Dialect::Oracle => Some(Engine::Oracle),
        Dialect::Firebird => Some(Engine::Firebird),
        Dialect::Hana => Some(Engine::Hana),
        Dialect::Db2 => Some(Engine::Db2),
        Dialect::Ansi
        | Dialect::AnsiBacktick
        | Dialect::Cql
        | Dialect::Partiql
        | Dialect::Opensearch
        | Dialect::Elasticsearch => None,
    }
}

fn traced(language: nd_driver::Language) -> Option<(Engine, Format)> {
    match language {
        nd_driver::Language::Cql => Some((Engine::Cassandra, Format::CassandraTrace)),
        nd_driver::Language::Search => Some((Engine::Elasticsearch, Format::ElasticProfile)),
        _ => None,
    }
}

#[derive(Debug, PartialEq, Eq)]
struct Explain {
    setup: Vec<String>,
    sql: String,
    teardown: Option<String>,
    format: Format,
}

fn plain(sql: String, format: Format) -> Explain {
    Explain {
        setup: Vec::new(),
        sql,
        teardown: None,
        format,
    }
}

const ORACLE_STATEMENT: &str = "nd_explain";
const DB2_QUERYNO: u32 = 4242;

fn explain_sql(engine: Engine, sql: &str, measure: bool) -> Explain {
    match (engine, measure) {
        (Engine::Postgres, true) => plain(
            format!("EXPLAIN (ANALYZE, BUFFERS, FORMAT JSON) {sql}"),
            Format::PostgresJson,
        ),
        (Engine::Postgres, false) => {
            plain(format!("EXPLAIN (FORMAT JSON) {sql}"), Format::PostgresJson)
        }
        (Engine::Mysql, true) => plain(format!("EXPLAIN ANALYZE {sql}"), Format::MysqlTree),
        (Engine::Mysql, false) => plain(format!("EXPLAIN FORMAT=JSON {sql}"), Format::MysqlJson),
        (Engine::Mariadb, true) => plain(format!("ANALYZE FORMAT=JSON {sql}"), Format::MariadbJson),
        (Engine::Mariadb, false) => {
            plain(format!("EXPLAIN FORMAT=JSON {sql}"), Format::MariadbJson)
        }
        (Engine::Sqlite, _) => plain(format!("EXPLAIN QUERY PLAN {sql}"), Format::SqliteQueryPlan),
        (Engine::Mssql, true) => Explain {
            setup: vec!["SET STATISTICS XML ON".to_owned()],
            sql: sql.to_owned(),
            teardown: Some("SET STATISTICS XML OFF".to_owned()),
            format: Format::MssqlXml,
        },
        (Engine::Mssql, false) => Explain {
            setup: vec!["SET SHOWPLAN_XML ON".to_owned()],
            sql: sql.to_owned(),
            teardown: Some("SET SHOWPLAN_XML OFF".to_owned()),
            format: Format::MssqlXml,
        },
        (Engine::Duckdb, true) => plain(
            format!("EXPLAIN (ANALYZE, FORMAT JSON) {sql}"),
            Format::DuckdbJson,
        ),
        (Engine::Duckdb, false) => {
            plain(format!("EXPLAIN (FORMAT JSON) {sql}"), Format::DuckdbJson)
        }
        (Engine::Cockroach, true) => plain(
            format!("EXPLAIN ANALYZE (VERBOSE) {sql}"),
            Format::CockroachText,
        ),
        (Engine::Cockroach, false) => {
            plain(format!("EXPLAIN (VERBOSE) {sql}"), Format::CockroachText)
        }
        (Engine::Clickhouse, _) => plain(
            format!("EXPLAIN json = 1, indexes = 1, description = 1 {sql}"),
            Format::ClickhouseJson,
        ),
        (Engine::Mongodb, _) => plain(sql.to_owned(), Format::MongoJson),
        (Engine::Firebird, _) => plain(sql.to_owned(), Format::FirebirdPlan),
        (Engine::Hana | Engine::Db2, _) => vendor_explain(engine, sql),
        (Engine::Cassandra, _) => plain(sql.to_owned(), Format::CassandraTrace),
        (Engine::Elasticsearch, _) => plain(sql.to_owned(), Format::ElasticProfile),
        (Engine::Tidb, true) => plain(format!("EXPLAIN ANALYZE {sql}"), Format::TidbTable),
        (Engine::Tidb, false) => plain(format!("EXPLAIN FORMAT='brief' {sql}"), Format::TidbTable),
        (Engine::Oracle, true) => Explain {
            setup: vec![
                "ALTER SESSION SET STATISTICS_LEVEL = ALL".to_owned(),
                sql.to_owned(),
            ],
            sql: "SELECT plan_table_output FROM TABLE(DBMS_XPLAN.DISPLAY_CURSOR(NULL, NULL, 'ALLSTATS LAST'))".to_owned(),
            teardown: Some("ALTER SESSION SET STATISTICS_LEVEL = TYPICAL".to_owned()),
            format: Format::OracleXplan,
        },
        (Engine::Oracle, false) => Explain {
            setup: vec![format!(
                "EXPLAIN PLAN SET STATEMENT_ID = '{ORACLE_STATEMENT}' FOR {sql}"
            )],
            sql: format!(
                "SELECT plan_table_output FROM TABLE(DBMS_XPLAN.DISPLAY(NULL, '{ORACLE_STATEMENT}', 'TYPICAL'))"
            ),
            teardown: Some(format!(
                "DELETE FROM plan_table WHERE statement_id = '{ORACLE_STATEMENT}'"
            )),
            format: Format::OracleXplan,
        },
    }
}

fn vendor_explain(engine: Engine, sql: &str) -> Explain {
    if engine == Engine::Hana {
        return Explain {
            setup: vec![format!(
                "EXPLAIN PLAN SET STATEMENT_NAME = '{ORACLE_STATEMENT}' FOR {sql}"
            )],
            sql: format!(
                "SELECT OPERATOR_ID, PARENT_OPERATOR_ID, OPERATOR_NAME, OPERATOR_DETAILS, TABLE_NAME, \
SCHEMA_NAME, OUTPUT_SIZE, SUBTREE_COST FROM EXPLAIN_PLAN_TABLE \
WHERE STATEMENT_NAME = '{ORACLE_STATEMENT}' ORDER BY OPERATOR_ID"
            ),
            teardown: Some(format!(
                "DELETE FROM EXPLAIN_PLAN_TABLE WHERE STATEMENT_NAME = '{ORACLE_STATEMENT}'"
            )),
            format: Format::HanaPlanTable,
        };
    }
    Explain {
        setup: vec![format!(
            "EXPLAIN PLAN SET QUERYNO = {DB2_QUERYNO} FOR {sql}"
        )],
        sql: format!(
            "SELECT o.OPERATOR_ID, \
(SELECT MIN(s.TARGET_ID) FROM EXPLAIN_STREAM s WHERE s.EXPLAIN_TIME = o.EXPLAIN_TIME AND s.SOURCE_TYPE = 'O' \
  AND s.SOURCE_ID = o.OPERATOR_ID AND s.TARGET_ID > 0), \
o.OPERATOR_TYPE, CAST(NULL AS VARCHAR(1)), \
(SELECT MIN(RTRIM(s.OBJECT_NAME)) FROM EXPLAIN_STREAM s WHERE s.EXPLAIN_TIME = o.EXPLAIN_TIME AND s.SOURCE_TYPE = 'D' \
  AND s.TARGET_ID = o.OPERATOR_ID), \
(SELECT MIN(RTRIM(s.OBJECT_SCHEMA)) FROM EXPLAIN_STREAM s WHERE s.EXPLAIN_TIME = o.EXPLAIN_TIME AND s.SOURCE_TYPE = 'D' \
  AND s.TARGET_ID = o.OPERATOR_ID), \
COALESCE((SELECT MIN(s.STREAM_COUNT) FROM EXPLAIN_STREAM s WHERE s.EXPLAIN_TIME = o.EXPLAIN_TIME AND s.SOURCE_TYPE = 'O' \
  AND s.SOURCE_ID = o.OPERATOR_ID), (SELECT MAX(s.STREAM_COUNT) FROM EXPLAIN_STREAM s WHERE s.EXPLAIN_TIME = o.EXPLAIN_TIME \
  AND s.SOURCE_TYPE = 'O' AND s.TARGET_ID = o.OPERATOR_ID)), \
o.TOTAL_COST FROM EXPLAIN_OPERATOR o \
WHERE o.EXPLAIN_TIME = (SELECT MAX(st.EXPLAIN_TIME) FROM EXPLAIN_STATEMENT st WHERE st.QUERYNO = {DB2_QUERYNO}) \
ORDER BY o.OPERATOR_ID"
        ),
        teardown: Some(format!(
            "DELETE FROM EXPLAIN_INSTANCE WHERE EXPLAIN_TIME IN \
(SELECT EXPLAIN_TIME FROM EXPLAIN_STATEMENT WHERE QUERYNO = {DB2_QUERYNO})"
        )),
        format: Format::Db2Explain,
    }
}

const SHOWPLAN_COLUMN: &str = "Showplan";

async fn drain(conn: &dyn Connection, sql: &str) -> std::result::Result<(), DriverError> {
    let (tx, mut rx) = mpsc::channel(8);
    let sink = async { while rx.recv().await.is_some() {} };
    let (outcome, ()) = tokio::join!(conn.execute(sql, tx), sink);
    outcome.map(|_| ())
}

async fn plan_rows(
    conn: &dyn Connection,
    engine: Engine,
    sql: &str,
) -> std::result::Result<Vec<Row>, DriverError> {
    let (tx, mut rx) = mpsc::channel(8);
    let gather = async {
        let mut rows = Vec::new();
        let mut keep = engine != Engine::Mssql;
        while let Some(chunk) = rx.recv().await {
            match chunk {
                Chunk::Columns(cols) if engine == Engine::Mssql => {
                    keep = cols
                        .first()
                        .is_some_and(|c| c.name.contains(SHOWPLAN_COLUMN));
                }
                Chunk::Rows(r) if keep => rows.extend(r),
                _ => {}
            }
        }
        rows
    };
    let (outcome, rows) = tokio::join!(conn.execute(sql, tx), gather);
    outcome.map(|_| rows)
}

fn plan_text(engine: Engine, rows: &[Row]) -> String {
    let cell = |c: &Option<String>| c.clone().unwrap_or_default();
    match engine {
        Engine::Sqlite => rows
            .iter()
            .map(|r| r.iter().map(cell).collect::<Vec<_>>().join("|"))
            .collect::<Vec<_>>()
            .join("\n"),
        Engine::Duckdb => rows
            .iter()
            .filter_map(|r| r.get(1).or_else(|| r.first()).map(cell))
            .collect::<Vec<_>>()
            .join("\n"),
        Engine::Tidb | Engine::Hana | Engine::Db2 => rows
            .iter()
            .map(|r| r.iter().map(cell).collect::<Vec<_>>().join("\t"))
            .collect::<Vec<_>>()
            .join("\n"),
        _ => rows
            .iter()
            .filter_map(|r| r.first().map(cell))
            .collect::<Vec<_>>()
            .join("\n"),
    }
}

pub(crate) fn decide(
    engine: Engine,
    environment: Environment,
    writes: bool,
    nested: bool,
    mode: ExplainMode,
) -> Execution {
    if mode == ExplainMode::Estimate {
        return Execution::Estimated(EstimateReason::Requested);
    }
    match (engine, writes) {
        (
            Engine::Sqlite | Engine::Clickhouse | Engine::Firebird | Engine::Hana | Engine::Db2,
            _,
        ) => Execution::Estimated(EstimateReason::EngineCannotMeasure),
        (_, true) if environment == Environment::Production => {
            Execution::Estimated(EstimateReason::ProductionWrite)
        }
        (Engine::Mysql | Engine::Mariadb, true) => {
            Execution::Estimated(EstimateReason::NoSafeRollback)
        }
        (Engine::Duckdb, true) if nested => Execution::Estimated(EstimateReason::NoSavepoint),
        (_, true) => Execution::MeasuredRolledBack,
        (_, false) => Execution::Measured,
    }
}

async fn read_plan(
    conn: &dyn Connection,
    engine: Engine,
    format: Format,
    raw: &str,
    sql: &str,
    measure: bool,
    dialect: Dialect,
) -> (Plan, Option<String>, Vec<Finding>) {
    let catalog = match tokio::time::timeout(CATALOG_LIMIT, conn.schema()).await {
        Ok(Ok(schema)) => Some(Catalog::from(&schema)),
        _ => None,
    };
    let (plan, unreadable) = match nd_plan::parse(format, raw) {
        Ok(mut plan) => {
            nd_plan::annotate(&mut plan, sql);
            (plan, None)
        }
        Err(e) => (
            Plan::build(
                engine,
                measure,
                nd_plan::Tree::leaf(nd_plan::Node::new("?", nd_plan::NodeKind::Other)),
                raw,
            ),
            Some(e.to_string()),
        ),
    };
    let findings = if unreadable.is_some() {
        Vec::new()
    } else {
        nd_plan::diagnose(&plan, sql, catalog.as_ref(), dialect)
    };
    (plan, unreadable, findings)
}

impl Core {
    pub async fn explain(&self, req: ExplainRequest) -> Result<PlanReport> {
        let session = self.live(&req.session_id).await?;
        if session.caps.language == nd_driver::Language::Mql {
            return self.explain_mongo(&session, &req).await;
        }
        if let Some((engine, format)) = traced(session.caps.language) {
            return self.explain_traced(&session, &req, engine, format).await;
        }
        let _busy = claim(&session)?;
        let conn = session.conn.as_ref();
        let dialect = conn.sql()?;
        let engine = engine(conn).ok_or(CoreError::PlanUnavailable)?;
        let text = req.text.as_str();
        let st = select(text, dialect, &req.scope)
            .into_iter()
            .next()
            .ok_or_else(|| CoreError::NotFound("statement".to_owned()))?;
        let shown = crate::params::filled_text(text, dialect, &st, req.params.as_deref())
            .map_err(CoreError::Param)?;
        let sql = nd_plan::strip_explain(&shown);
        let writes = !nd_sql::is_read(sql, dialect);
        let nested = session.in_transaction.load(Ordering::Acquire);
        let mut execution = decide(engine, session.environment(), writes, nested, req.mode);
        let mut measure = !matches!(execution, Execution::Estimated(_));
        let access = if measure && writes {
            Access::Write { confirm: None }
        } else {
            Access::Read
        };
        self.admit(&session, access).await?;

        let stop = Arc::new(AtomicBool::new(false));
        let canceller = conn.canceller();
        Self::lock(&self.runs)?.insert(
            req.run_id.clone(),
            RunHandle {
                cancelled: stop.clone(),
                canceller: canceller.clone(),
            },
        );
        let started = Instant::now();
        let explain = explain_sql(engine, sql, measure);
        let format = explain.format;
        let outcome = if engine == Engine::Firebird {
            timed(
                conn.explain_native(sql),
                self.timeout_for(session.environment()),
                &canceller,
            )
            .await
            .map(|plan| vec![vec![Some(plan)]])
        } else if execution == Execution::MeasuredRolledBack {
            self.rolled_back(&session, engine, &explain, &canceller)
                .await
        } else {
            self.switched(&session, engine, &explain, &canceller).await
        };
        Self::lock(&self.runs)?.remove(&req.run_id);
        let elapsed_ms = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);
        self.record(
            &session.connection_id,
            &explain.sql,
            elapsed_ms,
            None,
            outcome.as_ref().err().map(ToString::to_string),
        );
        let mut raw = plan_text(engine, &outcome?);
        if let Some(estimate) = self
            .unmeasurable(&session, engine, format, sql, &raw, measure, &canceller)
            .await?
        {
            raw = estimate;
            execution = Execution::Estimated(EstimateReason::NoPrivilege);
            measure = false;
        }

        let (plan, unreadable, findings) =
            read_plan(conn, engine, format, &raw, sql, measure, dialect).await;
        Ok(PlanReport {
            plan,
            findings,
            execution,
            unreadable,
            sql: sql.to_owned(),
            from: byte_to_utf16(text, st.range.start),
            to: byte_to_utf16(text, st.range.end),
            dialect: Some(dialect),
            elapsed_ms,
            at: now_ms(),
        })
    }

    async fn explain_traced(
        &self,
        session: &Arc<Session>,
        req: &ExplainRequest,
        engine: Engine,
        format: Format,
    ) -> Result<PlanReport> {
        let _busy = claim(session)?;
        let conn = session.conn.as_ref();
        let text = req.text.as_str();
        let range = crate::native::select(session, text, &req.scope)
            .into_iter()
            .next()
            .ok_or_else(|| CoreError::NotFound("command".to_owned()))?;
        let command = &text[range.clone()];
        self.admit(session, Access::Read).await?;
        let stop = Arc::new(AtomicBool::new(false));
        let canceller = conn.canceller();
        Self::lock(&self.runs)?.insert(
            req.run_id.clone(),
            RunHandle {
                cancelled: stop,
                canceller: canceller.clone(),
            },
        );
        let started = Instant::now();
        let outcome = timed(
            conn.explain_native(command),
            self.timeout_for(session.environment()),
            &canceller,
        )
        .await;
        Self::lock(&self.runs)?.remove(&req.run_id);
        let elapsed_ms = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);
        self.record(
            &session.connection_id,
            command,
            elapsed_ms,
            None,
            outcome.as_ref().err().map(ToString::to_string),
        );
        let raw = outcome?;
        let (plan, unreadable) = match nd_plan::parse(format, &raw) {
            Ok(plan) => (plan, None),
            Err(e) => (
                Plan::build(
                    engine,
                    true,
                    nd_plan::Tree::leaf(nd_plan::Node::new("?", nd_plan::NodeKind::Other)),
                    &raw,
                ),
                Some(e.to_string()),
            ),
        };
        Ok(PlanReport {
            plan,
            findings: Vec::new(),
            execution: Execution::Measured,
            unreadable,
            sql: command.to_owned(),
            from: byte_to_utf16(text, range.start),
            to: byte_to_utf16(text, range.end),
            dialect: None,
            elapsed_ms,
            at: now_ms(),
        })
    }

    async fn explain_mongo(
        &self,
        session: &Arc<Session>,
        req: &ExplainRequest,
    ) -> Result<PlanReport> {
        let _busy = claim(session)?;
        let conn = session.conn.as_ref();
        let text = req.text.as_str();
        let range = crate::native::select(session, text, &req.scope)
            .into_iter()
            .next()
            .ok_or_else(|| CoreError::NotFound("command".to_owned()))?;
        let command = &text[range.clone()];
        let writes = conn.classify(command).write;
        let execution = mongo_execution(command, session.environment(), writes, req.mode);
        let measure = !matches!(execution, Execution::Estimated(_));
        let explain = mongo_explain(command, measure).ok_or(CoreError::PlanUnavailable)?;
        self.admit(session, Access::Read).await?;
        let stop = Arc::new(AtomicBool::new(false));
        let canceller = conn.canceller();
        Self::lock(&self.runs)?.insert(
            req.run_id.clone(),
            RunHandle {
                cancelled: stop,
                canceller: canceller.clone(),
            },
        );
        let started = Instant::now();
        let outcome = timed(
            documents(conn, &explain),
            self.timeout_for(session.environment()),
            &canceller,
        )
        .await;
        Self::lock(&self.runs)?.remove(&req.run_id);
        let elapsed_ms = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);
        self.record(
            &session.connection_id,
            &explain,
            elapsed_ms,
            None,
            outcome.as_ref().err().map(ToString::to_string),
        );
        let raw = outcome?.into_iter().next().unwrap_or_default();
        let catalog = match tokio::time::timeout(CATALOG_LIMIT, conn.schema()).await {
            Ok(Ok(schema)) => Some(Catalog::from(&schema)),
            _ => None,
        };
        let (plan, unreadable) = match nd_plan::parse(Format::MongoJson, &raw) {
            Ok(plan) => (plan, None),
            Err(e) => (
                Plan::build(
                    Engine::Mongodb,
                    measure,
                    nd_plan::Tree::leaf(nd_plan::Node::new("?", nd_plan::NodeKind::Other)),
                    &raw,
                ),
                Some(e.to_string()),
            ),
        };
        let findings = if unreadable.is_some() {
            Vec::new()
        } else {
            nd_plan::diagnose_mongo(&plan, catalog.as_ref())
        };
        Ok(PlanReport {
            plan,
            findings,
            execution,
            unreadable,
            sql: command.to_owned(),
            from: byte_to_utf16(text, range.start),
            to: byte_to_utf16(text, range.end),
            dialect: None,
            elapsed_ms,
            at: now_ms(),
        })
    }

    #[allow(clippy::too_many_arguments)]
    async fn unmeasurable(
        &self,
        session: &Session,
        engine: Engine,
        format: Format,
        sql: &str,
        raw: &str,
        measure: bool,
        canceller: &Arc<dyn Canceller>,
    ) -> Result<Option<String>> {
        if engine != Engine::Oracle
            || !measure
            || nd_plan::parse(format, raw).is_ok()
            || !raw.contains("privilege")
        {
            return Ok(None);
        }
        let estimate = explain_sql(engine, sql, false);
        let rows = self.switched(session, engine, &estimate, canceller).await?;
        Ok(Some(plan_text(engine, &rows)))
    }

    async fn switched(
        &self,
        session: &Session,
        engine: Engine,
        explain: &Explain,
        canceller: &Arc<dyn Canceller>,
    ) -> std::result::Result<Vec<Row>, DriverError> {
        let conn = session.conn.as_ref();
        let limit = self.timeout_for(session.environment());
        let mut out = Ok(Vec::new());
        let mut ran = 0;
        for step in &explain.setup {
            if let Err(e) = timed(drain(conn, step), limit, canceller).await {
                out = Err(e);
                break;
            }
            ran += 1;
        }
        if out.is_ok() {
            out = timed(plan_rows(conn, engine, &explain.sql), limit, canceller).await;
        }
        if ran > 0 || explain.setup.is_empty() {
            self.switch_back(conn, explain, &out).await?;
        }
        out
    }

    async fn switch_back(
        &self,
        conn: &dyn Connection,
        explain: &Explain,
        out: &std::result::Result<Vec<Row>, DriverError>,
    ) -> std::result::Result<(), DriverError> {
        if let Some(teardown) = explain.teardown.as_deref() {
            let back = quiet(conn, teardown).await;
            if let Err(e) = &back {
                tracing::error!(error = %e, "explain could not switch the session back");
            }
            if out.is_ok() {
                back?;
            }
        }
        Ok(())
    }

    async fn rolled_back(
        &self,
        session: &Session,
        engine: Engine,
        explain: &Explain,
        canceller: &Arc<dyn Canceller>,
    ) -> std::result::Result<Vec<Row>, DriverError> {
        let conn = session.conn.as_ref();
        let nested = session.in_transaction.load(Ordering::Acquire);
        let (open, undo) = if nested && engine == Engine::Mssql {
            (
                format!("SAVE TRANSACTION {SAVEPOINT}"),
                format!("ROLLBACK TRANSACTION {SAVEPOINT}"),
            )
        } else if nested {
            (
                format!("SAVEPOINT {SAVEPOINT}"),
                format!("ROLLBACK TO SAVEPOINT {SAVEPOINT}"),
            )
        } else {
            (nd_sql::begin(conn.sql()?).to_owned(), "ROLLBACK".to_owned())
        };
        quiet(conn, &open).await?;
        let out = self.switched(session, engine, explain, canceller).await;
        let undone = quiet(conn, &undo).await;
        if nested && undone.is_ok() && !matches!(engine, Engine::Mssql | Engine::Oracle) {
            let _ = quiet(conn, &format!("RELEASE SAVEPOINT {SAVEPOINT}")).await;
        }
        if let Err(e) = undone {
            tracing::error!(error = %e, "explain rollback failed");
            if !nested {
                session.in_transaction.store(true, Ordering::Release);
            }
            return Err(e);
        }
        out
    }
}

const MONGO_EXPLAINABLE: &[&str] = &[
    "find",
    "aggregate",
    "count",
    "distinct",
    "update",
    "delete",
    "findAndModify",
];

fn mongo_execution(
    command: &str,
    environment: Environment,
    writes: bool,
    mode: ExplainMode,
) -> Execution {
    if mode == ExplainMode::Estimate {
        return Execution::Estimated(EstimateReason::Requested);
    }
    if !writes {
        return Execution::Measured;
    }
    if environment == Environment::Production {
        return Execution::Estimated(EstimateReason::ProductionWrite);
    }
    if command.contains("\"aggregate\"") {
        return Execution::Estimated(EstimateReason::EngineCannotMeasure);
    }
    Execution::Measured
}

fn mongo_explain(command: &str, measure: bool) -> Option<String> {
    let clean: String = command
        .lines()
        .map(|l| {
            let t = l.trim_start();
            if t.starts_with("//") { "" } else { l }
        })
        .collect::<Vec<_>>()
        .join("\n");
    let mut cmd: serde_json::Map<String, serde_json::Value> = serde_json::from_str(&clean).ok()?;
    let name = cmd.keys().next()?.clone();
    if !MONGO_EXPLAINABLE.contains(&name.as_str()) {
        return None;
    }
    let db = cmd.remove("$db");
    cmd.remove("comment");
    if name == "aggregate" && !cmd.contains_key("cursor") {
        cmd.insert("cursor".into(), serde_json::json!({}));
    }
    let mut out = serde_json::Map::new();
    out.insert("explain".into(), serde_json::Value::Object(cmd));
    out.insert(
        "verbosity".into(),
        (if measure {
            "executionStats"
        } else {
            "queryPlanner"
        })
        .into(),
    );
    if let Some(db) = db {
        out.insert("$db".into(), db);
    }
    serde_json::to_string(&serde_json::Value::Object(out)).ok()
}

async fn documents(
    conn: &dyn Connection,
    command: &str,
) -> std::result::Result<Vec<String>, DriverError> {
    let (tx, mut rx) = mpsc::channel(8);
    let gather = async {
        let mut docs = Vec::new();
        while let Some(chunk) = rx.recv().await {
            if let Chunk::Documents(d) = chunk {
                docs.extend(d);
            }
        }
        docs
    };
    let (outcome, docs) = tokio::join!(conn.execute(command, tx), gather);
    outcome.map(|_| docs)
}
