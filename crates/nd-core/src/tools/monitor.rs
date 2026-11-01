use crate::session::Sql as _;
use std::time::Instant;

use nd_driver::{Connection, DriverError, Row};
use serde::{Deserialize, Serialize};

use super::activity::{HANA_NEED, cell, hana_sees_all, num, rows, unlock_internals};
use super::transfer::qualified;
use super::{Engine, ToolFault, ms};
use crate::guard::{Access, collect, quiet};
use crate::{Core, Result};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "snake_case")]
pub enum MonitorPart {
    Connections,
    Transactions,
    Databases,
    CacheHit,
    SlowQueries,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum GapReason {
    FileDatabase,
    NotExposed,
    Permission { need: String },
    ExtensionMissing { name: String },
    ExtensionNotLoaded { name: String },
    FeatureOff { name: String },
    Failed { message: String },
}

#[derive(Debug, Clone, Serialize, specta::Type)]
pub struct MonitorGap {
    pub part: MonitorPart,
    pub reason: GapReason,
}

#[derive(Debug, Clone, Default, Serialize, specta::Type)]
pub struct ConnectionCount {
    pub total: Option<f64>,
    pub active: Option<f64>,
    pub idle_in_transaction: Option<f64>,
    pub waiting: Option<f64>,
    pub max: Option<f64>,
}

#[derive(Debug, Clone, Serialize, specta::Type)]
pub struct OpenTransaction {
    pub id: String,
    pub user: Option<String>,
    pub database: Option<String>,
    pub age_ms: Option<f64>,
    pub state: Option<String>,
    pub query: Option<String>,
}

#[derive(Debug, Clone, Serialize, specta::Type)]
pub struct DatabaseSize {
    pub name: String,
    pub bytes: Option<f64>,
}

#[derive(Debug, Clone, Default, Serialize, specta::Type)]
pub struct Monitor {
    pub connections: Option<ConnectionCount>,
    pub transactions: Vec<OpenTransaction>,
    pub databases: Vec<DatabaseSize>,
    pub cache_hit: Option<f64>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    #[specta(optional)]
    pub merges: Vec<super::BackgroundMerge>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    #[specta(optional)]
    pub long_ops: Vec<LongOp>,
    pub gaps: Vec<MonitorGap>,
}

#[derive(Debug, Clone, Serialize, specta::Type)]
pub struct LongOp {
    pub session: String,
    pub operation: String,
    pub target: Option<String>,
    pub progress: Option<f64>,
    pub elapsed_ms: Option<f64>,
    pub remaining_ms: Option<f64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "snake_case")]
pub enum SlowSource {
    PgStatStatements,
    PerformanceSchema,
    QueryStore,
    PlanCache,
    StatementStatistics,
    QueryLog,
    StatementsSummary,
}

#[derive(Debug, Clone, Serialize, specta::Type)]
pub struct SlowQuery {
    pub text: String,
    pub sample: Option<String>,
    pub calls: Option<f64>,
    pub total_ms: Option<f64>,
    pub mean_ms: Option<f64>,
    pub rows: Option<f64>,
}

#[derive(Debug, Clone, Serialize, specta::Type)]
pub struct SlowQueries {
    pub source: Option<SlowSource>,
    pub gap: Option<GapReason>,
    pub queries: Vec<SlowQuery>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "snake_case")]
pub enum Maintenance {
    Vacuum,
    Analyze,
    Optimize,
}

#[derive(Debug, Clone, Deserialize, specta::Type)]
pub struct MaintenanceRequest {
    pub session_id: String,
    pub action: Maintenance,
    pub table: Option<MaintenanceTable>,
    pub confirm: Option<String>,
}

#[derive(Debug, Clone, Deserialize, specta::Type)]
pub struct MaintenanceTable {
    pub namespace: String,
    pub name: String,
}

#[derive(Debug, Clone, Serialize, specta::Type)]
pub struct MaintenanceDone {
    pub sql: String,
    pub messages: Vec<String>,
    #[specta(type = f64)]
    pub elapsed_ms: u64,
}

const PG_CONNECTIONS: &str = "SELECT count(*)::text, count(*) FILTER (WHERE state = 'active')::text, \
count(*) FILTER (WHERE state LIKE 'idle in transaction%')::text, count(*) FILTER (WHERE wait_event_type = 'Lock')::text, \
current_setting('max_connections') FROM pg_stat_activity WHERE backend_type = 'client backend'";

const PG_TRANSACTIONS: &str = "SELECT pid::text, usename, datname, \
(extract(epoch FROM clock_timestamp() - xact_start) * 1000)::float8::text, state, query \
FROM pg_stat_activity WHERE xact_start IS NOT NULL AND pid <> pg_backend_pid() AND backend_type = 'client backend' \
ORDER BY xact_start LIMIT 20";

const PG_DATABASES: &str = "SELECT datname, CASE WHEN has_database_privilege(oid, 'CONNECT') THEN pg_database_size(oid)::text END \
FROM pg_database WHERE datallowconn AND NOT datistemplate ORDER BY pg_database_size(oid) DESC NULLS LAST";

const PG_DATABASES_SAFE: &str = "SELECT datname, CASE WHEN has_database_privilege(oid, 'CONNECT') THEN pg_database_size(oid)::text END \
FROM pg_database WHERE datallowconn AND NOT datistemplate ORDER BY datname";

const PG_CACHE: &str = "SELECT (sum(blks_hit) / nullif(sum(blks_hit) + sum(blks_read), 0))::float8::text FROM pg_stat_database";

const PG_HAS_STATEMENTS: &str =
    "SELECT extversion FROM pg_extension WHERE extname = 'pg_stat_statements'";

const PG_STATEMENTS: &str = "SELECT query, NULL, calls::text, total_exec_time::text, mean_exec_time::text, rows::text \
FROM pg_stat_statements WHERE dbid = (SELECT oid FROM pg_database WHERE datname = current_database()) \
AND query !~* '^\\s*(explain|begin|commit|rollback|set|show)\\b' ORDER BY total_exec_time DESC LIMIT 50";

const PG_STATEMENTS_OLD: &str = "SELECT query, NULL, calls::text, total_time::text, mean_time::text, rows::text \
FROM pg_stat_statements WHERE dbid = (SELECT oid FROM pg_database WHERE datname = current_database()) \
AND query !~* '^\\s*(explain|begin|commit|rollback|set|show)\\b' ORDER BY total_time DESC LIMIT 50";

const CRDB_CONNECTIONS: &str = "SELECT count(*)::STRING, count(*) FILTER (WHERE status = 'ACTIVE')::STRING, NULL::STRING, NULL::STRING, NULL::STRING \
FROM [SHOW CLUSTER SESSIONS]";

const CRDB_TRANSACTIONS: &str = "SELECT t.session_id, s.user_name, NULL::STRING, (extract(epoch FROM now() - t.start) * 1000)::FLOAT8::STRING, \
lower(s.status), coalesce(nullif(s.active_queries, ''), s.last_active_query) \
FROM crdb_internal.cluster_transactions t LEFT JOIN [SHOW CLUSTER SESSIONS] s ON s.session_id = t.session_id \
WHERE t.session_id <> (SELECT * FROM [SHOW session_id]) ORDER BY t.start LIMIT 20";

const CRDB_DATABASES: &str = "SELECT current_database(), sum((span_stats->>'approximate_disk_bytes')::INT8)::STRING \
FROM [SHOW RANGES FROM CURRENT_CATALOG WITH DETAILS]";

const CRDB_STATEMENTS: &str = "SELECT metadata->>'query', NULL::STRING, sum((statistics->'statistics'->>'cnt')::INT8)::STRING, \
sum((statistics->'statistics'->'runLat'->>'mean')::FLOAT8 * (statistics->'statistics'->>'cnt')::FLOAT8 * 1000)::STRING, NULL::STRING, \
sum((statistics->'statistics'->'numRows'->>'mean')::FLOAT8 * (statistics->'statistics'->>'cnt')::FLOAT8)::STRING \
FROM crdb_internal.statement_statistics WHERE metadata->>'db' = current_database() AND NOT (metadata->>'internal')::BOOL \
GROUP BY 1 ORDER BY 4 DESC LIMIT 50";

const MY_STATUS: &str = "SHOW GLOBAL STATUS WHERE Variable_name IN ('Threads_connected', 'Threads_running', \
'Innodb_row_lock_current_waits', 'Innodb_buffer_pool_read_requests', 'Innodb_buffer_pool_reads')";

const MY_MAX: &str = "SELECT @@max_connections";

const MY_TRANSACTIONS: &str = "SELECT t.trx_mysql_thread_id, p.USER, p.DB, timestampdiff(MICROSECOND, t.trx_started, now()) / 1000, \
t.trx_state, coalesce(t.trx_query, p.INFO) FROM information_schema.INNODB_TRX t \
LEFT JOIN information_schema.PROCESSLIST p ON p.ID = t.trx_mysql_thread_id \
WHERE t.trx_mysql_thread_id <> CONNECTION_ID() ORDER BY t.trx_started LIMIT 20";

const MY_DATABASES: &str = "SELECT TABLE_SCHEMA, SUM(DATA_LENGTH + INDEX_LENGTH) FROM information_schema.TABLES \
WHERE TABLE_SCHEMA NOT IN ('mysql', 'information_schema', 'performance_schema', 'sys') \
GROUP BY TABLE_SCHEMA ORDER BY 2 DESC";

const MY_PERFORMANCE: &str = "SELECT @@performance_schema";

const TIDB_CONNECTIONS: &str = "SELECT COUNT(*), SUM(COMMAND <> 'Sleep'), SUM(COMMAND = 'Sleep' AND TxnStart <> ''), \
(SELECT COUNT(*) FROM information_schema.DATA_LOCK_WAITS), NULLIF(@@max_connections, 0) FROM information_schema.PROCESSLIST";

const TIDB_TRANSACTIONS: &str = "SELECT t.SESSION_ID, t.USER, t.DB, timestampdiff(MICROSECOND, t.START_TIME, now()) / 1000, \
t.STATE, t.CURRENT_SQL_DIGEST_TEXT FROM information_schema.TIDB_TRX t \
WHERE t.SESSION_ID <> CONNECTION_ID() ORDER BY t.START_TIME LIMIT 20";

const TIDB_STATEMENTS: &str = "SELECT DIGEST_TEXT, QUERY_SAMPLE_TEXT, EXEC_COUNT, SUM_LATENCY / 1e6, AVG_LATENCY / 1e6, \
AVG_RESULT_ROWS * EXEC_COUNT FROM information_schema.STATEMENTS_SUMMARY WHERE SCHEMA_NAME = DATABASE() \
AND DIGEST_TEXT NOT LIKE 'explain%' AND DIGEST_TEXT NOT LIKE 'show%' AND DIGEST_TEXT NOT LIKE 'set%' \
ORDER BY SUM_LATENCY DESC LIMIT 50";

fn tidb(conn: &dyn Connection) -> bool {
    conn.edition()
        .is_some_and(|e| e.flavor == nd_driver::Flavor::Tidb)
}

const MYSQL_DIGESTS: &str = "SELECT DIGEST_TEXT, QUERY_SAMPLE_TEXT, COUNT_STAR, SUM_TIMER_WAIT / 1e9, AVG_TIMER_WAIT / 1e9, SUM_ROWS_SENT \
FROM performance_schema.events_statements_summary_by_digest WHERE SCHEMA_NAME = DATABASE() AND DIGEST_TEXT IS NOT NULL \
AND DIGEST_TEXT NOT LIKE 'EXPLAIN%' AND DIGEST_TEXT NOT LIKE 'SHOW%' AND DIGEST_TEXT NOT LIKE 'SET%' \
ORDER BY SUM_TIMER_WAIT DESC LIMIT 50";

const MARIADB_DIGESTS: &str = "SELECT DIGEST_TEXT, NULL, COUNT_STAR, SUM_TIMER_WAIT / 1e9, AVG_TIMER_WAIT / 1e9, SUM_ROWS_SENT \
FROM performance_schema.events_statements_summary_by_digest WHERE SCHEMA_NAME = DATABASE() AND DIGEST_TEXT IS NOT NULL \
AND DIGEST_TEXT NOT LIKE 'EXPLAIN%' AND DIGEST_TEXT NOT LIKE 'SHOW%' AND DIGEST_TEXT NOT LIKE 'SET%' \
ORDER BY SUM_TIMER_WAIT DESC LIMIT 50";

const MS_CONNECTIONS: &str = "SELECT CAST(COUNT(*) AS varchar(12)), \
CAST(SUM(CASE WHEN r.session_id IS NOT NULL THEN 1 ELSE 0 END) AS varchar(12)), \
CAST(SUM(CASE WHEN r.session_id IS NULL AND s.open_transaction_count > 0 THEN 1 ELSE 0 END) AS varchar(12)), \
CAST(SUM(CASE WHEN r.blocking_session_id > 0 THEN 1 ELSE 0 END) AS varchar(12)), CAST(@@MAX_CONNECTIONS AS varchar(12)) \
FROM sys.dm_exec_sessions s LEFT JOIN sys.dm_exec_requests r ON r.session_id = s.session_id WHERE s.is_user_process = 1";

const MS_TRANSACTIONS: &str = "SELECT TOP 20 CAST(st.session_id AS varchar(12)), s.login_name, DB_NAME(s.database_id), \
CAST(DATEDIFF_BIG(millisecond, at.transaction_begin_time, SYSDATETIME()) AS varchar(30)), s.status, CAST(t.text AS nvarchar(max)) \
FROM sys.dm_tran_session_transactions st JOIN sys.dm_tran_active_transactions at ON at.transaction_id = st.transaction_id \
JOIN sys.dm_exec_sessions s ON s.session_id = st.session_id LEFT JOIN sys.dm_exec_connections c ON c.session_id = st.session_id \
OUTER APPLY sys.dm_exec_sql_text(c.most_recent_sql_handle) t \
WHERE st.session_id <> @@SPID AND s.is_user_process = 1 ORDER BY at.transaction_begin_time";

const MS_DATABASES: &str = "SELECT DB_NAME(database_id), CAST(SUM(CAST(size AS bigint)) * 8192 AS varchar(30)) \
FROM sys.master_files GROUP BY database_id ORDER BY SUM(CAST(size AS bigint)) DESC";

const MS_DATABASE: &str = "SELECT DB_NAME(), CAST(SUM(CAST(size AS bigint)) * 8192 AS varchar(30)) FROM sys.database_files";

const MS_CACHE: &str = "SELECT CAST(CAST(a.cntr_value AS float) / NULLIF(b.cntr_value, 0) AS varchar(30)) \
FROM sys.dm_os_performance_counters a JOIN sys.dm_os_performance_counters b \
ON b.object_name = a.object_name AND RTRIM(b.counter_name) = 'Buffer cache hit ratio base' \
WHERE RTRIM(a.counter_name) = 'Buffer cache hit ratio'";

const MS_CAN_SEE: &str =
    "SELECT CAST(HAS_PERMS_BY_NAME(NULL, NULL, 'VIEW SERVER STATE') AS varchar(1))";

const MS_QUERY_STORE: &str = "SELECT actual_state_desc FROM sys.database_query_store_options";

const MS_STORE_QUERIES: &str = "WITH s AS (SELECT q.query_text_id, SUM(rs.count_executions) AS calls, \
SUM(rs.avg_duration * rs.count_executions) / 1000.0 AS total_ms, SUM(rs.avg_rowcount * rs.count_executions) AS row_count \
FROM sys.query_store_query q JOIN sys.query_store_plan p ON p.query_id = q.query_id \
JOIN sys.query_store_runtime_stats rs ON rs.plan_id = p.plan_id GROUP BY q.query_text_id) \
SELECT TOP 50 CAST(qt.query_sql_text AS nvarchar(max)), NULL, CAST(s.calls AS varchar(30)), CAST(s.total_ms AS varchar(30)), \
CAST(s.total_ms / NULLIF(s.calls, 0) AS varchar(30)), CAST(s.row_count AS varchar(30)) \
FROM s JOIN sys.query_store_query_text qt ON qt.query_text_id = s.query_text_id \
WHERE qt.query_sql_text NOT LIKE '%sys.query_store%' ORDER BY s.total_ms DESC";

const MS_PLAN_CACHE: &str = "SELECT TOP 50 CAST(SUBSTRING(t.text, s.statement_start_offset / 2 + 1, \
(CASE s.statement_end_offset WHEN -1 THEN DATALENGTH(t.text) ELSE s.statement_end_offset END - s.statement_start_offset) / 2 + 1) AS nvarchar(max)), \
NULL, CAST(s.execution_count AS varchar(30)), CAST(s.total_elapsed_time / 1000.0 AS varchar(30)), \
CAST(s.total_elapsed_time / 1000.0 / NULLIF(s.execution_count, 0) AS varchar(30)), CAST(s.total_rows AS varchar(30)) \
FROM sys.dm_exec_query_stats s CROSS APPLY sys.dm_exec_sql_text(s.sql_handle) t \
CROSS APPLY sys.dm_exec_plan_attributes(s.plan_handle) pa \
WHERE pa.attribute = 'dbid' AND CAST(pa.value AS int) = DB_ID() ORDER BY s.total_elapsed_time DESC";

const SQLITE_SIZE: &str = "SELECT 'main', CAST(page_count * page_size AS TEXT) FROM pragma_page_count(), pragma_page_size()";

const DUCK_SIZE: &str = "SELECT database_name, CAST(total_blocks * block_size AS VARCHAR) FROM pragma_database_size() \
WHERE database_name NOT IN ('system', 'temp')";

fn code_of(e: &DriverError) -> Option<&str> {
    match e {
        DriverError::Query { code, .. } => code.as_deref(),
        _ => None,
    }
}

fn reason(e: &DriverError, need: &str) -> GapReason {
    let text = e.to_string();
    let lower = text.to_lowercase();
    let denied = matches!(
        code_of(e),
        Some("42501" | "1142" | "1227" | "1044" | "1045" | "297" | "300" | "229" | "262")
    ) || lower.contains("permission")
        || lower.contains("denied")
        || lower.contains("privilege");
    if denied {
        GapReason::Permission { need: need.into() }
    } else {
        GapReason::Failed { message: text }
    }
}

fn first(rows: &[Row]) -> Row {
    rows.first().cloned().unwrap_or_default()
}

fn count_of(r: &Row) -> ConnectionCount {
    ConnectionCount {
        total: num(r, 0),
        active: num(r, 1),
        idle_in_transaction: num(r, 2),
        waiting: num(r, 3),
        max: num(r, 4),
    }
}

fn transaction_of(r: &Row) -> OpenTransaction {
    OpenTransaction {
        id: cell(r, 0).unwrap_or_default(),
        user: cell(r, 1),
        database: cell(r, 2),
        age_ms: num(r, 3),
        state: cell(r, 4).map(|s| s.to_lowercase()),
        query: cell(r, 5),
    }
}

fn database_of(r: &Row) -> DatabaseSize {
    DatabaseSize {
        name: cell(r, 0).unwrap_or_default(),
        bytes: num(r, 1),
    }
}

fn slow_of(r: &Row) -> SlowQuery {
    let calls = num(r, 2);
    let total_ms = num(r, 3);
    let mean_ms = num(r, 4).or_else(|| match (total_ms, calls) {
        (Some(t), Some(c)) if c > 0.0 => Some(t / c),
        _ => None,
    });
    SlowQuery {
        text: cell(r, 0).unwrap_or_default().trim().to_owned(),
        sample: cell(r, 1).map(|s| s.trim().to_owned()),
        calls,
        total_ms,
        mean_ms,
        rows: num(r, 5),
    }
}

struct Reader<'a> {
    conn: &'a dyn Connection,
    gaps: Vec<MonitorGap>,
}

impl Reader<'_> {
    async fn read(&mut self, part: MonitorPart, sql: &str, need: &str) -> Option<Vec<Row>> {
        match rows(self.conn, sql).await {
            Ok(r) => Some(r),
            Err(e) => {
                self.gap(part, reason(&e, need));
                None
            }
        }
    }

    fn gap(&mut self, part: MonitorPart, reason: GapReason) {
        self.gaps.push(MonitorGap { part, reason });
    }
}

async fn server_monitor(conn: &dyn Connection, sql: [&str; 4], need: &str, cache: bool) -> Monitor {
    let mut r = Reader {
        conn,
        gaps: Vec::new(),
    };
    let connections = r
        .read(MonitorPart::Connections, sql[0], need)
        .await
        .map(|rows| count_of(&first(&rows)));
    let transactions = r
        .read(MonitorPart::Transactions, sql[1], need)
        .await
        .map(|rows| rows.iter().map(transaction_of).collect())
        .unwrap_or_default();
    let databases = r
        .read(MonitorPart::Databases, sql[2], need)
        .await
        .map(|rows| rows.iter().map(database_of).collect())
        .unwrap_or_default();
    let cache_hit = if cache {
        r.read(MonitorPart::CacheHit, sql[3], need)
            .await
            .and_then(|rows| num(&first(&rows), 0))
    } else {
        r.gap(MonitorPart::CacheHit, GapReason::NotExposed);
        None
    };
    Monitor {
        connections,
        transactions,
        databases,
        cache_hit,
        gaps: r.gaps,
        merges: Vec::new(),
        long_ops: Vec::new(),
    }
}

async fn mysql_monitor(conn: &dyn Connection) -> Monitor {
    let mut r = Reader {
        conn,
        gaps: Vec::new(),
    };
    let status = r.read(MonitorPart::Connections, MY_STATUS, "PROCESS").await;
    let var = |name: &str| {
        status.as_ref().and_then(|rows| {
            rows.iter()
                .find(|row| cell(row, 0).is_some_and(|n| n.eq_ignore_ascii_case(name)))
                .and_then(|row| num(row, 1))
        })
    };
    let max = rows(conn, MY_MAX)
        .await
        .ok()
        .and_then(|rows| num(&first(&rows), 0));
    let connections = status.as_ref().map(|_| ConnectionCount {
        total: var("Threads_connected"),
        active: var("Threads_running"),
        idle_in_transaction: None,
        waiting: var("Innodb_row_lock_current_waits"),
        max,
    });
    let cache_hit = match (
        var("Innodb_buffer_pool_read_requests"),
        var("Innodb_buffer_pool_reads"),
    ) {
        (Some(requests), Some(reads)) if requests > 0.0 => Some(1.0 - reads / requests),
        _ => None,
    };
    if status.is_some() && cache_hit.is_none() {
        r.gap(MonitorPart::CacheHit, GapReason::NotExposed);
    }
    let transactions = r
        .read(MonitorPart::Transactions, MY_TRANSACTIONS, "PROCESS")
        .await
        .map(|rows| rows.iter().map(transaction_of).collect())
        .unwrap_or_default();
    let databases = r
        .read(MonitorPart::Databases, MY_DATABASES, "SELECT")
        .await
        .map(|rows| rows.iter().map(database_of).collect())
        .unwrap_or_default();
    Monitor {
        connections,
        transactions,
        databases,
        cache_hit,
        gaps: r.gaps,
        merges: Vec::new(),
        long_ops: Vec::new(),
    }
}

async fn mssql_monitor(conn: &dyn Connection) -> Monitor {
    let mut m = server_monitor(
        conn,
        [MS_CONNECTIONS, MS_TRANSACTIONS, MS_DATABASES, MS_CACHE],
        "VIEW SERVER STATE",
        true,
    )
    .await;
    if m.databases.is_empty() {
        m.gaps.retain(|g| g.part != MonitorPart::Databases);
        if let Ok(rows) = rows(conn, MS_DATABASE).await {
            m.databases = rows.iter().map(database_of).collect();
        }
    }
    let blind = rows(conn, MS_CAN_SEE)
        .await
        .ok()
        .and_then(|rows| cell(&first(&rows), 0))
        .as_deref()
        == Some("0");
    if blind {
        for part in [MonitorPart::Connections, MonitorPart::Transactions] {
            if !m.gaps.iter().any(|g| g.part == part) {
                m.gaps.push(MonitorGap {
                    part,
                    reason: GapReason::Permission {
                        need: "VIEW SERVER STATE".into(),
                    },
                });
            }
        }
    }
    m
}

const ORA_NEED: &str = "SELECT_CATALOG_ROLE";

const ORA_CONNECTIONS: &str = "SELECT COUNT(*), SUM(CASE WHEN s.status = 'ACTIVE' THEN 1 ELSE 0 END), \
SUM(CASE WHEN s.status <> 'ACTIVE' AND s.taddr IS NOT NULL THEN 1 ELSE 0 END), \
SUM(CASE WHEN s.blocking_session IS NOT NULL THEN 1 ELSE 0 END), MAX(p.value) \
FROM v$session s CROSS JOIN (SELECT value FROM v$parameter WHERE name = 'sessions') p WHERE s.type = 'USER'";

const ORA_TRANSACTIONS: &str = "SELECT s.sid || ',' || s.serial#, s.username, s.schemaname, \
ROUND((SYSDATE - t.start_date) * 86400000), LOWER(s.status), \
(SELECT q.sql_text FROM v$sql q WHERE q.sql_id = NVL(s.sql_id, s.prev_sql_id) FETCH FIRST 1 ROWS ONLY) \
FROM v$transaction t JOIN v$session s ON s.taddr = t.addr ORDER BY t.start_date FETCH FIRST 20 ROWS ONLY";

const ORA_DATABASES: &str =
    "SELECT SYS_CONTEXT('USERENV', 'CON_NAME'), SUM(bytes) FROM dba_data_files";

const ORA_DATABASE_SAFE: &str = "SELECT SYS_CONTEXT('USERENV', 'CON_NAME') || ' (' || USER || ')', SUM(bytes) FROM user_segments";

const ORA_CACHE: &str = "SELECT ROUND(1 - p.value / NULLIF(d.value + c.value, 0), 6) FROM v$sysstat p, v$sysstat d, v$sysstat c \
WHERE p.name = 'physical reads cache' AND d.name = 'db block gets from cache' AND c.name = 'consistent gets from cache'";

const ORA_LONGOPS: &str = "SELECT sid || ',' || serial#, opname, target, ROUND(sofar / NULLIF(totalwork, 0), 4), \
elapsed_seconds * 1000, time_remaining * 1000 FROM v$session_longops WHERE sofar < totalwork ORDER BY start_time";

const ORA_SQLAREA: &str = "SELECT SUBSTR(sql_text, 1, 1000), NULL, executions, ROUND(elapsed_time / 1000, 3), \
ROUND(elapsed_time / 1000 / NULLIF(executions, 0), 3), rows_processed FROM v$sqlarea \
WHERE parsing_schema_name NOT IN ('SYS', 'SYSTEM') AND command_type IN (2, 3, 6, 7, 189) \
ORDER BY elapsed_time DESC FETCH FIRST 50 ROWS ONLY";

const FB_CONNECTIONS: &str = "SELECT COUNT(*), \
SUM(CASE WHEN EXISTS (SELECT 1 FROM MON$STATEMENTS s WHERE s.MON$ATTACHMENT_ID = a.MON$ATTACHMENT_ID AND s.MON$STATE = 1) THEN 1 ELSE 0 END), \
SUM(CASE WHEN EXISTS (SELECT 1 FROM MON$TRANSACTIONS t WHERE t.MON$ATTACHMENT_ID = a.MON$ATTACHMENT_ID) \
  AND NOT EXISTS (SELECT 1 FROM MON$STATEMENTS s WHERE s.MON$ATTACHMENT_ID = a.MON$ATTACHMENT_ID AND s.MON$STATE = 1) THEN 1 ELSE 0 END), \
CAST(NULL AS BIGINT), CAST(NULL AS BIGINT) \
FROM MON$ATTACHMENTS a WHERE COALESCE(a.MON$SYSTEM_FLAG, 0) = 0";

const FB_TRANSACTIONS: &str = "SELECT t.MON$TRANSACTION_ID, TRIM(a.MON$USER), a.MON$ATTACHMENT_NAME, \
DATEDIFF(MILLISECOND FROM t.MON$TIMESTAMP TO CURRENT_TIMESTAMP), CASE t.MON$STATE WHEN 1 THEN 'active' ELSE 'idle' END, \
(SELECT FIRST 1 s.MON$SQL_TEXT FROM MON$STATEMENTS s WHERE s.MON$TRANSACTION_ID = t.MON$TRANSACTION_ID) \
FROM MON$TRANSACTIONS t JOIN MON$ATTACHMENTS a ON a.MON$ATTACHMENT_ID = t.MON$ATTACHMENT_ID \
WHERE t.MON$ATTACHMENT_ID <> CURRENT_CONNECTION AND COALESCE(a.MON$SYSTEM_FLAG, 0) = 0 \
ORDER BY t.MON$TIMESTAMP ROWS 20";

const FB_DATABASES: &str =
    "SELECT MON$DATABASE_NAME, CAST(MON$PAGES AS BIGINT) * MON$PAGE_SIZE FROM MON$DATABASE";

const FB_CACHE: &str = "SELECT 1 - CAST(io.MON$PAGE_READS AS DOUBLE PRECISION) / NULLIF(io.MON$PAGE_FETCHES, 0) \
FROM MON$DATABASE d JOIN MON$IO_STATS io ON io.MON$STAT_ID = d.MON$STAT_ID";

const DB2_CONNECTIONS: &str = "SELECT COUNT(*), \
(SELECT COUNT(DISTINCT APPLICATION_HANDLE) FROM TABLE(MON_GET_ACTIVITY(NULL, -2))), CAST(NULL AS BIGINT), \
(SELECT COUNT(*) FROM SYSIBMADM.MON_LOCKWAITS), CAST(NULL AS BIGINT) FROM TABLE(MON_GET_CONNECTION(NULL, -2))";

const DB2_TRANSACTIONS: &str = "SELECT CAST(u.APPLICATION_HANDLE AS VARCHAR(20)), RTRIM(c.SESSION_AUTH_ID), CURRENT SERVER, \
TIMESTAMPDIFF(2, CHAR(CURRENT TIMESTAMP - u.UOW_START_TIME)) * 1000, LOWER(u.WORKLOAD_OCCURRENCE_STATE), CAST(NULL AS VARCHAR(8)) \
FROM TABLE(MON_GET_UNIT_OF_WORK(NULL, -2)) u JOIN TABLE(MON_GET_CONNECTION(NULL, -2)) c ON c.APPLICATION_HANDLE = u.APPLICATION_HANDLE \
WHERE u.UOW_START_TIME IS NOT NULL AND u.APPLICATION_HANDLE <> MON_GET_APPLICATION_HANDLE() \
ORDER BY u.UOW_START_TIME FETCH FIRST 20 ROWS ONLY";

const DB2_DATABASES: &str = "SELECT CURRENT SERVER, SUM(TBSP_USED_PAGES * TBSP_PAGE_SIZE) FROM TABLE(MON_GET_TABLESPACE(NULL, -2))";

const DB2_CACHE: &str = "SELECT 1 - CAST(SUM(POOL_DATA_P_READS + POOL_INDEX_P_READS) AS DOUBLE) \
/ NULLIF(SUM(POOL_DATA_L_READS + POOL_INDEX_L_READS), 0) FROM TABLE(MON_GET_BUFFERPOOL(NULL, -2))";

const DB2_STATEMENTS: &str = "SELECT STMT_TEXT, CAST(NULL AS VARCHAR(8)), NUM_EXECUTIONS, TOTAL_ACT_TIME, \
CAST(NULL AS DOUBLE), ROWS_RETURNED FROM TABLE(MON_GET_PKG_CACHE_STMT(NULL, NULL, NULL, -2)) \
ORDER BY TOTAL_ACT_TIME DESC FETCH FIRST 50 ROWS ONLY";

const HANA_CONNECTIONS: &str = "SELECT COUNT(*), SUM(CASE WHEN CONNECTION_STATUS = 'RUNNING' THEN 1 ELSE 0 END), \
CAST(NULL AS BIGINT), (SELECT COUNT(*) FROM M_BLOCKED_TRANSACTIONS), CAST(NULL AS BIGINT) \
FROM M_CONNECTIONS WHERE CONNECTION_ID > 0";

const HANA_TRANSACTIONS: &str = "SELECT TO_VARCHAR(t.TRANSACTION_ID), c.USER_NAME, c.CURRENT_SCHEMA_NAME, \
SECONDS_BETWEEN(t.START_TIME, CURRENT_TIMESTAMP) * 1000, LOWER(t.TRANSACTION_STATUS), CAST(NULL AS NVARCHAR(8)) \
FROM M_TRANSACTIONS t JOIN M_CONNECTIONS c ON c.CONNECTION_ID = t.CONNECTION_ID \
WHERE t.TRANSACTION_STATUS = 'ACTIVE' AND c.OWN = 'FALSE' ORDER BY t.START_TIME LIMIT 20";

const HANA_DATABASES: &str = "SELECT DATABASE_NAME, \
(SELECT SUM(USED_SIZE) FROM M_VOLUME_FILES WHERE FILE_TYPE = 'DATA') FROM M_DATABASE";

const HANA_STATEMENTS: &str = "SELECT STATEMENT_STRING, CAST(NULL AS NVARCHAR(8)), EXECUTION_COUNT, \
TOTAL_EXECUTION_TIME / 1000, AVG_EXECUTION_TIME / 1000, TOTAL_RESULT_RECORD_COUNT \
FROM M_SQL_PLAN_CACHE ORDER BY TOTAL_EXECUTION_TIME DESC LIMIT 50";

async fn hana_monitor(conn: &dyn Connection) -> Monitor {
    if hana_sees_all(conn).await {
        return server_monitor(
            conn,
            [HANA_CONNECTIONS, HANA_TRANSACTIONS, HANA_DATABASES, ""],
            HANA_NEED,
            false,
        )
        .await;
    }
    let need = |part| MonitorGap {
        part,
        reason: GapReason::Permission {
            need: HANA_NEED.into(),
        },
    };
    Monitor {
        gaps: vec![
            need(MonitorPart::Connections),
            need(MonitorPart::Transactions),
            need(MonitorPart::Databases),
            MonitorGap {
                part: MonitorPart::CacheHit,
                reason: GapReason::NotExposed,
            },
        ],
        ..Monitor::default()
    }
}

async fn oracle_monitor(conn: &dyn Connection) -> Monitor {
    let mut m = server_monitor(
        conn,
        [ORA_CONNECTIONS, ORA_TRANSACTIONS, ORA_DATABASES, ORA_CACHE],
        ORA_NEED,
        true,
    )
    .await;
    if m.databases.is_empty()
        && let Ok(found) = rows(conn, ORA_DATABASE_SAFE).await
    {
        m.gaps.retain(|g| g.part != MonitorPart::Databases);
        m.databases = found.iter().map(database_of).collect();
    }
    if let Ok(found) = rows(conn, ORA_LONGOPS).await {
        m.long_ops = found
            .iter()
            .map(|r| LongOp {
                session: cell(r, 0).unwrap_or_default(),
                operation: cell(r, 1).unwrap_or_default(),
                target: cell(r, 2),
                progress: num(r, 3),
                elapsed_ms: num(r, 4),
                remaining_ms: num(r, 5),
            })
            .collect();
    }
    m
}

fn file_monitor(databases: Vec<DatabaseSize>) -> Monitor {
    let gaps = [
        MonitorPart::Connections,
        MonitorPart::Transactions,
        MonitorPart::CacheHit,
    ]
    .into_iter()
    .map(|part| MonitorGap {
        part,
        reason: GapReason::FileDatabase,
    })
    .collect();
    Monitor {
        databases,
        gaps,
        ..Monitor::default()
    }
}

fn slow(source: SlowSource, found: &[Row]) -> SlowQueries {
    SlowQueries {
        source: Some(source),
        gap: None,
        queries: found.iter().map(slow_of).collect(),
    }
}

fn no_slow(gap: GapReason) -> SlowQueries {
    SlowQueries {
        source: None,
        gap: Some(gap),
        queries: Vec::new(),
    }
}

const STATEMENTS: &str = "pg_stat_statements";

async fn postgres_slow(conn: &dyn Connection) -> SlowQueries {
    match rows(conn, PG_HAS_STATEMENTS).await {
        Ok(found) if found.is_empty() => {
            return no_slow(GapReason::ExtensionMissing {
                name: STATEMENTS.into(),
            });
        }
        Err(e) => return no_slow(reason(&e, "pg_read_all_stats")),
        Ok(_) => {}
    }
    let read = match rows(conn, PG_STATEMENTS).await {
        Err(e) if e.to_string().contains("total_exec_time") => rows(conn, PG_STATEMENTS_OLD).await,
        other => other,
    };
    match read {
        Ok(found) => slow(SlowSource::PgStatStatements, &found),
        Err(e) if e.to_string().contains("shared_preload_libraries") => {
            no_slow(GapReason::ExtensionNotLoaded {
                name: STATEMENTS.into(),
            })
        }
        Err(e) => no_slow(reason(&e, "pg_read_all_stats")),
    }
}

async fn mysql_slow(conn: &dyn Connection, engine: Engine) -> SlowQueries {
    let on = rows(conn, MY_PERFORMANCE)
        .await
        .ok()
        .and_then(|rows| cell(&first(&rows), 0));
    if on.as_deref() == Some("0") {
        return no_slow(GapReason::FeatureOff {
            name: "performance_schema".into(),
        });
    }
    let sql = if engine == Engine::Mariadb {
        MARIADB_DIGESTS
    } else {
        MYSQL_DIGESTS
    };
    match rows(conn, sql).await {
        Ok(found) => slow(SlowSource::PerformanceSchema, &found),
        Err(e) => no_slow(reason(&e, "SELECT ON performance_schema.*")),
    }
}

async fn mssql_slow(conn: &dyn Connection) -> SlowQueries {
    let state = rows(conn, MS_QUERY_STORE)
        .await
        .ok()
        .and_then(|rows| cell(&first(&rows), 0))
        .unwrap_or_default();
    if state.starts_with("READ") {
        match rows(conn, MS_STORE_QUERIES).await {
            Ok(found) => return slow(SlowSource::QueryStore, &found),
            Err(e) => return no_slow(reason(&e, "VIEW DATABASE STATE")),
        }
    }
    let off = GapReason::FeatureOff {
        name: "Query Store".into(),
    };
    match rows(conn, MS_PLAN_CACHE).await {
        Ok(found) => SlowQueries {
            gap: Some(off),
            ..slow(SlowSource::PlanCache, &found)
        },
        Err(_) => no_slow(off),
    }
}

pub(crate) fn maintenance_sql(
    engine: Engine,
    action: Maintenance,
    table: Option<&str>,
) -> Option<String> {
    use Maintenance::{Analyze, Optimize, Vacuum};
    let on = |head: &str| Some(table.map_or_else(|| head.to_owned(), |t| format!("{head} {t}")));
    match (engine, action) {
        (Engine::Postgres, Vacuum) => on("VACUUM (ANALYZE)"),
        (Engine::Postgres | Engine::Duckdb | Engine::Sqlite | Engine::Libsql, Analyze) => {
            on("ANALYZE")
        }
        (Engine::Cockroach, Analyze) => table.map(|t| format!("ANALYZE {t}")),
        (Engine::Mysql | Engine::Mariadb, Analyze) => table.map(|t| format!("ANALYZE TABLE {t}")),
        (Engine::Mysql | Engine::Mariadb, Optimize) => table.map(|t| format!("OPTIMIZE TABLE {t}")),
        (Engine::Mssql, Analyze) => Some(table.map_or_else(
            || "EXEC sp_updatestats".to_owned(),
            |t| format!("UPDATE STATISTICS {t}"),
        )),
        (Engine::Sqlite | Engine::Libsql, Vacuum) => table.is_none().then(|| "VACUUM".to_owned()),
        (Engine::Duckdb, Vacuum) => table.is_none().then(|| "CHECKPOINT".to_owned()),
        (Engine::Clickhouse, Optimize) => table.map(|t| format!("OPTIMIZE TABLE {t} FINAL")),
        (Engine::Oracle, Analyze) => Some(table.map_or_else(
            || "BEGIN DBMS_STATS.GATHER_SCHEMA_STATS(USER); END;".to_owned(),
            |t| {
                let lit = |s: &str| format!("'{}'", s.replace('\'', "''"));
                let (owner, name) = t.split_once("\".\"").map_or_else(
                    || ("USER".to_owned(), lit(t)),
                    |(o, n)| (lit(&format!("{o}\"")), lit(&format!("\"{n}"))),
                );
                format!("BEGIN DBMS_STATS.GATHER_TABLE_STATS({owner}, {name}); END;")
            },
        )),
        _ => None,
    }
}

impl Core {
    pub async fn monitor(&self, session_id: &str) -> Result<Monitor> {
        let session = self.live(session_id).await?;
        self.authorize(&session, Access::Read)?;
        let side = self.side(&session).await?;
        let conn = side.conn()?;
        let engine = Engine::of(conn);
        let mut m = match engine {
            Engine::Postgres => {
                let mut m = server_monitor(
                    conn,
                    [PG_CONNECTIONS, PG_TRANSACTIONS, PG_DATABASES, PG_CACHE],
                    "pg_read_all_stats",
                    true,
                )
                .await;
                if m.databases.is_empty()
                    && let Ok(found) = rows(conn, PG_DATABASES_SAFE).await
                {
                    m.gaps.retain(|g| g.part != MonitorPart::Databases);
                    m.databases = found.iter().map(database_of).collect();
                }
                m
            }
            Engine::Cockroach => {
                unlock_internals(conn).await;
                server_monitor(
                    conn,
                    [CRDB_CONNECTIONS, CRDB_TRANSACTIONS, CRDB_DATABASES, ""],
                    "VIEWACTIVITY",
                    false,
                )
                .await
            }
            Engine::Mysql if tidb(conn) => {
                server_monitor(
                    conn,
                    [TIDB_CONNECTIONS, TIDB_TRANSACTIONS, MY_DATABASES, ""],
                    "PROCESS",
                    false,
                )
                .await
            }
            Engine::Mysql | Engine::Mariadb => mysql_monitor(conn).await,
            Engine::Mssql => mssql_monitor(conn).await,
            Engine::Oracle => oracle_monitor(conn).await,
            Engine::Db2 => {
                server_monitor(
                    conn,
                    [DB2_CONNECTIONS, DB2_TRANSACTIONS, DB2_DATABASES, DB2_CACHE],
                    "SYSMON",
                    true,
                )
                .await
            }
            Engine::Hana => hana_monitor(conn).await,
            Engine::Firebird => {
                server_monitor(
                    conn,
                    [FB_CONNECTIONS, FB_TRANSACTIONS, FB_DATABASES, FB_CACHE],
                    "SYSDBA",
                    true,
                )
                .await
            }
            Engine::Clickhouse => super::clickhouse::monitor(conn).await,
            Engine::Mongo | Engine::Odbc => return Err(ToolFault::Unsupported.into()),
            Engine::Sqlite | Engine::Libsql | Engine::Duckdb => {
                let sql = if engine == Engine::Duckdb {
                    DUCK_SIZE
                } else {
                    SQLITE_SIZE
                };
                let found = rows(conn, sql).await;
                let found = side.check(found)?;
                file_monitor(found.iter().map(database_of).collect())
            }
        };
        m.gaps.dedup_by(|a, b| a.part == b.part);
        Ok(m)
    }

    pub async fn slow_queries(&self, session_id: &str) -> Result<SlowQueries> {
        let session = self.live(session_id).await?;
        self.authorize(&session, Access::Read)?;
        let side = self.side(&session).await?;
        let conn = side.conn()?;
        let engine = Engine::of(conn);
        Ok(match engine {
            Engine::Postgres => postgres_slow(conn).await,
            Engine::Cockroach => {
                unlock_internals(conn).await;
                match rows(conn, CRDB_STATEMENTS).await {
                    Ok(found) => slow(SlowSource::StatementStatistics, &found),
                    Err(e) => no_slow(reason(&e, "VIEWACTIVITY")),
                }
            }
            Engine::Mysql if tidb(conn) => match rows(conn, TIDB_STATEMENTS).await {
                Ok(found) => slow(SlowSource::StatementsSummary, &found),
                Err(e) => no_slow(reason(&e, "PROCESS")),
            },
            Engine::Mysql | Engine::Mariadb => mysql_slow(conn, engine).await,
            Engine::Mssql => mssql_slow(conn).await,
            Engine::Oracle => match rows(conn, ORA_SQLAREA).await {
                Ok(found) => slow(SlowSource::PlanCache, &found),
                Err(e) => no_slow(reason(&e, ORA_NEED)),
            },
            Engine::Sqlite | Engine::Libsql => no_slow(GapReason::FileDatabase),
            Engine::Duckdb | Engine::Mongo | Engine::Firebird | Engine::Odbc => {
                no_slow(GapReason::NotExposed)
            }
            Engine::Db2 => match rows(conn, DB2_STATEMENTS).await {
                Ok(found) => slow(SlowSource::PlanCache, &found),
                Err(e) => no_slow(reason(&e, "SYSMON")),
            },
            Engine::Hana if !hana_sees_all(conn).await => no_slow(GapReason::Permission {
                need: HANA_NEED.into(),
            }),
            Engine::Hana => match rows(conn, HANA_STATEMENTS).await {
                Ok(found) => slow(SlowSource::PlanCache, &found),
                Err(e) => no_slow(reason(&e, HANA_NEED)),
            },
            Engine::Clickhouse => super::clickhouse::slow(conn).await,
        })
    }

    pub async fn maintain(&self, req: MaintenanceRequest) -> Result<MaintenanceDone> {
        let session = self.live(&req.session_id).await?;
        let dialect = session.conn.sql()?;
        let engine = Engine::of(session.conn.as_ref());
        let table = req
            .table
            .as_ref()
            .map(|t| qualified(dialect, &t.namespace, &t.name));
        let sql =
            maintenance_sql(engine, req.action, table.as_deref()).ok_or(ToolFault::Unsupported)?;
        self.authorize(
            &session,
            Access::Write {
                confirm: req.confirm.as_deref(),
            },
        )?;
        let started = Instant::now();
        let side = self.dial_tool(&session).await?;
        let conn = side.conn.as_ref();
        if session.guarded()
            && let Some(off) = nd_sql::read_only(dialect, false)
        {
            quiet(conn, off).await?;
        }
        let done = collect(conn, &sql, None).await;
        let error = done.as_ref().err().map(ToString::to_string);
        self.audit(&session, &sql, None, error);
        let (found, _) = done?;
        let messages = if matches!(engine, Engine::Mysql | Engine::Mariadb) {
            found
                .iter()
                .filter_map(|r| {
                    let kind = cell(r, 2)?;
                    let text = cell(r, 3)?;
                    Some(format!("{kind}: {text}"))
                })
                .collect()
        } else {
            Vec::new()
        };
        Ok(MaintenanceDone {
            sql,
            messages,
            elapsed_ms: ms(started),
        })
    }
}
