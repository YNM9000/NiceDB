mod client;
mod schema;
mod value;

use std::net::IpAddr;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;

use nd_driver::{
    Canceller, Chunk, Connection, DdlKind, Dependency, Dialect, DriverError, FirebirdTarget,
    ObjectRef, Outcome, Result, RowBatcher, Schema, ServerInfo, tidy,
};
use rsfbclient_core::SqlType;
use rsfbclient_rust::RustFbClientAttachmentConfig;
use tokio::sync::{mpsc, oneshot};

use client::{Fb, Ran, Rows};
pub use schema::NAMESPACE;

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
    Plan {
        sql: String,
        reply: oneshot::Sender<Result<String>>,
    },
}

pub struct FirebirdConnection {
    jobs: std::sync::mpsc::Sender<Job>,
    server: ServerInfo,
    stopper: Arc<Stopper>,
    read_only: Arc<AtomicBool>,
}

struct Stopper {
    config: RustFbClientAttachmentConfig,
    attachment: String,
    busy: Arc<AtomicBool>,
}

const CANCEL: &str = "DELETE FROM MON$STATEMENTS WHERE MON$ATTACHMENT_ID = ? AND MON$STATE = 1";

#[async_trait::async_trait]
impl Canceller for Stopper {
    async fn cancel(&self) {
        if !self.busy.load(Ordering::Acquire) {
            return;
        }
        let config = self.config.clone();
        let id = SqlType::Text(self.attachment.clone());
        let done = tokio::task::spawn_blocking(move || {
            let mut side = Fb::attach(&config)?;
            let out = side.query(CANCEL, vec![id]);
            side.detach();
            out
        })
        .await;
        match done {
            Ok(Ok(_)) => {}
            Ok(Err(e)) => tracing::warn!("firebird cancel: {e}"),
            Err(e) => tracing::warn!("firebird cancel: {e}"),
        }
    }
}

pub async fn connect(
    target: &FirebirdTarget,
    password: Option<&str>,
    via: Option<(IpAddr, u16)>,
) -> Result<FirebirdConnection> {
    let (host, port) = via.map_or_else(
        || (target.host.clone(), target.port),
        |(ip, p)| (ip.to_string(), p),
    );
    let config = RustFbClientAttachmentConfig {
        host,
        port,
        db_name: target.database.clone(),
        user: target.user.clone(),
        pass: password.unwrap_or_default().to_owned(),
        role_name: target.role.clone().filter(|r| !r.is_empty()),
    };
    let database = target.database.clone();
    let (ready_tx, ready_rx) = oneshot::channel::<Result<(String, String)>>();
    let (jobs, rx) = std::sync::mpsc::channel::<Job>();
    let busy = Arc::new(AtomicBool::new(false));
    let flag = busy.clone();
    let read_only = Arc::new(AtomicBool::new(false));
    let locked = read_only.clone();
    let attach = config.clone();
    thread::Builder::new()
        .name("nd-firebird".into())
        .spawn(move || {
            let mut fb = match Fb::attach(&attach) {
                Ok(fb) => fb,
                Err(e) => {
                    let _ = ready_tx.send(Err(e));
                    return;
                }
            };
            fb.read_only = locked;
            let first = |fb: &mut Fb, sql: &str| {
                fb.query(sql, vec![])
                    .ok()
                    .and_then(|r| r.into_iter().next())
                    .and_then(|r| r.into_iter().next().flatten())
                    .unwrap_or_default()
            };
            let version = first(&mut fb, VERSION);
            let attachment = first(&mut fb, ATTACHMENT);
            let _ = ready_tx.send(Ok((version, attachment)));
            for job in rx {
                serve(&mut fb, job, &database, &flag);
            }
            fb.detach();
        })
        .map_err(|e| DriverError::Io {
            message: e.to_string(),
        })?;
    let (version, attachment) = ready_rx.await.map_err(|_| worker_gone())??;
    let short = target
        .database
        .rsplit(['/', '\\'])
        .next()
        .unwrap_or(&target.database)
        .to_owned();
    Ok(FirebirdConnection {
        jobs,
        server: ServerInfo {
            product: "Firebird".into(),
            version,
            database: short,
        },
        stopper: Arc::new(Stopper {
            config,
            attachment,
            busy,
        }),
        read_only,
    })
}

fn serve(fb: &mut Fb, job: Job, database: &str, busy: &AtomicBool) {
    match job {
        Job::Execute {
            sql,
            params,
            out,
            reply,
        } => {
            busy.store(true, Ordering::Release);
            let done = execute(fb, &sql, &params, out);
            busy.store(false, Ordering::Release);
            let _ = reply.send(done);
        }
        Job::Schema { reply } => {
            let _ = reply.send(load_schema(fb, database));
        }
        Job::Ddl { object, reply } => {
            let _ = reply.send(ddl(fb, &object));
        }
        Job::Dependencies { object, reply } => {
            let _ = reply.send(dependencies(fb, &object));
        }
        Job::Plan { sql, reply } => {
            let _ = reply.send(fb.plan(&sql));
        }
    }
}

const VERSION: &str = "SELECT rdb$get_context('SYSTEM', 'ENGINE_VERSION') FROM rdb$database";
const ATTACHMENT: &str = "SELECT CURRENT_CONNECTION FROM rdb$database";

fn worker_gone() -> DriverError {
    DriverError::Io {
        message: "firebird worker exited".into(),
    }
}

#[derive(Debug, PartialEq, Eq)]
enum Kind {
    Begin,
    Commit,
    Rollback,
    Plain,
}

fn kind(sql: &str) -> Kind {
    let words: Vec<String> = sql
        .split_whitespace()
        .take(3)
        .map(str::to_ascii_uppercase)
        .collect();
    let w = |i: usize| words.get(i).map_or("", String::as_str);
    match (w(0), w(1)) {
        ("SET", "TRANSACTION") => Kind::Begin,
        ("COMMIT", _) if w(1) != "RETAIN" => Kind::Commit,
        ("ROLLBACK", "" | "WORK") if w(2).is_empty() => Kind::Rollback,
        _ => Kind::Plain,
    }
}

fn execute(
    fb: &mut Fb,
    sql: &str,
    params: &[Option<String>],
    out: mpsc::Sender<Chunk>,
) -> Result<Outcome> {
    let command = sql.split_whitespace().next().map(str::to_uppercase);
    let bound: Vec<SqlType> = params.iter().map(|p| value::param(p.as_ref())).collect();
    let done = |rows_affected| {
        Ok(Outcome {
            command: command.clone(),
            rows_affected,
        })
    };
    match kind(sql) {
        Kind::Begin => {
            fb.begin()?;
            return done(None);
        }
        Kind::Commit => {
            fb.end(true)?;
            return done(None);
        }
        Kind::Rollback => {
            fb.end(false)?;
            return done(None);
        }
        Kind::Plain => {}
    }
    let sink = std::cell::RefCell::new(RowBatcher::new(out));
    let ran = fb.run(
        sql,
        bound,
        &mut |cols| sink.borrow().columns_blocking(cols),
        &mut |row| sink.borrow_mut().push_blocking(row),
    )?;
    sink.borrow_mut().flush_blocking()?;
    match ran {
        Ran::Count(n) => done(Some(n)),
        Ran::Done => done(None),
    }
}

fn load_schema(fb: &mut Fb, database: &str) -> Result<Schema> {
    let mut q = |sql: &str| fb.query(sql, vec![]);
    let catalog = schema::Catalog {
        relations: q(schema::RELATIONS)?,
        columns: q(schema::COLUMNS)?,
        indexes: q(schema::INDEXES)?,
        foreign_keys: q(schema::FOREIGN_KEYS)?,
        procedures: q(schema::PROCEDURES)?,
        generators: q(schema::GENERATORS)?,
        triggers: q(schema::TRIGGERS)?,
    };
    Ok(Schema {
        databases: vec![database.to_owned()],
        current_database: database.to_owned(),
        namespaces: vec![schema::assemble(&catalog)],
    })
}

fn text(name: &str) -> Vec<SqlType> {
    vec![SqlType::Text(name.to_owned())]
}

fn first_cell(rows: Rows) -> Option<String> {
    rows.into_iter()
        .next()
        .and_then(|r| r.into_iter().next().flatten())
}

fn source(fb: &mut Fb, sql: &str, o: &ObjectRef) -> Result<String> {
    first_cell(fb.query(sql, text(&o.name))?).ok_or_else(|| o.not_found())
}

fn index_ddl(fb: &mut Fb, o: &ObjectRef) -> Result<String> {
    let facts = fb.query(schema::INDEX_FACTS, text(&o.name))?;
    let r = facts.into_iter().next().ok_or_else(|| o.not_found())?;
    let cell = |i: usize| r.get(i).cloned().flatten();
    let condition = fb
        .query(schema::INDEX_CONDITION, text(&o.name))
        .ok()
        .and_then(first_cell);
    let columns = fb
        .query(schema::INDEX_SEGMENTS, text(&o.name))?
        .into_iter()
        .filter_map(|r| r.into_iter().next().flatten())
        .collect();
    let facts = schema::IndexFacts {
        table: cell(0).unwrap_or_default(),
        unique: cell(1).as_deref() == Some("1"),
        descending: cell(2).as_deref() == Some("1"),
        expression: cell(3),
        condition,
        inactive: cell(4).as_deref() == Some("1"),
        columns,
        constraint: cell(5).zip(cell(6)),
    };
    Ok(schema::index_ddl(&o.name, &facts))
}

fn ddl(fb: &mut Fb, o: &ObjectRef) -> Result<String> {
    let q = |n: &str| nd_sql::name(Dialect::Firebird, n);
    match o.kind {
        DdlKind::Table => {
            let s = load_schema(fb, "")?;
            let t = s.namespaces[0]
                .tables
                .iter()
                .find(|t| t.name == o.name)
                .ok_or_else(|| o.not_found())?;
            Ok(schema::create_table(t))
        }
        DdlKind::View => {
            let body = source(
                fb,
                "SELECT RDB$VIEW_SOURCE FROM RDB$RELATIONS WHERE RDB$RELATION_NAME = ?",
                o,
            )?;
            Ok(format!(
                "CREATE OR ALTER VIEW {} AS\n{};",
                q(&o.name),
                body.trim()
            ))
        }
        DdlKind::Routine => source(
            fb,
            "SELECT RDB$PROCEDURE_SOURCE FROM RDB$PROCEDURES WHERE RDB$PROCEDURE_NAME = ?",
            o,
        ),
        DdlKind::Trigger => source(
            fb,
            "SELECT RDB$TRIGGER_SOURCE FROM RDB$TRIGGERS WHERE RDB$TRIGGER_NAME = ?",
            o,
        ),
        DdlKind::Sequence => Ok(format!("CREATE SEQUENCE {};", q(&o.name))),
        DdlKind::Index => index_ddl(fb, o),
    }
}

fn dependencies(fb: &mut Fb, o: &ObjectRef) -> Result<Vec<Dependency>> {
    let rows = fb.query(schema::DEPENDENCIES, vec![SqlType::Text(o.name.clone()); 4])?;
    Ok(tidy(
        rows.iter().filter_map(|r| schema::dependency(r)).collect(),
    ))
}

impl FirebirdConnection {
    async fn ask<T>(&self, job: impl FnOnce(oneshot::Sender<Result<T>>) -> Job) -> Result<T> {
        let (reply, rx) = oneshot::channel();
        self.jobs.send(job(reply)).map_err(|_| worker_gone())?;
        rx.await.map_err(|_| worker_gone())?
    }
}

#[async_trait::async_trait]
impl Connection for FirebirdConnection {
    fn capabilities(&self) -> nd_driver::Capabilities {
        let mut caps = nd_driver::Capabilities::relational(Dialect::Firebird).groups(&[
            nd_driver::Group::Tables,
            nd_driver::Group::Views,
            nd_driver::Group::Functions,
            nd_driver::Group::Sequences,
            nd_driver::Group::Triggers,
        ]);
        caps.tools.retain(|t| *t != nd_driver::Tool::Backup);
        caps
    }

    fn server(&self) -> &ServerInfo {
        &self.server
    }

    fn canceller(&self) -> Arc<dyn Canceller> {
        self.stopper.clone()
    }

    async fn set_read_only(&self, on: bool) -> Result<bool> {
        self.read_only.store(on, Ordering::Release);
        Ok(true)
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

    async fn explain_native(&self, command: &str) -> Result<String> {
        self.ask(|reply| Job::Plan {
            sql: command.to_owned(),
            reply,
        })
        .await
    }

    async fn ping(&self) -> Result<()> {
        let (tx, mut rx) = mpsc::channel(4);
        let drain = async { while rx.recv().await.is_some() {} };
        let (done, ()) = tokio::join!(self.execute("SELECT 1 FROM RDB$DATABASE", tx), drain);
        done.map(|_| ())
    }
}
