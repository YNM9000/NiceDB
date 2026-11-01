use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;

use hdbconnect::{ConnectParamsBuilder, HdbResponse, HdbValue};
use nd_driver::{
    Canceller, Chunk, Column, Connection, DdlKind, Dependency, DriverError, Namespace, ObjectRef,
    Outcome, Result, RowBatcher, Schema, ServerInfo, assemble_catalog, tidy,
};
use tokio::sync::{mpsc, oneshot};

use crate::catalog;
use crate::value;

type Rows = Vec<Vec<Option<String>>>;

fn server(e: &hdbconnect::HdbError) -> Option<&hdbconnect::ServerError> {
    e.server_error().or_else(|| match e {
        hdbconnect::HdbError::Authentication { source } => source.server_error(),
        _ => None,
    })
}

fn describe(e: &hdbconnect::HdbError) -> String {
    if let Some(s) = server(e) {
        return s.text().to_owned();
    }
    let mut out = e.to_string();
    let mut next = e.inner();
    while let Some(cause) = next {
        out.push_str(": ");
        out.push_str(&cause.to_string());
        next = cause.source();
    }
    out
}

fn map_err(e: hdbconnect::HdbError) -> DriverError {
    DriverError::Query {
        message: describe(&e),
        code: server(&e).map(|s| s.code().to_string()),
        detail: None,
        hint: None,
        offset: None,
    }
}

fn ddl_with_rows(e: &hdbconnect::HdbError) -> bool {
    matches!(e, hdbconnect::HdbError::Impl(m) if m.contains("affected-row-count > 0"))
}

fn at(sql: &str, position: i32) -> Option<u32> {
    let n = usize::try_from(position).ok()?.checked_sub(1)?;
    let byte = sql
        .char_indices()
        .map(|(b, _)| b)
        .chain(std::iter::once(sql.len()))
        .nth(n)?;
    u32::try_from(byte).ok()
}

fn statement_err(e: hdbconnect::HdbError, sql: &str) -> DriverError {
    let offset = server(&e).and_then(|s| at(sql, s.position()));
    match map_err(e) {
        DriverError::Query {
            message,
            code,
            detail,
            hint,
            ..
        } => DriverError::Query {
            message,
            code,
            detail,
            hint,
            offset,
        },
        other => other,
    }
}

fn connect_err(e: &hdbconnect::HdbError) -> DriverError {
    let message = describe(e);
    match e {
        hdbconnect::HdbError::Authentication { .. } => DriverError::Auth { message },
        hdbconnect::HdbError::TlsInit { .. }
        | hdbconnect::HdbError::TlsProtocol { .. }
        | hdbconnect::HdbError::TlsInvalidDnsName { .. } => DriverError::Tls { message },
        _ => DriverError::Connect { message },
    }
}

fn open(address: &str, password: &str) -> Result<hdbconnect::Connection> {
    let _ = rustls::crypto::ring::default_provider().install_default();
    let mut params = ConnectParamsBuilder::from(address).map_err(|e| connect_err(&e))?;
    params.password(password);
    hdbconnect::Connection::new(params).map_err(|e| connect_err(&e))
}

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

struct Stopper {
    address: String,
    password: String,
    session: String,
    busy: Arc<AtomicBool>,
}

#[async_trait::async_trait]
impl Canceller for Stopper {
    async fn cancel(&self) {
        if !self.busy.load(Ordering::Acquire) || !self.session.bytes().all(|b| b.is_ascii_digit()) {
            return;
        }
        let (address, password) = (self.address.clone(), self.password.clone());
        let sql = format!("ALTER SYSTEM CANCEL SESSION '{}'", self.session);
        let done = tokio::task::spawn_blocking(move || {
            let side = open(&address, &password)?;
            side.exec(sql).map_err(map_err)
        })
        .await;
        if let Ok(Err(e)) = done {
            tracing::warn!("hana cancel: {e}");
        }
    }
}

pub struct HanaConnection {
    jobs: std::sync::mpsc::Sender<Job>,
    server: ServerInfo,
    stopper: Arc<Stopper>,
}

pub async fn connect(address: String, password: String) -> Result<Box<dyn Connection>> {
    let busy = Arc::new(AtomicBool::new(false));
    let flag = busy.clone();
    let (ready_tx, ready_rx) = oneshot::channel::<Result<(ServerInfo, String)>>();
    let (jobs, rx) = std::sync::mpsc::channel::<Job>();
    let (addr, pw) = (address.clone(), password.clone());
    thread::Builder::new()
        .name("nd-hana".into())
        .spawn(move || {
            let conn = match open(&addr, &pw) {
                Ok(c) => c,
                Err(e) => {
                    let _ = ready_tx.send(Err(e));
                    return;
                }
            };
            let mut w = Worker {
                conn,
                own: String::new(),
                database: String::new(),
            };
            let version = w.first(catalog::VERSION);
            let database = w.first(catalog::DATABASE);
            w.database.clone_from(&database);
            w.own = w.first(catalog::CURRENT_SCHEMA);
            let session = w.first("SELECT CURRENT_CONNECTION FROM DUMMY");
            let _ = ready_tx.send(Ok((
                ServerInfo {
                    product: "SAP HANA".into(),
                    version,
                    database,
                },
                session,
            )));
            for job in rx {
                if let Job::Execute { .. } = job {
                    flag.store(true, Ordering::Release);
                }
                w.serve(job);
                flag.store(false, Ordering::Release);
            }
        })
        .map_err(|e| DriverError::Io {
            message: e.to_string(),
        })?;
    let (server, session) = ready_rx.await.map_err(|_| gone())??;
    Ok(Box::new(HanaConnection {
        jobs,
        server,
        stopper: Arc::new(Stopper {
            address,
            password,
            session,
            busy,
        }),
    }))
}

fn gone() -> DriverError {
    DriverError::Io {
        message: "hana worker exited".into(),
    }
}

struct Worker {
    conn: hdbconnect::Connection,
    own: String,
    database: String,
}

impl Worker {
    fn serve(&mut self, job: Job) {
        match job {
            Job::Execute {
                sql,
                params,
                out,
                reply,
            } => {
                let _ = reply.send(self.execute(&sql, &params, out));
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

    fn first(&mut self, sql: &str) -> String {
        self.query(sql, &[])
            .ok()
            .and_then(|r| r.into_iter().next())
            .and_then(|r| r.into_iter().next().flatten())
            .unwrap_or_default()
    }

    fn query(&mut self, sql: &str, params: &[Option<String>]) -> Result<Rows> {
        let mut rows = Vec::new();
        self.run(sql, params, &mut |_| Ok(()), &mut |r| {
            rows.push(r);
            Ok(())
        })?;
        Ok(rows)
    }

    fn run(
        &mut self,
        sql: &str,
        params: &[Option<String>],
        head: &mut dyn FnMut(Vec<Column>) -> Result<()>,
        row: &mut dyn FnMut(Vec<Option<String>>) -> Result<()>,
    ) -> Result<Option<u64>> {
        let answer = if params.is_empty() {
            self.conn.statement(sql)
        } else {
            let mut stmt = self.conn.prepare(sql).map_err(|e| statement_err(e, sql))?;
            let values = params
                .iter()
                .map(|p| p.clone().map_or(HdbValue::NULL, HdbValue::STRING))
                .collect();
            stmt.execute_row(values)
        };
        let mut response: HdbResponse = match answer {
            Ok(r) => r,
            Err(e) if ddl_with_rows(&e) => return Ok(None),
            Err(e) => return Err(statement_err(e, sql)),
        };
        if let Ok(rs) = response.get_result_set() {
            let columns = rs
                .metadata()
                .iter()
                .map(|f| Column {
                    name: f.displayname().to_owned(),
                    type_name: Some(value::type_name(f.type_id())),
                })
                .collect();
            head(columns)?;
            for r in rs {
                let mut cells = Vec::new();
                for v in r.map_err(map_err)? {
                    cells.push(value::text(v)?);
                }
                row(cells)?;
            }
            return Ok(None);
        }
        Ok(response
            .get_affected_rows()
            .ok()
            .and_then(|v| v.first().copied())
            .and_then(|n| u64::try_from(n).ok()))
    }

    fn execute(
        &mut self,
        sql: &str,
        params: &[Option<String>],
        out: mpsc::Sender<Chunk>,
    ) -> Result<Outcome> {
        let command = sql.split_whitespace().next().map(str::to_uppercase);
        let upper = sql.trim().trim_end_matches(';').to_ascii_uppercase();
        let words: Vec<&str> = upper.split_whitespace().collect();
        match words.as_slice() {
            ["SET", "TRANSACTION", ..] => {
                self.conn.set_auto_commit(false).map_err(map_err)?;
                self.run(sql, params, &mut |_| Ok(()), &mut |_| Ok(()))?;
            }
            ["COMMIT"] | ["COMMIT", "WORK"] => {
                self.conn.commit().map_err(map_err)?;
                self.conn.set_auto_commit(true).map_err(map_err)?;
            }
            ["ROLLBACK"] | ["ROLLBACK", "WORK"] => {
                self.conn.rollback().map_err(map_err)?;
                self.conn.set_auto_commit(true).map_err(map_err)?;
            }
            _ => {
                let sink = std::cell::RefCell::new(RowBatcher::new(out));
                let n = self.run(
                    sql,
                    params,
                    &mut |c| sink.borrow().columns_blocking(c),
                    &mut |r| sink.borrow_mut().push_blocking(r),
                )?;
                sink.borrow_mut().flush_blocking()?;
                return Ok(Outcome {
                    command,
                    rows_affected: n,
                });
            }
        }
        Ok(Outcome {
            command,
            rows_affected: None,
        })
    }

    fn schema(&mut self) -> Result<Schema> {
        let mut answers = Vec::new();
        for sql in catalog::catalog_sql() {
            answers.push(self.query(&sql, &[])?);
        }
        let rows = catalog::catalog(answers);
        let own = (!self.own.is_empty()).then_some(self.own.as_str());
        let mut namespaces = assemble_catalog(&rows, own);
        if let Some(own) = own
            && !namespaces.iter().any(|n| n.name == own)
        {
            namespaces.insert(0, Namespace::new(own.to_owned()));
        }
        Ok(Schema {
            databases: Vec::new(),
            current_database: self.database.clone(),
            namespaces,
        })
    }

    fn one(&mut self, sql: &str, params: &[&str], o: &ObjectRef) -> Result<String> {
        let p: Vec<Option<String>> = params.iter().map(|v| Some((*v).to_owned())).collect();
        self.query(sql, &p)?
            .into_iter()
            .find_map(|r| r.into_iter().last().flatten())
            .ok_or_else(|| o.not_found())
    }

    fn ddl(&mut self, o: &ObjectRef) -> Result<String> {
        let (ns, name) = (o.namespace.as_str(), o.name.as_str());
        match o.kind {
            DdlKind::View => {
                let body = self.one(catalog::VIEW_TEXT, &[ns, name], o)?;
                Ok(catalog::create_view(ns, name, &body))
            }
            DdlKind::Routine => self.one(catalog::ROUTINE_TEXT, &[ns, name, ns, name], o),
            DdlKind::Trigger => self.one(catalog::TRIGGER_TEXT, &[ns, name], o),
            DdlKind::Table | DdlKind::Index | DdlKind::Sequence => {
                self.one(catalog::OBJECT_DEFINITION, &[ns, name], o)
            }
        }
    }

    fn dependencies(&mut self, o: &ObjectRef) -> Result<Vec<Dependency>> {
        let mut p = Vec::new();
        for _ in 0..4 {
            p.push(Some(o.namespace.clone()));
            p.push(Some(o.name.clone()));
        }
        let rows = self.query(catalog::DEPENDENCIES, &p)?;
        Ok(tidy(
            rows.iter()
                .filter_map(|r| Dependency::from_row(r))
                .collect(),
        ))
    }
}

impl HanaConnection {
    async fn ask<T>(&self, job: impl FnOnce(oneshot::Sender<Result<T>>) -> Job) -> Result<T> {
        let (reply, rx) = oneshot::channel();
        self.jobs.send(job(reply)).map_err(|_| gone())?;
        rx.await.map_err(|_| gone())?
    }
}

#[async_trait::async_trait]
impl Connection for HanaConnection {
    fn capabilities(&self) -> nd_driver::Capabilities {
        crate::capabilities()
    }

    fn server(&self) -> &ServerInfo {
        &self.server
    }

    fn canceller(&self) -> Arc<dyn Canceller> {
        self.stopper.clone()
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
        self.ask(|reply| Job::Execute {
            sql: sql.to_owned(),
            params: params.to_vec(),
            out,
            reply,
        })
        .await
    }

    async fn schema(&self) -> Result<Schema> {
        self.ask(|reply| Job::Schema { reply }).await
    }

    async fn ddl(&self, object: &ObjectRef) -> Result<String> {
        self.ask(|reply| Job::Ddl {
            object: object.clone(),
            reply,
        })
        .await
    }

    async fn dependencies(&self, object: &ObjectRef) -> Result<Vec<Dependency>> {
        self.ask(|reply| Job::Dependencies {
            object: object.clone(),
            reply,
        })
        .await
    }

    async fn ping(&self) -> Result<()> {
        let (tx, mut rx) = mpsc::channel(4);
        let drain = async { while rx.recv().await.is_some() {} };
        let (done, ()) = tokio::join!(self.execute("SELECT 1 FROM DUMMY", tx), drain);
        done.map(|_| ())
    }
}
