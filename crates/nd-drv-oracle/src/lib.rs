mod schema;
mod statement;
mod value;

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::net::IpAddr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use nd_driver::{
    Canceller, Chunk, Column, Connection, DdlKind, Dependency, Dialect, DriverError, ObjectRef,
    OracleRole, OracleTarget, Outcome, Result, RowBatcher, Schema, ServerInfo, TlsMode, tidy,
};
use oracledb::{ErrorKind, ToDbValue};
use tokio::sync::{mpsc, oneshot};

pub use statement::{Kind, prepare};

type Ora = oracledb::Connection;

enum Job {
    Execute {
        sql: String,
        params: Vec<Option<String>>,
        out: mpsc::Sender<Chunk>,
        reply: oneshot::Sender<Result<Outcome>>,
    },
    Schema {
        reply: oneshot::Sender<Result<Schema>>,
    },
    Ddl {
        object: ObjectRef,
        reply: oneshot::Sender<Result<String>>,
    },
    Dependencies {
        object: ObjectRef,
        reply: oneshot::Sender<Result<Vec<Dependency>>>,
    },
}

#[derive(Clone)]
struct Logon {
    connect: String,
    user: String,
    password: String,
    role: OracleRole,
    ca_pem: Option<String>,
}

impl Logon {
    fn open(&self) -> std::result::Result<Ora, oracledb::Error> {
        let wallet = match &self.ca_pem {
            Some(pem) => Some(wallet_dir(pem)?),
            None => None,
        };
        let mut config = oracledb::Config::default()
            .set_credentials(&self.user, &self.password)
            .set_connect_string(&self.connect)?
            .set_program("NiceDB")?
            .set_transport_connect_timeout(Some(Duration::from_secs(15)))
            .set_auth_mode(match self.role {
                OracleRole::Normal => oracledb::AUTH_MODE_DEFAULT,
                OracleRole::Sysdba => oracledb::AUTH_MODE_SYSDBA,
                OracleRole::Sysoper => oracledb::AUTH_MODE_SYSOPER,
            });
        if let Some(w) = &wallet {
            config = config.set_wallet_location(w.path().to_string_lossy().into_owned());
        }
        oracledb::connect(config)
    }
}

struct Stopper {
    stop: AtomicBool,
    busy: AtomicBool,
    in_tx: AtomicBool,
    killed: AtomicBool,
    read_only: AtomicBool,
    privileged: bool,
    session: (usize, usize),
    logon: Logon,
    helper: Mutex<Option<Ora>>,
}

const GRACE: Duration = Duration::from_millis(1500);

impl Stopper {
    fn end_on_server(&self) {
        let waited = std::time::Instant::now();
        while self.busy.load(Ordering::Acquire) && waited.elapsed() < GRACE {
            thread::sleep(Duration::from_millis(50));
        }
        if !self.busy.load(Ordering::Acquire) || self.in_tx.load(Ordering::Acquire) {
            return;
        }
        let Ok(mut helper) = self.helper.lock() else {
            return;
        };
        if helper.is_none() {
            match self.logon.open() {
                Ok(c) => {
                    let _ = c.set_call_timeout(Some(Duration::from_secs(5)));
                    *helper = Some(c);
                }
                Err(e) => {
                    tracing::warn!(error = %e, "oracle stop session failed");
                    return;
                }
            }
        }
        let sql = format!(
            "ALTER SYSTEM DISCONNECT SESSION '{}, {}' IMMEDIATE",
            self.session.0, self.session.1
        );
        self.killed.store(true, Ordering::Release);
        if let Some(c) = helper.as_ref()
            && let Err(e) = c.execute(&sql, &[])
        {
            let marked = matches!(e.kind(), ErrorKind::DbError(d) if d.code() == 31);
            if !marked {
                tracing::warn!(error = %e, "oracle stop failed");
                self.killed.store(false, Ordering::Release);
                *helper = None;
            }
        }
    }
}

struct OracleCanceller(Arc<Stopper>);

#[async_trait::async_trait]
impl Canceller for OracleCanceller {
    async fn cancel(&self) {
        let s = &self.0;
        s.stop.store(true, Ordering::Release);
        if s.privileged && s.busy.load(Ordering::Acquire) {
            let s = s.clone();
            let _ = tokio::task::spawn_blocking(move || s.end_on_server()).await;
        }
    }
}

pub struct OracleConnection {
    jobs: std::sync::mpsc::Sender<Job>,
    server: ServerInfo,
    stopper: Arc<Stopper>,
}

pub fn descriptor(t: &OracleTarget, host: &str, port: u16, wallet: Option<&str>) -> String {
    if !t.connect_string.trim().is_empty() {
        return t.connect_string.trim().to_owned();
    }
    let protocol = if t.tls == TlsMode::Disable {
        "TCP"
    } else {
        "TCPS"
    };
    let data = if t.sid { "SID" } else { "SERVICE_NAME" };
    let mut security = String::new();
    if t.tls == TlsMode::Insecure {
        security.push_str("(SSL_SERVER_DN_MATCH=OFF)");
    }
    if let Some(w) = wallet {
        let _ = write!(security, "(WALLET_LOCATION={w})");
    }
    let security = if security.is_empty() {
        String::new()
    } else {
        format!("(SECURITY={security})")
    };
    format!(
        "(DESCRIPTION=(ADDRESS=(PROTOCOL={protocol})(HOST={host})(PORT={port}))(CONNECT_DATA=({data}={})){security})",
        t.database.trim()
    )
}

fn read_ca(ca_file: &str) -> Result<String> {
    std::fs::read_to_string(ca_file).map_err(|e| DriverError::Tls {
        message: format!("{ca_file}: {e}"),
    })
}

fn wallet_dir(pem: &str) -> std::io::Result<nd_driver::PrivateDir> {
    let dir = nd_driver::PrivateDir::new()?;
    dir.write("ewallet.pem", &[pem.as_bytes()])?;
    Ok(dir)
}

pub(crate) fn map_err(e: &oracledb::Error) -> DriverError {
    match e.kind() {
        ErrorKind::DbError(d) if d.code() == 1013 => DriverError::Cancelled,
        ErrorKind::DbError(d) if matches!(d.code(), 1017 | 1005 | 28000 | 28001) => {
            DriverError::Auth {
                message: d.message().to_owned(),
            }
        }
        ErrorKind::DbError(d) if matches!(d.code(), 28 | 1012 | 3113 | 3114 | 3135) => {
            DriverError::Io {
                message: d.message().to_owned(),
            }
        }
        ErrorKind::DbError(d) => DriverError::Query {
            message: d.message().to_owned(),
            code: Some(format!("ORA-{:05}", d.code())),
            detail: None,
            hint: None,
            offset: u32::try_from(d.offset()).ok().filter(|o| *o > 0),
        },
        ErrorKind::CallTimeoutExceeded => DriverError::Cancelled,
        ErrorKind::TlsOperation => DriverError::Tls {
            message: e.to_string(),
        },
        ErrorKind::DeadConnection | ErrorKind::NotConnected | ErrorKind::StreamOperation => {
            DriverError::Io {
                message: e.to_string(),
            }
        }
        _ => DriverError::query(e.to_string()),
    }
}

fn connect_err(e: &oracledb::Error) -> DriverError {
    match map_err(e) {
        a @ (DriverError::Auth { .. } | DriverError::Tls { .. }) => a,
        other => DriverError::Connect {
            message: other.to_string(),
        },
    }
}

fn worker_gone() -> DriverError {
    DriverError::Io {
        message: "oracle worker exited".into(),
    }
}

const SESSION: &str = "ALTER SESSION SET NLS_DATE_FORMAT = 'YYYY-MM-DD HH24:MI:SS' \
     NLS_TIMESTAMP_FORMAT = 'YYYY-MM-DD HH24:MI:SS.FF' \
     NLS_TIMESTAMP_TZ_FORMAT = 'YYYY-MM-DD HH24:MI:SS.FF TZH:TZM' \
     NLS_NUMERIC_CHARACTERS = '.,'";

const ABOUT: &str = "SELECT SYS_CONTEXT('USERENV', 'CON_NAME'), \
     (SELECT COUNT(*) FROM session_privs WHERE privilege = 'ALTER SYSTEM') FROM dual";

const METADATA: &str = "BEGIN
  DBMS_METADATA.SET_TRANSFORM_PARAM(DBMS_METADATA.SESSION_TRANSFORM, 'SQLTERMINATOR', TRUE);
  DBMS_METADATA.SET_TRANSFORM_PARAM(DBMS_METADATA.SESSION_TRANSFORM, 'PRETTY', TRUE);
  DBMS_METADATA.SET_TRANSFORM_PARAM(DBMS_METADATA.SESSION_TRANSFORM, 'SEGMENT_ATTRIBUTES', FALSE);
  DBMS_METADATA.SET_TRANSFORM_PARAM(DBMS_METADATA.SESSION_TRANSFORM, 'STORAGE', FALSE);
END;";

struct Ready {
    version: String,
    database: String,
    privileged: bool,
    session: (usize, usize),
}

pub async fn connect(
    target: &OracleTarget,
    password: Option<&str>,
    via: Option<(IpAddr, u16)>,
) -> Result<OracleConnection> {
    let (host, port) = via.map_or_else(
        || (target.host.clone(), target.port),
        |(ip, p)| (ip.to_string(), p),
    );
    let ca_pem = match target.ca_file.as_deref().filter(|f| !f.trim().is_empty()) {
        Some(f) if target.tls != TlsMode::Disable => Some(read_ca(f)?),
        _ => None,
    };
    let logon = Logon {
        connect: descriptor(target, &host, port, None),
        user: target.user.clone(),
        password: password.unwrap_or_default().to_owned(),
        role: target.role,
        ca_pem,
    };
    let (ready_tx, ready_rx) = oneshot::channel::<Result<(Ready, Arc<Stopper>)>>();
    let (jobs, rx) = std::sync::mpsc::channel::<Job>();
    let first = logon.clone();
    thread::Builder::new()
        .name("nd-oracle".into())
        .spawn(move || {
            let conn = match first.open() {
                Ok(c) => c,
                Err(e) => {
                    let _ = ready_tx.send(Err(connect_err(&e)));
                    return;
                }
            };
            let ready = match setup(&conn) {
                Ok(r) => r,
                Err(e) => {
                    let _ = ready_tx.send(Err(e));
                    return;
                }
            };
            let stopper = Arc::new(Stopper {
                stop: AtomicBool::new(false),
                busy: AtomicBool::new(false),
                in_tx: AtomicBool::new(false),
                killed: AtomicBool::new(false),
                read_only: AtomicBool::new(false),
                privileged: ready.privileged,
                session: ready.session,
                logon: first,
                helper: Mutex::new(None),
            });
            let database = ready.database.clone();
            let _ = ready_tx.send(Ok((ready, stopper.clone())));
            let mut worker = Worker {
                conn,
                in_tx: false,
                metadata: false,
                stopper,
                database,
            };
            for job in rx {
                worker.run(job);
            }
        })
        .map_err(|e| DriverError::Io {
            message: e.to_string(),
        })?;
    let (ready, stopper) = ready_rx.await.map_err(|_| worker_gone())??;
    Ok(OracleConnection {
        jobs,
        server: ServerInfo {
            product: "Oracle".into(),
            version: ready.version,
            database: ready.database,
        },
        stopper,
    })
}

fn setup(conn: &Ora) -> Result<Ready> {
    let _ = conn.set_module("NiceDB");
    conn.execute(SESSION, &[]).map_err(|e| map_err(&e))?;
    let about = rows(conn, ABOUT, &[])?;
    let first = about.into_iter().next().unwrap_or_default();
    let cell = |i: usize| first.get(i).cloned().flatten().unwrap_or_default();
    let session = (
        conn.session_id().unwrap_or_default(),
        conn.serial_num().unwrap_or_default(),
    );
    Ok(Ready {
        version: conn.version().map(|v| v.to_string()).unwrap_or_default(),
        database: cell(0),
        privileged: cell(1) != "0",
        session,
    })
}

pub(crate) fn rows(conn: &Ora, sql: &str, params: &[&dyn ToDbValue]) -> Result<schema::Rows> {
    let cursor = conn.query(sql, params).map_err(|e| map_err(&e))?;
    let readers: Vec<value::Reader> = cursor.columns().iter().map(value::reader).collect();
    let mut out = Vec::new();
    for row in cursor {
        let row = row.map_err(|e| map_err(&e))?;
        let mut cells = Vec::with_capacity(readers.len());
        for (i, r) in readers.iter().enumerate() {
            cells.push(value::cell(&row, i, *r).map_err(|e| map_err(&e))?);
        }
        out.push(cells);
    }
    Ok(out)
}

fn named(conn: &Ora, sql: &str, params: &[(&str, &dyn ToDbValue)]) -> Result<schema::Rows> {
    let cursor = conn.query_named(sql, params).map_err(|e| map_err(&e))?;
    let readers: Vec<value::Reader> = cursor.columns().iter().map(value::reader).collect();
    let mut out = Vec::new();
    for row in cursor {
        let row = row.map_err(|e| map_err(&e))?;
        let mut cells = Vec::with_capacity(readers.len());
        for (i, r) in readers.iter().enumerate() {
            cells.push(value::cell(&row, i, *r).map_err(|e| map_err(&e))?);
        }
        out.push(cells);
    }
    Ok(out)
}

struct Worker {
    conn: Ora,
    in_tx: bool,
    metadata: bool,
    stopper: Arc<Stopper>,
    database: String,
}

impl Worker {
    fn run(&mut self, job: Job) {
        match job {
            Job::Execute {
                sql,
                params,
                out,
                reply,
            } => {
                self.stopper.stop.store(false, Ordering::Release);
                self.stopper.busy.store(true, Ordering::Release);
                let mut done = self.execute(&sql, &params, out);
                self.stopper.busy.store(false, Ordering::Release);
                self.stopper.in_tx.store(self.in_tx, Ordering::Release);
                if self.stopper.killed.swap(false, Ordering::AcqRel) && done.is_err() {
                    done = Err(DriverError::Io {
                        message: "stopped: the server closed the session to end the statement"
                            .into(),
                    });
                }
                let _ = reply.send(done);
            }
            Job::Schema { reply } => {
                let _ = reply.send(self.schema());
            }
            Job::Ddl { object, reply } => {
                let _ = reply.send(self.ddl(&object));
            }
            Job::Dependencies { object, reply } => {
                let _ = reply.send(self.dependencies(&object));
            }
        }
    }

    fn execute(
        &mut self,
        text: &str,
        params: &[Option<String>],
        out: mpsc::Sender<Chunk>,
    ) -> Result<Outcome> {
        let (sql, kind) = prepare(text);
        let command = sql.split_whitespace().next().map(str::to_uppercase);
        let plain = |rows_affected| {
            Ok(Outcome {
                command: command.clone(),
                rows_affected,
            })
        };
        match kind {
            Kind::Commit => {
                self.conn.commit().map_err(|e| map_err(&e))?;
                self.in_tx = false;
                return plain(None);
            }
            Kind::Rollback => {
                self.conn.rollback().map_err(|e| map_err(&e))?;
                self.in_tx = false;
                return plain(None);
            }
            Kind::Begin | Kind::InTransaction | Kind::Code | Kind::Other => {}
        }
        let binds: Vec<&dyn ToDbValue> = params.iter().map(|p| p as &dyn ToDbValue).collect();
        let mut stmt = self
            .conn
            .statement(&sql)
            .map_err(|e| map_err(&e))?
            .fetch_array_size(500)
            .prefetch_rows(200)
            .build()
            .map_err(|e| map_err(&e))?;
        if stmt.is_query() {
            let locks = sql.to_ascii_uppercase().contains("FOR UPDATE");
            let fetched = self.stream(stmt, &binds, out);
            if locks && !self.in_tx {
                let _ = self.conn.commit();
            }
            fetched?;
            return plain(None);
        }
        let guarded =
            !self.in_tx && kind != Kind::Begin && self.stopper.read_only.load(Ordering::Acquire);
        if guarded {
            self.conn
                .execute("SET TRANSACTION READ ONLY", &[])
                .map_err(|e| map_err(&e))?;
        }
        let result = stmt.execute(&binds);
        if guarded {
            let _ = self.conn.rollback();
        }
        let result = result.map_err(|e| map_err(&e))?;
        let affected = (stmt.is_dml() || stmt.is_plsql()).then(|| result.rows_affected());
        match kind {
            Kind::Begin => self.in_tx = true,
            Kind::InTransaction => {}
            _ if nd_sql::transaction_fit(&sql, Dialect::Oracle) == nd_sql::TxFit::Commits => {
                self.in_tx = false;
            }
            _ if !self.in_tx => self.conn.commit().map_err(|e| map_err(&e))?,
            _ => {}
        }
        plain(affected.filter(|_| stmt.is_dml()))
    }

    fn stream(
        &self,
        stmt: oracledb::Statement,
        binds: &[&dyn ToDbValue],
        out: mpsc::Sender<Chunk>,
    ) -> Result<()> {
        let cursor = stmt.query(binds).map_err(|e| map_err(&e))?;
        let readers: Vec<value::Reader> = cursor.columns().iter().map(value::reader).collect();
        let mut sink = RowBatcher::new(out);
        sink.columns_blocking(
            cursor
                .columns()
                .iter()
                .map(|c| Column {
                    name: c.name().to_owned(),
                    type_name: Some(c.data_type()),
                })
                .collect(),
        )?;
        for row in cursor {
            if self.stopper.stop.load(Ordering::Acquire) {
                return Err(DriverError::Cancelled);
            }
            let row = row.map_err(|e| map_err(&e))?;
            let mut cells = Vec::with_capacity(readers.len());
            for (i, r) in readers.iter().enumerate() {
                cells.push(value::cell(&row, i, *r).map_err(|e| map_err(&e))?);
            }
            sink.push_blocking(cells)?;
        }
        sink.flush_blocking()
    }

    fn schema(&mut self) -> Result<Schema> {
        let mut parts = BTreeMap::new();
        for (part, sql) in schema::queries() {
            parts.insert(part, rows(&self.conn, &sql, &[])?);
        }
        Ok(Schema {
            databases: vec![self.database.clone()],
            current_database: self.database.clone(),
            namespaces: schema::assemble(&parts),
        })
    }

    fn object_type(&self, o: &ObjectRef) -> Result<String> {
        let marked = o.signature.as_deref().unwrap_or_default();
        let candidates: &[&str] = match (o.kind, marked) {
            (DdlKind::Table, "SYNONYM") => &["SYNONYM"],
            (DdlKind::Table, _) => &["TABLE"],
            (DdlKind::View, _) => &["VIEW", "MATERIALIZED VIEW"],
            (DdlKind::Routine, "PACKAGE") => &["PACKAGE"],
            (DdlKind::Routine, "TYPE") => &["TYPE"],
            (DdlKind::Routine, _) => &["PROCEDURE", "FUNCTION"],
            (DdlKind::Trigger, _) => &["TRIGGER"],
            (DdlKind::Index, _) => &["INDEX"],
            (DdlKind::Sequence, _) => &["SEQUENCE"],
        };
        let list = candidates
            .iter()
            .map(|c| format!("'{c}'"))
            .collect::<Vec<_>>()
            .join(", ");
        let sql = format!(
            "SELECT object_type FROM all_objects WHERE owner = :o AND object_name = :n \
             AND object_type IN ({list}) ORDER BY object_type DESC"
        );
        let found = named(
            &self.conn,
            &sql,
            &[("o", &o.namespace.as_str()), ("n", &o.name.as_str())],
        )?;
        found
            .into_iter()
            .next()
            .and_then(|r| r.into_iter().next().flatten())
            .map(|t| t.replace(' ', "_"))
            .ok_or_else(|| o.not_found())
    }

    fn ddl(&mut self, o: &ObjectRef) -> Result<String> {
        if !self.metadata {
            self.conn.execute(METADATA, &[]).map_err(|e| map_err(&e))?;
            self.metadata = true;
        }
        let kind = self.object_type(o)?;
        let one = |sql: &str, t: &str| -> Result<Option<String>> {
            Ok(named(
                &self.conn,
                sql,
                &[
                    ("t", &t),
                    ("n", &o.name.as_str()),
                    ("o", &o.namespace.as_str()),
                ],
            )?
            .into_iter()
            .next()
            .and_then(|r| r.into_iter().next().flatten()))
        };
        let mut text = one("SELECT DBMS_METADATA.GET_DDL(:t, :n, :o) FROM dual", &kind)?
            .ok_or_else(|| o.not_found())?
            .trim()
            .to_owned();
        if kind == "TABLE" {
            for dependent in ["INDEX", "COMMENT"] {
                if let Ok(Some(more)) = one(
                    "SELECT DBMS_METADATA.GET_DEPENDENT_DDL(:t, :n, :o) FROM dual",
                    dependent,
                ) {
                    text.push_str("\n\n");
                    text.push_str(more.trim());
                }
            }
        }
        Ok(text)
    }

    fn dependencies(&self, o: &ObjectRef) -> Result<Vec<Dependency>> {
        let found = named(
            &self.conn,
            DEPENDENCIES,
            &[("o", &o.namespace.as_str()), ("n", &o.name.as_str())],
        )?;
        Ok(tidy(
            found
                .iter()
                .filter_map(|r| Dependency::from_row(r))
                .collect(),
        ))
    }
}

const DEPENDENCIES: &str = "SELECT dir, via, kind, ns, name, detail FROM (
  SELECT 'uses' dir, 'definition' via, \
    CASE d.referenced_type WHEN 'TABLE' THEN 'table' WHEN 'VIEW' THEN 'view' WHEN 'MATERIALIZED VIEW' THEN 'view' \
      WHEN 'SEQUENCE' THEN 'sequence' WHEN 'TRIGGER' THEN 'trigger' \
      WHEN 'PROCEDURE' THEN 'routine' WHEN 'FUNCTION' THEN 'routine' WHEN 'PACKAGE' THEN 'routine' END kind, \
    d.referenced_owner ns, d.referenced_name name, CAST(NULL AS VARCHAR2(128)) detail
    FROM all_dependencies d WHERE d.owner = :o AND d.name = :n AND d.referenced_owner NOT IN ('SYS', 'PUBLIC')
  UNION
  SELECT 'used_by', CASE WHEN d.type = 'TRIGGER' THEN 'trigger' ELSE 'definition' END, \
    CASE d.type WHEN 'TABLE' THEN 'table' WHEN 'VIEW' THEN 'view' WHEN 'MATERIALIZED VIEW' THEN 'view' \
      WHEN 'TRIGGER' THEN 'trigger' WHEN 'PROCEDURE' THEN 'routine' WHEN 'FUNCTION' THEN 'routine' \
      WHEN 'PACKAGE' THEN 'routine' WHEN 'PACKAGE BODY' THEN 'routine' END, d.owner, d.name, NULL
    FROM all_dependencies d WHERE d.referenced_owner = :o AND d.referenced_name = :n
  UNION
  SELECT 'uses', 'foreign_key', 'table', r.owner, r.table_name, c.constraint_name
    FROM all_constraints c JOIN all_constraints r ON r.owner = c.r_owner AND r.constraint_name = c.r_constraint_name
   WHERE c.constraint_type = 'R' AND c.owner = :o AND c.table_name = :n
  UNION
  SELECT 'used_by', 'foreign_key', 'table', c.owner, c.table_name, c.constraint_name
    FROM all_constraints c JOIN all_constraints r ON r.owner = c.r_owner AND r.constraint_name = c.r_constraint_name
   WHERE c.constraint_type = 'R' AND r.owner = :o AND r.table_name = :n
) WHERE kind IS NOT NULL";

#[async_trait::async_trait]
impl Connection for OracleConnection {
    fn capabilities(&self) -> nd_driver::Capabilities {
        let mut caps = nd_driver::Capabilities::relational(Dialect::Oracle)
            .groups(&[
                nd_driver::Group::Tables,
                nd_driver::Group::Views,
                nd_driver::Group::MaterializedViews,
                nd_driver::Group::Functions,
                nd_driver::Group::Packages,
                nd_driver::Group::Sequences,
                nd_driver::Group::Synonyms,
                nd_driver::Group::Types,
                nd_driver::Group::Triggers,
            ])
            .namespaces(nd_driver::NamespaceLevel::Always);
        caps.tree.databases = false;
        caps.tools.retain(|t| *t != nd_driver::Tool::Backup);
        caps.cancel = if self.stopper.privileged {
            nd_driver::Cancel::Server
        } else {
            nd_driver::Cancel::BetweenFetches
        };
        caps
    }

    fn server(&self) -> &ServerInfo {
        &self.server
    }

    fn canceller(&self) -> Arc<dyn Canceller> {
        Arc::new(OracleCanceller(self.stopper.clone()))
    }

    async fn execute(&self, sql: &str, out: mpsc::Sender<Chunk>) -> Result<Outcome> {
        self.execute_bound(sql, &[], out).await
    }

    async fn execute_bound(
        &self,
        sql: &str,
        params: &[Option<String>],
        out: mpsc::Sender<Chunk>,
    ) -> Result<Outcome> {
        let (reply, rx) = oneshot::channel();
        self.jobs
            .send(Job::Execute {
                sql: sql.to_owned(),
                params: params.to_vec(),
                out,
                reply,
            })
            .map_err(|_| worker_gone())?;
        rx.await.map_err(|_| worker_gone())?
    }

    async fn schema(&self) -> Result<Schema> {
        let (reply, rx) = oneshot::channel();
        self.jobs
            .send(Job::Schema { reply })
            .map_err(|_| worker_gone())?;
        rx.await.map_err(|_| worker_gone())?
    }

    async fn ddl(&self, object: &ObjectRef) -> Result<String> {
        let (reply, rx) = oneshot::channel();
        self.jobs
            .send(Job::Ddl {
                object: object.clone(),
                reply,
            })
            .map_err(|_| worker_gone())?;
        rx.await.map_err(|_| worker_gone())?
    }

    async fn dependencies(&self, object: &ObjectRef) -> Result<Vec<Dependency>> {
        let (reply, rx) = oneshot::channel();
        self.jobs
            .send(Job::Dependencies {
                object: object.clone(),
                reply,
            })
            .map_err(|_| worker_gone())?;
        rx.await.map_err(|_| worker_gone())?
    }

    async fn set_read_only(&self, on: bool) -> Result<bool> {
        self.stopper.read_only.store(on, Ordering::Release);
        Ok(true)
    }

    async fn ping(&self) -> Result<()> {
        let (tx, mut rx) = mpsc::channel(4);
        let drain = async { while rx.recv().await.is_some() {} };
        let (done, ()) = tokio::join!(self.execute("SELECT 1 FROM dual", tx), drain);
        done.map(|_| ())
    }
}
