use nd_driver::{Connection, Row};
use serde::{Deserialize, Serialize};

use super::{Engine, ToolFault};
use crate::guard::{Access, collect};
use crate::{Core, Result};

#[derive(Debug, Clone, Serialize, specta::Type)]
pub struct ServerSession {
    pub id: String,
    pub user: Option<String>,
    pub database: Option<String>,
    pub client: Option<String>,
    pub application: Option<String>,
    pub state: Option<String>,
    pub query: Option<String>,
    pub duration_ms: Option<f64>,
    pub wait: Option<String>,
    pub blocked_by: Vec<String>,
}

#[derive(Debug, Clone, Serialize, specta::Type)]
pub struct LockWait {
    pub waiting: String,
    pub blocking: String,
    pub object: Option<String>,
    pub mode: Option<String>,
    pub waited_ms: Option<f64>,
}

#[derive(Debug, Clone, Serialize, specta::Type)]
pub struct Activity {
    pub sessions: Vec<ServerSession>,
    pub locks: Vec<LockWait>,
    pub partial: bool,
    pub can_cancel: bool,
}

#[derive(Debug, Clone, Serialize, specta::Type)]
pub struct TableSize {
    pub namespace: String,
    pub name: String,
    pub rows: Option<f64>,
    pub data_bytes: Option<f64>,
    pub index_bytes: Option<f64>,
    pub total_bytes: Option<f64>,
}

#[derive(Debug, Clone, Serialize, specta::Type)]
pub struct IndexUsage {
    pub namespace: String,
    pub table: String,
    pub name: String,
    pub bytes: Option<f64>,
    pub scans: Option<f64>,
    pub unique: bool,
    pub unused: bool,
}

#[derive(Debug, Clone, Serialize, specta::Type)]
pub struct Sizes {
    pub tables: Vec<TableSize>,
    pub indexes: Vec<IndexUsage>,
    pub usage_known: bool,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    #[specta(optional)]
    pub partitions: Vec<super::PartitionSize>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    #[specta(optional)]
    pub hypertables: Vec<super::HypertableSize>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, specta::Type)]
#[serde(rename_all = "snake_case")]
pub enum KillMode {
    Cancel,
    Terminate,
}

pub(crate) fn cell(r: &Row, i: usize) -> Option<String> {
    r.get(i).cloned().flatten().filter(|s| !s.is_empty())
}

pub(crate) fn num(r: &Row, i: usize) -> Option<f64> {
    cell(r, i).and_then(|s| s.parse().ok())
}

fn my_table(r: &Row) -> TableSize {
    TableSize {
        namespace: cell(r, 0).unwrap_or_default(),
        name: cell(r, 1).unwrap_or_default(),
        rows: num(r, 2),
        data_bytes: num(r, 3),
        index_bytes: num(r, 4),
        total_bytes: num(r, 5),
    }
}

pub(crate) async fn rows(
    conn: &dyn Connection,
    sql: &str,
) -> std::result::Result<Vec<Row>, nd_driver::DriverError> {
    collect(conn, sql, None).await.map(|(rows, _)| rows)
}

const PG_SESSIONS: &str = "SELECT pid::text, usename, datname, coalesce(client_addr::text, client_hostname), \
application_name, state, query, \
CASE WHEN state <> 'idle' THEN (extract(epoch FROM clock_timestamp() - coalesce(query_start, xact_start)) * 1000)::float8::text END, \
nullif(concat_ws(': ', wait_event_type, wait_event), ''), array_to_string(pg_blocking_pids(pid), ',') \
FROM pg_stat_activity WHERE pid <> pg_backend_pid() AND backend_type = 'client backend' \
ORDER BY state = 'idle', query_start";

const PG_LOCKS: &str = "SELECT w.pid::text, b.pid::text, coalesce(w.relation::regclass::text, w.locktype), w.mode, \
(extract(epoch FROM clock_timestamp() - a.query_start) * 1000)::float8::text \
FROM pg_locks w JOIN pg_stat_activity a ON a.pid = w.pid \
CROSS JOIN LATERAL unnest(pg_blocking_pids(w.pid)) AS b(pid) WHERE NOT w.granted";

const MY_SESSIONS: &str = "SELECT ID, USER, DB, HOST, NULL, COMMAND, INFO, TIME * 1000, STATE \
FROM information_schema.PROCESSLIST WHERE ID <> CONNECTION_ID() AND COMMAND <> 'Daemon' \
AND USER NOT IN ('system user', 'event_scheduler') ORDER BY COMMAND = 'Sleep', TIME DESC";

const MYSQL_LOCKS: &str = "SELECT r.trx_mysql_thread_id, b.trx_mysql_thread_id, \
concat(l.OBJECT_SCHEMA, '.', l.OBJECT_NAME), l.LOCK_MODE, timestampdiff(SECOND, r.trx_wait_started, now()) * 1000 \
FROM performance_schema.data_lock_waits w \
JOIN information_schema.INNODB_TRX r ON r.trx_id = w.REQUESTING_ENGINE_TRANSACTION_ID \
JOIN information_schema.INNODB_TRX b ON b.trx_id = w.BLOCKING_ENGINE_TRANSACTION_ID \
JOIN performance_schema.data_locks l ON l.ENGINE_LOCK_ID = w.REQUESTING_ENGINE_LOCK_ID";

const MY_METADATA_LOCKS: &str = "SELECT tw.PROCESSLIST_ID, tg.PROCESSLIST_ID, \
concat(w.OBJECT_SCHEMA, '.', w.OBJECT_NAME), concat('metadata ', w.LOCK_TYPE), tw.PROCESSLIST_TIME * 1000 \
FROM performance_schema.metadata_locks w \
JOIN performance_schema.metadata_locks g ON g.OBJECT_TYPE = w.OBJECT_TYPE \
AND g.OBJECT_SCHEMA <=> w.OBJECT_SCHEMA AND g.OBJECT_NAME <=> w.OBJECT_NAME \
AND g.LOCK_STATUS = 'GRANTED' AND g.OWNER_THREAD_ID <> w.OWNER_THREAD_ID \
JOIN performance_schema.threads tw ON tw.THREAD_ID = w.OWNER_THREAD_ID \
JOIN performance_schema.threads tg ON tg.THREAD_ID = g.OWNER_THREAD_ID \
WHERE w.LOCK_STATUS = 'PENDING' AND w.OBJECT_TYPE IN ('TABLE', 'SCHEMA') \
AND tw.PROCESSLIST_ID IS NOT NULL AND tg.PROCESSLIST_ID IS NOT NULL \
AND NOT (w.LOCK_TYPE IN ('SHARED', 'SHARED_HIGH_PRIO', 'SHARED_READ', 'SHARED_WRITE') \
AND g.LOCK_TYPE IN ('SHARED', 'SHARED_HIGH_PRIO', 'SHARED_READ', 'SHARED_WRITE', 'INTENTION_EXCLUSIVE'))";

const MARIADB_LOCKS: &str = "SELECT r.trx_mysql_thread_id, b.trx_mysql_thread_id, l.lock_table, l.lock_mode, \
timestampdiff(SECOND, r.trx_wait_started, now()) * 1000 \
FROM information_schema.INNODB_LOCK_WAITS w \
JOIN information_schema.INNODB_TRX r ON r.trx_id = w.requesting_trx_id \
JOIN information_schema.INNODB_TRX b ON b.trx_id = w.blocking_trx_id \
JOIN information_schema.INNODB_LOCKS l ON l.lock_id = w.requested_lock_id";

const PG_TABLES: &str = "SELECT n.nspname, c.relname, CASE WHEN c.reltuples >= 0 THEN c.reltuples::float8::text END, \
pg_table_size(c.oid)::text, pg_indexes_size(c.oid)::text, pg_total_relation_size(c.oid)::text \
FROM pg_class c JOIN pg_namespace n ON n.oid = c.relnamespace \
WHERE c.relkind IN ('r', 'p', 'm') AND n.nspname NOT IN ('pg_catalog', 'information_schema') \
AND n.nspname NOT LIKE 'pg_toast%' ORDER BY pg_total_relation_size(c.oid) DESC LIMIT 1000";

const PG_INDEXES: &str = "SELECT s.schemaname, s.relname, s.indexrelname, pg_relation_size(s.indexrelid)::text, \
s.idx_scan::text, (i.indisunique OR i.indisprimary)::text \
FROM pg_stat_user_indexes s JOIN pg_index i ON i.indexrelid = s.indexrelid \
ORDER BY pg_relation_size(s.indexrelid) DESC LIMIT 2000";

const MY_TABLES: &str = "SELECT TABLE_SCHEMA, TABLE_NAME, TABLE_ROWS, DATA_LENGTH, INDEX_LENGTH, DATA_LENGTH + INDEX_LENGTH \
FROM information_schema.TABLES WHERE TABLE_SCHEMA = DATABASE() AND TABLE_TYPE = 'BASE TABLE' \
ORDER BY DATA_LENGTH + INDEX_LENGTH DESC";

const MY_INDEXES: &str = "SELECT TABLE_SCHEMA, TABLE_NAME, INDEX_NAME, MIN(NON_UNIQUE) \
FROM information_schema.STATISTICS WHERE TABLE_SCHEMA = DATABASE() GROUP BY TABLE_SCHEMA, TABLE_NAME, INDEX_NAME";

const MY_INDEX_BYTES: &str = "SELECT database_name, table_name, index_name, stat_value * @@innodb_page_size \
FROM mysql.innodb_index_stats WHERE stat_name = 'size' AND database_name = DATABASE()";

const MYSQL_INDEX_SCANS: &str = "SELECT OBJECT_SCHEMA, OBJECT_NAME, INDEX_NAME, COUNT_STAR \
FROM performance_schema.table_io_waits_summary_by_index_usage \
WHERE OBJECT_SCHEMA = DATABASE() AND INDEX_NAME IS NOT NULL";

const MS_SESSIONS: &str = "SELECT CAST(s.session_id AS varchar(12)), s.login_name, DB_NAME(s.database_id), s.host_name, s.program_name, s.status, \
CAST(st.text AS nvarchar(max)), CASE WHEN r.session_id IS NOT NULL THEN CAST(DATEDIFF_BIG(millisecond, r.start_time, GETDATE()) AS varchar(30)) END, \
r.wait_type, CASE WHEN r.blocking_session_id > 0 THEN CAST(r.blocking_session_id AS varchar(12)) END \
FROM sys.dm_exec_sessions s LEFT JOIN sys.dm_exec_requests r ON r.session_id = s.session_id \
LEFT JOIN sys.dm_exec_connections c ON c.session_id = s.session_id \
OUTER APPLY sys.dm_exec_sql_text(COALESCE(r.sql_handle, c.most_recent_sql_handle)) st \
WHERE s.is_user_process = 1 AND s.session_id <> @@SPID \
ORDER BY CASE s.status WHEN 'sleeping' THEN 1 ELSE 0 END, r.start_time";

const MS_CAN_SEE: &str =
    "SELECT CAST(HAS_PERMS_BY_NAME(NULL, NULL, 'VIEW SERVER STATE') AS varchar(1))";

const HANA_SEES_ALL: &str = "SELECT COUNT(*) FROM SYS.EFFECTIVE_PRIVILEGES WHERE USER_NAME = CURRENT_USER \
AND OBJECT_TYPE = 'SYSTEMPRIVILEGE' AND PRIVILEGE IN ('CATALOG READ', 'DATA ADMIN')";

pub(crate) const HANA_NEED: &str = "CATALOG READ";

pub(crate) async fn hana_sees_all(conn: &dyn Connection) -> bool {
    rows(conn, HANA_SEES_ALL)
        .await
        .ok()
        .and_then(|r| r.first().and_then(|r| cell(r, 0)))
        .as_deref()
        != Some("0")
}

const MS_LOCKS: &str = "SELECT CAST(r.session_id AS varchar(12)), CAST(r.blocking_session_id AS varchar(12)), r.wait_resource, r.wait_type, \
CAST(r.wait_time AS varchar(20)) FROM sys.dm_exec_requests r WHERE r.blocking_session_id > 0";

const CRDB_SESSIONS: &str = "SELECT session_id, user_name, NULL::STRING, client_address, application_name, lower(status), \
coalesce(nullif(active_queries, ''), last_active_query), \
CASE WHEN active_query_start IS NOT NULL THEN (extract(epoch FROM now() - active_query_start) * 1000)::FLOAT8::STRING END, NULL::STRING, NULL::STRING \
FROM [SHOW CLUSTER SESSIONS] WHERE session_id <> (SELECT * FROM [SHOW session_id]) \
ORDER BY status = 'IDLE', session_start";

const CRDB_LOCKS: &str = "SELECT DISTINCT wt.session_id, ht.session_id, concat_ws('.', w.database_name, w.schema_name, w.table_name), \
w.lock_strength, (extract(epoch FROM w.duration) * 1000)::FLOAT8::STRING \
FROM crdb_internal.cluster_locks w \
JOIN crdb_internal.cluster_locks h ON h.lock_key = w.lock_key AND h.granted AND NOT w.granted \
JOIN crdb_internal.cluster_transactions wt ON wt.id = w.txn_id \
JOIN crdb_internal.cluster_transactions ht ON ht.id = h.txn_id WHERE wt.session_id <> ht.session_id";

const CRDB_TABLES: &str = "SELECT t.schema_name, t.table_name, t.estimated_row_count::FLOAT8::STRING, NULL::STRING, NULL::STRING, s.bytes::STRING \
FROM [SHOW TABLES] t LEFT JOIN (SELECT schema_name, table_name, sum((span_stats->>'approximate_disk_bytes')::INT8) AS bytes \
FROM [SHOW RANGES FROM CURRENT_CATALOG WITH TABLES, DETAILS] GROUP BY 1, 2) s \
ON s.schema_name = t.schema_name AND s.table_name = t.table_name \
WHERE t.type = 'table' AND t.schema_name NOT IN ('crdb_internal', 'information_schema', 'pg_catalog', 'pg_extension') \
ORDER BY s.bytes DESC NULLS LAST LIMIT 1000";

const CRDB_INDEXES: &str = "SELECT t.schema_name, ti.descriptor_name, ti.index_name, NULL::STRING, coalesce(u.total_reads, 0)::STRING, ti.is_unique::STRING \
FROM crdb_internal.table_indexes ti \
JOIN crdb_internal.tables t ON t.table_id = ti.descriptor_id AND t.database_name = current_database() AND t.drop_time IS NULL \
LEFT JOIN crdb_internal.index_usage_statistics u ON u.table_id = ti.descriptor_id AND u.index_id = ti.index_id \
WHERE t.schema_name NOT IN ('crdb_internal', 'information_schema', 'pg_catalog', 'pg_extension') LIMIT 2000";

const MS_TABLES: &str = "SELECT s.name, t.name, CAST(ps.n AS varchar(30)), CAST(ps.data * 8192 AS varchar(30)), \
CAST(ps.idx * 8192 AS varchar(30)), CAST(ps.used * 8192 AS varchar(30)) \
FROM (SELECT object_id, SUM(CASE WHEN index_id IN (0, 1) THEN row_count END) AS n, \
SUM(CASE WHEN index_id IN (0, 1) THEN used_page_count END) AS data, \
SUM(CASE WHEN index_id > 1 THEN used_page_count ELSE 0 END) AS idx, SUM(used_page_count) AS used \
FROM sys.dm_db_partition_stats GROUP BY object_id) ps \
JOIN sys.objects t ON t.object_id = ps.object_id JOIN sys.schemas s ON s.schema_id = t.schema_id \
WHERE t.type = 'U' AND t.is_ms_shipped = 0 ORDER BY ps.used DESC";

const MS_INDEXES: &str = "SELECT s.name, t.name, i.name, CAST(ps.used * 8192 AS varchar(30)), NULL, \
CASE WHEN i.is_unique = 1 OR i.is_primary_key = 1 THEN 'true' ELSE 'false' END \
FROM (SELECT object_id, index_id, SUM(used_page_count) AS used FROM sys.dm_db_partition_stats GROUP BY object_id, index_id) ps \
JOIN sys.indexes i ON i.object_id = ps.object_id AND i.index_id = ps.index_id \
JOIN sys.objects t ON t.object_id = i.object_id JOIN sys.schemas s ON s.schema_id = t.schema_id \
WHERE i.name IS NOT NULL AND t.type = 'U' AND t.is_ms_shipped = 0 ORDER BY ps.used DESC";

const MS_INDEX_USAGE: &str = "SELECT s.name, t.name, i.name, CAST(MAX(us.user_seeks + us.user_scans + us.user_lookups) AS varchar(30)) \
FROM sys.indexes i JOIN sys.tables t ON t.object_id = i.object_id JOIN sys.schemas s ON s.schema_id = t.schema_id \
JOIN sys.dm_db_index_usage_stats us ON us.object_id = i.object_id AND us.index_id = i.index_id AND us.database_id = DB_ID() \
WHERE i.name IS NOT NULL GROUP BY s.name, t.name, i.name";

const DUCK_TABLES: &str = "SELECT schema_name, table_name, CAST(estimated_size AS VARCHAR), NULL, NULL, NULL \
FROM duckdb_tables() WHERE database_name = current_database() AND NOT internal ORDER BY estimated_size DESC";

const DUCK_INDEXES: &str = "SELECT schema_name, table_name, index_name, NULL, NULL, CAST(is_unique AS VARCHAR) \
FROM duckdb_indexes() WHERE database_name = current_database() ORDER BY schema_name, table_name, index_name";

const SQLITE_NAMES: &str = "SELECT type, name, tbl_name, NULL FROM sqlite_schema \
WHERE type IN ('table', 'index') AND name NOT LIKE 'sqlite_%'";

const SQLITE_SIZES: &str = "SELECT m.type, m.name, m.tbl_name, s.bytes FROM sqlite_schema m \
LEFT JOIN (SELECT name, SUM(pgsize) AS bytes FROM dbstat GROUP BY name) s ON s.name = m.name \
WHERE m.type IN ('table', 'index') AND m.name NOT LIKE 'sqlite_%'";

const ORA_SESSIONS: &str = "SELECT s.sid || ',' || s.serial#, s.username, s.schemaname, s.machine, \
NVL(s.module, s.program), LOWER(s.status), q.sql_text, \
CASE WHEN s.status = 'ACTIVE' THEN s.last_call_et * 1000 END, \
CASE WHEN s.wait_class <> 'Idle' THEN s.wait_class || ': ' || s.event END, \
(SELECT b.sid || ',' || b.serial# FROM v$session b WHERE b.sid = s.blocking_session) \
FROM v$session s LEFT JOIN v$sql q ON q.sql_id = s.sql_id AND q.child_number = s.sql_child_number \
WHERE s.type = 'USER' AND s.sid <> SYS_CONTEXT('USERENV', 'SID') \
ORDER BY CASE s.status WHEN 'ACTIVE' THEN 0 ELSE 1 END, s.last_call_et DESC";

const ORA_LOCKS: &str = "SELECT w.sid || ',' || w.serial#, b.sid || ',' || b.serial#, \
(SELECT o.owner || '.' || o.object_name FROM all_objects o WHERE o.object_id = w.row_wait_obj#), \
w.event, w.seconds_in_wait * 1000 \
FROM v$session w JOIN v$session b ON b.sid = w.blocking_session WHERE w.blocking_session IS NOT NULL";

const ORA_DBA_TABLES: &str = "SELECT t.owner, t.table_name, t.num_rows, \
(SELECT SUM(s.bytes) FROM dba_segments s WHERE s.owner = t.owner AND s.segment_name = t.table_name), \
(SELECT SUM(s.bytes) FROM dba_segments s JOIN dba_indexes i ON i.owner = s.owner AND i.index_name = s.segment_name \
  WHERE i.table_owner = t.owner AND i.table_name = t.table_name), \
(SELECT SUM(s.bytes) FROM dba_segments s WHERE s.owner = t.owner AND (s.segment_name = t.table_name \
  OR s.segment_name IN (SELECT i.index_name FROM dba_indexes i WHERE i.table_owner = t.owner AND i.table_name = t.table_name) \
  OR s.segment_name IN (SELECT l.segment_name FROM dba_lobs l WHERE l.owner = t.owner AND l.table_name = t.table_name))) total \
FROM dba_tables t JOIN dba_users u ON u.username = t.owner AND u.oracle_maintained = 'N' \
ORDER BY total DESC NULLS LAST FETCH FIRST 1000 ROWS ONLY";

const ORA_DBA_INDEXES: &str = "SELECT i.table_owner, i.table_name, i.index_name, \
(SELECT SUM(s.bytes) FROM dba_segments s WHERE s.owner = i.owner AND s.segment_name = i.index_name), NULL, \
CASE i.uniqueness WHEN 'UNIQUE' THEN 'true' ELSE 'false' END \
FROM dba_indexes i JOIN dba_users u ON u.username = i.table_owner AND u.oracle_maintained = 'N' \
WHERE i.index_type <> 'LOB' FETCH FIRST 2000 ROWS ONLY";

const ORA_USER_TABLES: &str = "SELECT USER, t.table_name, t.num_rows, \
(SELECT SUM(s.bytes) FROM user_segments s WHERE s.segment_name = t.table_name), \
(SELECT SUM(s.bytes) FROM user_segments s JOIN user_indexes i ON i.index_name = s.segment_name WHERE i.table_name = t.table_name), \
(SELECT SUM(s.bytes) FROM user_segments s WHERE s.segment_name = t.table_name \
  OR s.segment_name IN (SELECT i.index_name FROM user_indexes i WHERE i.table_name = t.table_name) \
  OR s.segment_name IN (SELECT l.segment_name FROM user_lobs l WHERE l.table_name = t.table_name)) total \
FROM user_tables t ORDER BY total DESC NULLS LAST FETCH FIRST 1000 ROWS ONLY";

const ORA_USER_INDEXES: &str = "SELECT USER, i.table_name, i.index_name, \
(SELECT SUM(s.bytes) FROM user_segments s WHERE s.segment_name = i.index_name), NULL, \
CASE i.uniqueness WHEN 'UNIQUE' THEN 'true' ELSE 'false' END \
FROM user_indexes i WHERE i.index_type <> 'LOB'";

const FB_SESSIONS: &str = "SELECT a.MON$ATTACHMENT_ID, TRIM(a.MON$USER), a.MON$ATTACHMENT_NAME, \
a.MON$REMOTE_ADDRESS, a.MON$REMOTE_PROCESS, CASE WHEN s.MON$STATE = 1 THEN 'active' ELSE 'idle' END, \
s.MON$SQL_TEXT, CASE WHEN s.MON$STATE = 1 THEN DATEDIFF(MILLISECOND FROM s.MON$TIMESTAMP TO CURRENT_TIMESTAMP) END, \
CAST(NULL AS VARCHAR(8)), CAST(NULL AS VARCHAR(8)) \
FROM MON$ATTACHMENTS a LEFT JOIN MON$STATEMENTS s ON s.MON$ATTACHMENT_ID = a.MON$ATTACHMENT_ID AND s.MON$STATE = 1 \
WHERE a.MON$ATTACHMENT_ID <> CURRENT_CONNECTION AND COALESCE(a.MON$SYSTEM_FLAG, 0) = 0 \
ORDER BY s.MON$STATE NULLS LAST, s.MON$TIMESTAMP";

const FB_TABLES: &str = "SELECT 'PUBLIC', TRIM(r.RDB$RELATION_NAME), \
(SELECT MAX(CAST(1 / i.RDB$STATISTICS AS BIGINT)) FROM RDB$INDICES i \
  WHERE i.RDB$RELATION_NAME = r.RDB$RELATION_NAME AND i.RDB$UNIQUE_FLAG = 1 AND i.RDB$STATISTICS > 0), \
CAST(NULL AS BIGINT), CAST(NULL AS BIGINT), CAST(NULL AS BIGINT) \
FROM RDB$RELATIONS r WHERE COALESCE(r.RDB$SYSTEM_FLAG, 0) = 0 AND r.RDB$VIEW_BLR IS NULL ORDER BY 2";

const FB_INDEXES: &str = "SELECT 'PUBLIC', TRIM(i.RDB$RELATION_NAME), TRIM(i.RDB$INDEX_NAME), CAST(NULL AS BIGINT), \
CAST(NULL AS BIGINT), CASE WHEN i.RDB$UNIQUE_FLAG = 1 THEN 'true' ELSE 'false' END \
FROM RDB$INDICES i WHERE COALESCE(i.RDB$SYSTEM_FLAG, 0) = 0 ORDER BY 2, 3";

const DB2_SESSIONS: &str = "SELECT CAST(c.APPLICATION_HANDLE AS VARCHAR(20)), RTRIM(c.SESSION_AUTH_ID), CURRENT SERVER, \
COALESCE(c.CLIENT_HOSTNAME, c.CLIENT_IPADDR), c.APPLICATION_NAME, \
CASE WHEN a.APPLICATION_HANDLE IS NULL THEN 'idle' ELSE 'active' END, a.STMT_TEXT, \
TIMESTAMPDIFF(2, CHAR(CURRENT TIMESTAMP - a.LOCAL_START_TIME)) * 1000, CAST(NULL AS VARCHAR(8)), CAST(NULL AS VARCHAR(8)) \
FROM TABLE(MON_GET_CONNECTION(NULL, -2)) c \
LEFT JOIN TABLE(MON_GET_ACTIVITY(NULL, -2)) a ON a.APPLICATION_HANDLE = c.APPLICATION_HANDLE \
WHERE c.APPLICATION_HANDLE <> MON_GET_APPLICATION_HANDLE()";

const DB2_LOCKS: &str = "SELECT CAST(REQ_APPLICATION_HANDLE AS VARCHAR(20)), CAST(HLD_APPLICATION_HANDLE AS VARCHAR(20)), \
RTRIM(TABSCHEMA) || '.' || RTRIM(TABNAME), LOCK_MODE_REQUESTED, LOCK_WAIT_ELAPSED_TIME * 1000 FROM SYSIBMADM.MON_LOCKWAITS";

const DB2_TABLES: &str = "SELECT RTRIM(a.TABSCHEMA), RTRIM(a.TABNAME), t.CARD, a.DATA_OBJECT_P_SIZE * 1024, \
a.INDEX_OBJECT_P_SIZE * 1024, (a.DATA_OBJECT_P_SIZE + a.INDEX_OBJECT_P_SIZE + a.LONG_OBJECT_P_SIZE + a.LOB_OBJECT_P_SIZE \
  + a.XML_OBJECT_P_SIZE) * 1024 AS total \
FROM SYSIBMADM.ADMINTABINFO a JOIN SYSCAT.TABLES t ON t.TABSCHEMA = a.TABSCHEMA AND t.TABNAME = a.TABNAME \
WHERE a.TABSCHEMA NOT LIKE 'SYS%' ORDER BY total DESC FETCH FIRST 1000 ROWS ONLY";

const DB2_INDEXES: &str = "SELECT RTRIM(i.TABSCHEMA), RTRIM(i.TABNAME), RTRIM(i.INDNAME), CAST(NULL AS BIGINT), \
COALESCE(m.INDEX_SCANS, 0) + COALESCE(m.INDEX_ONLY_SCANS, 0), \
CASE WHEN i.UNIQUERULE IN ('U', 'P') THEN 'true' ELSE 'false' END \
FROM SYSCAT.INDEXES i LEFT JOIN TABLE(MON_GET_INDEX(NULL, NULL, -2)) m \
  ON m.TABSCHEMA = i.TABSCHEMA AND m.TABNAME = i.TABNAME AND m.IID = i.IID \
WHERE i.TABSCHEMA NOT LIKE 'SYS%' FETCH FIRST 2000 ROWS ONLY";

const HANA_SESSIONS: &str = "SELECT TO_VARCHAR(c.CONNECTION_ID), c.USER_NAME, c.CURRENT_SCHEMA_NAME, c.CLIENT_HOST, \
CAST(NULL AS NVARCHAR(8)), LOWER(c.CONNECTION_STATUS), s.STATEMENT_STRING, \
CASE WHEN c.CONNECTION_STATUS = 'RUNNING' THEN SECONDS_BETWEEN(s.LAST_EXECUTED_TIME, CURRENT_TIMESTAMP) * 1000 END, \
CAST(NULL AS NVARCHAR(8)), CAST(NULL AS NVARCHAR(8)) \
FROM M_CONNECTIONS c LEFT JOIN M_ACTIVE_STATEMENTS s ON s.CONNECTION_ID = c.CONNECTION_ID AND s.STATEMENT_STATUS = 'ACTIVE' \
WHERE c.CONNECTION_ID > 0 AND c.OWN = 'FALSE' ORDER BY c.CONNECTION_STATUS DESC";

const HANA_LOCKS: &str = "SELECT TO_VARCHAR(BLOCKED_CONNECTION_ID), TO_VARCHAR(LOCK_OWNER_CONNECTION_ID), \
WAITING_SCHEMA_NAME || '.' || WAITING_OBJECT_NAME, LOCK_MODE, SECONDS_BETWEEN(BLOCKED_TIME, CURRENT_TIMESTAMP) * 1000 \
FROM M_BLOCKED_TRANSACTIONS";

const HANA_TABLES: &str = "SELECT SCHEMA_NAME, TABLE_NAME, RECORD_COUNT, TABLE_SIZE, CAST(NULL AS BIGINT), TABLE_SIZE \
FROM M_TABLES WHERE SCHEMA_NAME IN (SELECT SCHEMA_NAME FROM SYS.SCHEMAS WHERE SCHEMA_OWNER <> 'SYS' \
AND SCHEMA_OWNER NOT LIKE '\\_SYS%' ESCAPE '\\') ORDER BY 4 DESC LIMIT 1000";

const HANA_INDEXES: &str = "SELECT SCHEMA_NAME, TABLE_NAME, INDEX_NAME, CAST(NULL AS BIGINT), CAST(NULL AS BIGINT), \
CASE WHEN CONSTRAINT LIKE '%UNIQUE%' OR CONSTRAINT = 'PRIMARY KEY' THEN 'true' ELSE 'false' END \
FROM SYS.INDEXES WHERE SCHEMA_NAME IN (SELECT SCHEMA_NAME FROM SYS.SCHEMAS WHERE SCHEMA_OWNER <> 'SYS' \
AND SCHEMA_OWNER NOT LIKE '\\_SYS%' ESCAPE '\\') LIMIT 2000";

pub(crate) async fn unlock_internals(conn: &dyn Connection) {
    let _ = rows(conn, "SET allow_unsafe_internals = true").await;
}

fn table_size(r: &Row) -> TableSize {
    TableSize {
        namespace: cell(r, 0).unwrap_or_default(),
        name: cell(r, 1).unwrap_or_default(),
        rows: num(r, 2),
        data_bytes: num(r, 3),
        index_bytes: num(r, 4),
        total_bytes: num(r, 5),
    }
}

fn index_usage(r: &Row) -> IndexUsage {
    let scans = num(r, 4);
    let unique = cell(r, 5).as_deref() == Some("true");
    IndexUsage {
        namespace: cell(r, 0).unwrap_or_default(),
        table: cell(r, 1).unwrap_or_default(),
        name: cell(r, 2).unwrap_or_default(),
        bytes: num(r, 3),
        scans,
        unique,
        unused: scans == Some(0.0) && !unique,
    }
}

fn session_of(r: &Row) -> ServerSession {
    ServerSession {
        id: cell(r, 0).unwrap_or_default(),
        user: cell(r, 1),
        database: cell(r, 2),
        client: cell(r, 3),
        application: cell(r, 4),
        state: cell(r, 5),
        query: cell(r, 6),
        duration_ms: num(r, 7),
        wait: cell(r, 8),
        blocked_by: cell(r, 9)
            .map(|s| s.split(',').map(str::to_owned).collect())
            .unwrap_or_default(),
    }
}

fn lock_of(r: &Row) -> LockWait {
    LockWait {
        waiting: cell(r, 0).unwrap_or_default(),
        blocking: cell(r, 1).unwrap_or_default(),
        object: cell(r, 2),
        mode: cell(r, 3),
        waited_ms: num(r, 4),
    }
}

pub(crate) fn kill_sql(engine: Engine, id: &str, mode: KillMode) -> Result<String> {
    if engine == Engine::Clickhouse {
        if id.is_empty() || id.len() > 128 {
            return Err(ToolFault::Empty.into());
        }
        return Ok(super::clickhouse::kill_sql(id));
    }
    if engine == Engine::Mongo {
        return super::mongo::kill_command(id);
    }
    if matches!(engine, Engine::Hana | Engine::Db2) {
        if id.is_empty() || id.len() > 20 || !id.bytes().all(|b| b.is_ascii_digit()) {
            return Err(ToolFault::Empty.into());
        }
        return match (engine, mode) {
            (Engine::Hana, KillMode::Cancel) => Ok(format!("ALTER SYSTEM CANCEL SESSION '{id}'")),
            (Engine::Hana, KillMode::Terminate) => {
                Ok(format!("ALTER SYSTEM DISCONNECT SESSION '{id}'"))
            }
            (_, KillMode::Terminate) => Ok(format!(
                "CALL SYSPROC.ADMIN_CMD('FORCE APPLICATION ({id})')"
            )),
            (_, KillMode::Cancel) => Err(ToolFault::Unsupported.into()),
        };
    }
    if engine == Engine::Firebird {
        if id.is_empty() || id.len() > 20 || !id.bytes().all(|b| b.is_ascii_digit()) {
            return Err(ToolFault::Empty.into());
        }
        return Ok(match mode {
            KillMode::Cancel => {
                format!(
                    "DELETE FROM MON$STATEMENTS WHERE MON$ATTACHMENT_ID = {id} AND MON$STATE = 1"
                )
            }
            KillMode::Terminate => {
                format!("DELETE FROM MON$ATTACHMENTS WHERE MON$ATTACHMENT_ID = {id}")
            }
        });
    }
    if engine == Engine::Oracle {
        let parts: Vec<&str> = id.split(',').map(str::trim).collect();
        let valid = parts.len() == 2
            && parts
                .iter()
                .all(|p| !p.is_empty() && p.len() <= 12 && p.bytes().all(|b| b.is_ascii_digit()));
        if !valid {
            return Err(ToolFault::Empty.into());
        }
        let (sid, serial) = (parts[0], parts[1]);
        return Ok(match mode {
            KillMode::Cancel => format!("ALTER SYSTEM CANCEL SQL '{sid}, {serial}'"),
            KillMode::Terminate => format!("ALTER SYSTEM KILL SESSION '{sid},{serial}' IMMEDIATE"),
        });
    }
    let hex = engine == Engine::Cockroach;
    let ok = |b: u8| {
        if hex {
            b.is_ascii_hexdigit()
        } else {
            b.is_ascii_digit()
        }
    };
    if id.is_empty() || !id.bytes().all(ok) || id.len() > if hex { 64 } else { 20 } {
        return Err(ToolFault::Empty.into());
    }
    Ok(match (engine, mode) {
        (Engine::Postgres, KillMode::Cancel) => format!("SELECT pg_cancel_backend({id})"),
        (Engine::Postgres, KillMode::Terminate) => format!("SELECT pg_terminate_backend({id})"),
        (Engine::Mysql | Engine::Mariadb, KillMode::Cancel) => format!("KILL QUERY {id}"),
        (Engine::Mysql | Engine::Mariadb, KillMode::Terminate) => format!("KILL CONNECTION {id}"),
        (Engine::Mssql, KillMode::Terminate) => format!("KILL {id}"),
        (Engine::Cockroach, KillMode::Cancel) => format!(
            "CANCEL QUERIES SELECT query_id FROM [SHOW CLUSTER STATEMENTS] WHERE session_id = '{id}'"
        ),
        (Engine::Cockroach, KillMode::Terminate) => format!("CANCEL SESSION '{id}'"),
        (Engine::Mssql, KillMode::Cancel)
        | (
            Engine::Sqlite
            | Engine::Libsql
            | Engine::Duckdb
            | Engine::Clickhouse
            | Engine::Mongo
            | Engine::Oracle
            | Engine::Firebird
            | Engine::Hana
            | Engine::Db2
            | Engine::Odbc,
            _,
        ) => {
            return Err(ToolFault::Unsupported.into());
        }
    })
}

fn sqlite_sizes(rows: &[Row]) -> Sizes {
    let mut tables: Vec<TableSize> = rows
        .iter()
        .filter(|r| cell(r, 0).as_deref() == Some("table"))
        .map(|r| TableSize {
            namespace: "main".into(),
            name: cell(r, 1).unwrap_or_default(),
            rows: None,
            data_bytes: num(r, 3),
            index_bytes: None,
            total_bytes: None,
        })
        .collect();
    let indexes: Vec<IndexUsage> = rows
        .iter()
        .filter(|r| cell(r, 0).as_deref() == Some("index"))
        .map(|r| IndexUsage {
            namespace: "main".into(),
            table: cell(r, 2).unwrap_or_default(),
            name: cell(r, 1).unwrap_or_default(),
            bytes: num(r, 3),
            scans: None,
            unique: false,
            unused: false,
        })
        .collect();
    for t in &mut tables {
        let idx: Option<f64> = indexes
            .iter()
            .filter(|i| i.table == t.name)
            .map(|i| i.bytes)
            .sum();
        t.index_bytes = idx;
        t.total_bytes = t.data_bytes.map(|d| d + idx.unwrap_or(0.0));
    }
    tables.sort_by(|a, b| {
        b.total_bytes
            .partial_cmp(&a.total_bytes)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    Sizes {
        tables,
        indexes,
        usage_known: false,
        partitions: Vec::new(),
        hypertables: Vec::new(),
    }
}

async fn oracle_sizes(conn: &dyn Connection) -> std::result::Result<Sizes, nd_driver::DriverError> {
    let (tables, indexes) = match rows(conn, ORA_DBA_TABLES).await {
        Ok(t) => (t, rows(conn, ORA_DBA_INDEXES).await?),
        Err(_) => (
            rows(conn, ORA_USER_TABLES).await?,
            rows(conn, ORA_USER_INDEXES).await?,
        ),
    };
    Ok(Sizes {
        tables: tables.iter().map(table_size).collect(),
        indexes: indexes.iter().map(index_usage).collect(),
        usage_known: false,
        partitions: Vec::new(),
        hypertables: Vec::new(),
    })
}

async fn vendor_sizes(
    conn: &dyn Connection,
    engine: Engine,
) -> std::result::Result<Sizes, nd_driver::DriverError> {
    let (tables, indexes) = match engine {
        Engine::Hana => (HANA_TABLES, HANA_INDEXES),
        Engine::Db2 => (DB2_TABLES, DB2_INDEXES),
        _ => (FB_TABLES, FB_INDEXES),
    };
    let tables = rows(conn, tables).await?;
    let indexes = rows(conn, indexes).await.unwrap_or_default();
    Ok(Sizes {
        tables: tables.iter().map(table_size).collect(),
        indexes: indexes.iter().map(index_usage).collect(),
        usage_known: engine == Engine::Db2,
        partitions: Vec::new(),
        hypertables: Vec::new(),
    })
}

async fn mssql_usage(conn: &dyn Connection, indexes: &mut [IndexUsage]) -> bool {
    let Ok(used) = rows(conn, MS_INDEX_USAGE).await else {
        return false;
    };
    for r in used {
        let key = (cell(&r, 0), cell(&r, 1), cell(&r, 2));
        if let Some(i) = indexes.iter_mut().find(|i| {
            key == (
                Some(i.namespace.clone()),
                Some(i.table.clone()),
                Some(i.name.clone()),
            )
        }) {
            i.scans = num(&r, 3);
        }
    }
    for i in indexes {
        i.scans.get_or_insert(0.0);
        i.unused = i.scans == Some(0.0) && !i.unique;
    }
    true
}

impl Core {
    pub async fn activity(&self, session_id: &str) -> Result<Activity> {
        let session = self.live(session_id).await?;
        self.authorize(&session, Access::Read)?;
        let side = self.side(&session).await?;
        let conn = side.conn()?;
        let engine = Engine::of(conn);
        let (sessions, locks) = match engine {
            Engine::Postgres => (PG_SESSIONS, PG_LOCKS),
            Engine::Cockroach => (CRDB_SESSIONS, CRDB_LOCKS),
            Engine::Mariadb => (MY_SESSIONS, MARIADB_LOCKS),
            Engine::Mysql => (MY_SESSIONS, MYSQL_LOCKS),
            Engine::Mssql => (MS_SESSIONS, MS_LOCKS),
            Engine::Oracle => (ORA_SESSIONS, ORA_LOCKS),
            Engine::Hana => (HANA_SESSIONS, HANA_LOCKS),
            Engine::Db2 => (DB2_SESSIONS, DB2_LOCKS),
            Engine::Odbc => return Err(ToolFault::Unsupported.into()),
            Engine::Firebird => {
                let found = rows(conn, FB_SESSIONS).await;
                let sessions = side.check(found)?.iter().map(session_of).collect();
                return Ok(Activity {
                    sessions,
                    locks: Vec::new(),
                    partial: false,
                    can_cancel: true,
                });
            }
            Engine::Clickhouse => {
                let found = rows(conn, &super::clickhouse::sessions_sql()).await;
                let sessions = side.check(found)?.iter().map(session_of).collect();
                return Ok(Activity {
                    sessions,
                    locks: Vec::new(),
                    partial: false,
                    can_cancel: true,
                });
            }
            Engine::Mongo => {
                let found = super::mongo::activity(conn).await;
                return side.check(found);
            }
            Engine::Sqlite | Engine::Libsql | Engine::Duckdb => {
                return Err(ToolFault::Unsupported.into());
            }
        };
        if engine == Engine::Cockroach {
            unlock_internals(conn).await;
        }
        let found = rows(conn, sessions).await;
        let waits = rows(conn, locks).await;
        let mut sessions: Vec<ServerSession> = side.check(found)?.iter().map(session_of).collect();
        let blind = engine == Engine::Mssql
            && cell(
                rows(conn, MS_CAN_SEE)
                    .await
                    .unwrap_or_default()
                    .first()
                    .unwrap_or(&Vec::new()),
                0,
            )
            .as_deref()
                == Some("0");
        let partial =
            waits.is_err() || blind || (engine == Engine::Hana && !hana_sees_all(conn).await);
        let mut locks: Vec<LockWait> = waits
            .map(|r| r.iter().map(lock_of).collect())
            .unwrap_or_default();
        if matches!(engine, Engine::Mysql | Engine::Mariadb)
            && let Ok(found) = rows(conn, MY_METADATA_LOCKS).await
        {
            locks.extend(found.iter().map(lock_of));
        }
        for s in &mut sessions {
            for l in locks.iter().filter(|l| l.waiting == s.id) {
                if !s.blocked_by.contains(&l.blocking) {
                    s.blocked_by.push(l.blocking.clone());
                }
            }
        }
        Ok(Activity {
            sessions,
            locks,
            partial,
            can_cancel: !matches!(engine, Engine::Mssql | Engine::Db2),
        })
    }

    pub async fn sizes(&self, session_id: &str) -> Result<Sizes> {
        let session = self.live(session_id).await?;
        self.authorize(&session, Access::Read)?;
        let side = self.side(&session).await?;
        let conn = side.conn()?;
        if Engine::of(conn) == Engine::Cockroach {
            unlock_internals(conn).await;
        }
        match Engine::of(conn) {
            engine @ (Engine::Postgres | Engine::Cockroach | Engine::Mssql | Engine::Duckdb) => {
                let (tables, indexes) = match engine {
                    Engine::Postgres => (PG_TABLES, PG_INDEXES),
                    Engine::Cockroach => (CRDB_TABLES, CRDB_INDEXES),
                    Engine::Mssql => (MS_TABLES, MS_INDEXES),
                    _ => (DUCK_TABLES, DUCK_INDEXES),
                };
                let tables = rows(conn, tables).await;
                let indexes = rows(conn, indexes).await;
                let tables = side.check(tables)?;
                let indexes = side.check(indexes)?;
                let mut usage_known = engine != Engine::Duckdb;
                let mut indexes: Vec<IndexUsage> = indexes.iter().map(index_usage).collect();
                if engine == Engine::Mssql {
                    usage_known = mssql_usage(conn, &mut indexes).await;
                }
                let mut sizes = Sizes {
                    tables: tables.iter().map(table_size).collect(),
                    indexes,
                    usage_known,
                    partitions: Vec::new(),
                    hypertables: Vec::new(),
                };
                super::timescale::sizes(conn, &mut sizes).await;
                Ok(sizes)
            }
            Engine::Mysql | Engine::Mariadb => {
                let tables = rows(conn, MY_TABLES).await;
                let tables = side.check(tables)?;
                let list = rows(conn, MY_INDEXES).await;
                let list = side.check(list)?;
                let bytes = rows(conn, MY_INDEX_BYTES).await.unwrap_or_default();
                let scans = if Engine::of(conn) == Engine::Mariadb {
                    None
                } else {
                    rows(conn, MYSQL_INDEX_SCANS).await.ok()
                };
                let find = |set: &[Row], r: &Row| {
                    set.iter()
                        .find(|x| cell(x, 1) == cell(r, 1) && cell(x, 2) == cell(r, 2))
                        .and_then(|x| num(x, 3))
                };
                let usage_known = scans.as_ref().is_some_and(|s| !s.is_empty());
                Ok(Sizes {
                    tables: tables.iter().map(my_table).collect(),
                    indexes: list
                        .iter()
                        .map(|r| {
                            let unique = cell(r, 3).as_deref() == Some("0");
                            let used = scans
                                .as_deref()
                                .filter(|_| usage_known)
                                .map(|s| find(s, r).unwrap_or(0.0));
                            IndexUsage {
                                namespace: cell(r, 0).unwrap_or_default(),
                                table: cell(r, 1).unwrap_or_default(),
                                name: cell(r, 2).unwrap_or_default(),
                                bytes: find(&bytes, r),
                                scans: used,
                                unique,
                                unused: used == Some(0.0) && !unique,
                            }
                        })
                        .collect(),
                    usage_known,
                    partitions: Vec::new(),
                    hypertables: Vec::new(),
                })
            }
            Engine::Clickhouse => side.check(super::clickhouse::sizes(conn).await),
            Engine::Oracle => side.check(oracle_sizes(conn).await),
            engine @ (Engine::Hana | Engine::Db2 | Engine::Firebird) => {
                side.check(vendor_sizes(conn, engine).await)
            }
            Engine::Odbc => Err(ToolFault::Unsupported.into()),
            Engine::Mongo => {
                side.check(super::mongo::sizes(conn, &session.conn.server().database).await)
            }
            Engine::Sqlite | Engine::Libsql => {
                let found = if let Ok(found) = rows(conn, SQLITE_SIZES).await {
                    found
                } else {
                    let names = rows(conn, SQLITE_NAMES).await;
                    side.check(names)?
                };
                Ok(sqlite_sizes(&found))
            }
        }
    }

    pub async fn kill_session(
        &self,
        session_id: &str,
        target: &str,
        mode: KillMode,
        confirm: Option<&str>,
    ) -> Result<bool> {
        let session = self.live(session_id).await?;
        let sql = kill_sql(Engine::of(session.conn.as_ref()), target, mode)?;
        self.authorize(&session, Access::Write { confirm })?;
        let side = self.side(&session).await?;
        let conn = side.conn()?;
        let done = collect(conn, &sql, None).await;
        let error = done.as_ref().err().map(ToString::to_string);
        self.audit(&session, &sql, None, error);
        let (rows, _) = side.check(done)?;
        Ok(match Engine::of(conn) {
            Engine::Postgres => {
                cell(rows.first().unwrap_or(&Vec::new()), 0).as_deref() == Some("t")
            }
            _ => true,
        })
    }
}
