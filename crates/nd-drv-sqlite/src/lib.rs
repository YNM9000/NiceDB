pub mod ddl;
mod schema;
mod value;

use std::sync::Arc;
use std::thread;

use nd_driver::{
    Canceller, Chunk, Column, Connection, Dialect, DriverError, Outcome, Result, RowBatcher,
    Schema, ServerInfo, SqliteTarget,
};
use rusqlite::{InterruptHandle, OpenFlags};
use tokio::sync::{mpsc, oneshot};

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
        object: nd_driver::ObjectRef,
        reply: oneshot::Sender<Result<String>>,
    },
    Deps {
        object: nd_driver::ObjectRef,
        reply: oneshot::Sender<Result<Vec<nd_driver::Dependency>>>,
    },
}

pub struct SqliteConnection {
    jobs: std::sync::mpsc::Sender<Job>,
    interrupt: Arc<InterruptHandle>,
    server: ServerInfo,
}

pub async fn connect(target: &SqliteTarget) -> Result<SqliteConnection> {
    let worker_target = target.clone();
    let (ready_tx, ready_rx) = oneshot::channel();
    let (jobs, rx) = std::sync::mpsc::channel::<Job>();
    thread::Builder::new()
        .name("nd-sqlite".into())
        .spawn(move || {
            let conn = match open(&worker_target) {
                Ok(c) => c,
                Err(e) => {
                    let _ = ready_tx.send(Err(e));
                    return;
                }
            };
            let _ = ready_tx.send(Ok(Arc::new(conn.get_interrupt_handle())));
            for job in rx {
                match job {
                    Job::Execute {
                        sql,
                        params,
                        out,
                        reply,
                    } => {
                        let _ = reply.send(execute(&conn, &sql, &params, out));
                    }
                    Job::Schema { reply } => {
                        let _ = reply.send(schema::load(&conn).map_err(map_err));
                    }
                    Job::Ddl { object, reply } => {
                        let _ = reply.send(ddl::load(&conn, &object));
                    }
                    Job::Deps { object, reply } => {
                        let _ = reply.send(ddl::dependencies(&conn, &object));
                    }
                }
            }
        })
        .map_err(|e| DriverError::Io {
            message: e.to_string(),
        })?;

    let interrupt = ready_rx.await.map_err(|_| worker_gone())??;
    let database = std::path::Path::new(&target.path)
        .file_name()
        .map_or_else(|| target.path.clone(), |n| n.to_string_lossy().into_owned());
    Ok(SqliteConnection {
        jobs,
        interrupt,
        server: ServerInfo {
            product: "SQLite".into(),
            version: rusqlite::version().into(),
            database,
        },
    })
}

fn worker_gone() -> DriverError {
    DriverError::Io {
        message: "sqlite worker exited".into(),
    }
}

fn open(target: &SqliteTarget) -> Result<rusqlite::Connection> {
    let path = std::path::Path::new(&target.path);
    if !path.exists() {
        return Err(DriverError::Connect {
            message: format!("file not found: {}", target.path),
        });
    }
    let flags = if target.read_only {
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX
    } else {
        OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_NO_MUTEX
    };
    let conn =
        rusqlite::Connection::open_with_flags(path, flags).map_err(|e| DriverError::Connect {
            message: e.to_string(),
        })?;
    conn.busy_timeout(std::time::Duration::from_secs(5))
        .map_err(map_err)?;
    conn.query_row("SELECT count(*) FROM sqlite_schema", [], |_| Ok(()))
        .map_err(|e| DriverError::Connect {
            message: e.to_string(),
        })?;
    Ok(conn)
}

fn execute(
    conn: &rusqlite::Connection,
    sql: &str,
    params: &[Option<String>],
    out: mpsc::Sender<Chunk>,
) -> Result<Outcome> {
    let mut stmt = conn.prepare(sql).map_err(map_err)?;
    for (i, p) in params.iter().enumerate() {
        stmt.raw_bind_parameter(i + 1, p).map_err(map_err)?;
    }
    let command = sql.split_whitespace().next().map(str::to_uppercase);
    let n = stmt.column_count();
    if n == 0 {
        stmt.raw_execute().map_err(map_err)?;
        return Ok(Outcome {
            command,
            rows_affected: Some(conn.changes()),
        });
    }
    let columns: Vec<Column> = stmt
        .columns()
        .iter()
        .map(|c| Column {
            name: c.name().to_owned(),
            type_name: c.decl_type().map(str::to_owned),
        })
        .collect();
    let writes = !stmt.readonly();
    let mut sink = RowBatcher::new(out);
    sink.columns_blocking(columns)?;
    let mut rows = stmt.raw_query();
    while let Some(row) = rows.next().map_err(map_err)? {
        let mut cells = Vec::with_capacity(n);
        for i in 0..n {
            cells.push(value::display(row.get_ref(i).map_err(map_err)?));
        }
        sink.push_blocking(cells)?;
    }
    sink.flush_blocking()?;
    Ok(Outcome {
        command,
        rows_affected: writes.then(|| conn.changes()),
    })
}

pub(crate) fn map_err(e: rusqlite::Error) -> DriverError {
    match e {
        rusqlite::Error::SqliteFailure(f, _)
            if f.code == rusqlite::ErrorCode::OperationInterrupted =>
        {
            DriverError::Cancelled
        }
        rusqlite::Error::SqlInputError { msg, offset, .. } => DriverError::Query {
            message: msg,
            code: None,
            detail: None,
            hint: None,
            offset: u32::try_from(offset).ok(),
        },
        other => DriverError::query(other.to_string()),
    }
}

struct Interrupter(Arc<InterruptHandle>);

#[async_trait::async_trait]
impl Canceller for Interrupter {
    async fn cancel(&self) {
        self.0.interrupt();
    }
}

#[async_trait::async_trait]
impl Connection for SqliteConnection {
    fn capabilities(&self) -> nd_driver::Capabilities {
        nd_driver::Capabilities::relational(Dialect::Sqlite)
            .embedded()
            .groups(&[
                nd_driver::Group::Tables,
                nd_driver::Group::Views,
                nd_driver::Group::Triggers,
            ])
    }

    fn server(&self) -> &ServerInfo {
        &self.server
    }

    fn canceller(&self) -> Arc<dyn Canceller> {
        Arc::new(Interrupter(self.interrupt.clone()))
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

    async fn ddl(&self, object: &nd_driver::ObjectRef) -> Result<String> {
        let (reply, rx) = oneshot::channel();
        self.jobs
            .send(Job::Ddl {
                object: object.clone(),
                reply,
            })
            .map_err(|_| worker_gone())?;
        rx.await.map_err(|_| worker_gone())?
    }

    async fn dependencies(
        &self,
        object: &nd_driver::ObjectRef,
    ) -> Result<Vec<nd_driver::Dependency>> {
        let (reply, rx) = oneshot::channel();
        self.jobs
            .send(Job::Deps {
                object: object.clone(),
                reply,
            })
            .map_err(|_| worker_gone())?;
        rx.await.map_err(|_| worker_gone())?
    }
}
