mod schema;

use std::fmt::Write as _;
use std::future::Future;
use std::net::SocketAddr;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};

use libsql::Value;
use nd_driver::{
    Canceller, Chunk, Column, Connection, Dialect, DriverError, LibsqlTarget, Outcome, Result, Row,
    RowBatcher, Schema, ServerInfo, SqliteTarget,
};
use tokio::sync::{Mutex, mpsc, watch};

const QUERIES: &[&str] = &["SELECT", "WITH", "VALUES", "PRAGMA", "EXPLAIN"];

pub async fn connect(
    target: &LibsqlTarget,
    token: Option<&str>,
    via: Option<SocketAddr>,
) -> Result<Box<dyn Connection>> {
    match target {
        LibsqlTarget::File { path, read_only } => {
            let inner = nd_drv_sqlite::connect(&SqliteTarget {
                path: path.clone(),
                read_only: *read_only,
            })
            .await?;
            let mut server = inner.server().clone();
            server.product = "libSQL".into();
            Ok(Box::new(Local { inner, server }))
        }
        LibsqlTarget::Remote { url, .. } => Ok(Box::new(connect_remote(url, token, via).await?)),
    }
}

struct Local {
    inner: nd_drv_sqlite::SqliteConnection,
    server: ServerInfo,
}

#[async_trait::async_trait]
impl Connection for Local {
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
        self.inner.canceller()
    }

    async fn execute(&self, sql: &str, out: mpsc::Sender<Chunk>) -> Result<Outcome> {
        self.inner.execute(sql, out).await
    }

    async fn execute_bound(
        &self,
        sql: &str,
        params: &[Option<String>],
        out: mpsc::Sender<Chunk>,
    ) -> Result<Outcome> {
        self.inner.execute_bound(sql, params, out).await
    }

    async fn schema(&self) -> Result<Schema> {
        self.inner.schema().await
    }

    async fn ddl(&self, object: &nd_driver::ObjectRef) -> Result<String> {
        self.inner.ddl(object).await
    }

    async fn dependencies(
        &self,
        object: &nd_driver::ObjectRef,
    ) -> Result<Vec<nd_driver::Dependency>> {
        self.inner.dependencies(object).await
    }
}

pub struct RemoteConnection {
    conn: Mutex<libsql::Connection>,
    _db: libsql::Database,
    cancel: Arc<watch::Sender<u64>>,
    server: ServerInfo,
}

pub(crate) fn map_error(e: &libsql::Error) -> DriverError {
    match e {
        libsql::Error::SqliteFailure(code, message) => DriverError::Query {
            message: message.clone(),
            code: Some(code.to_string()),
            detail: None,
            hint: None,
            offset: None,
        },
        libsql::Error::Hrana(inner) => {
            let message = inner.to_string();
            DriverError::Query {
                message: message
                    .strip_prefix("SQLite error: ")
                    .unwrap_or(&message)
                    .to_owned(),
                code: None,
                detail: None,
                hint: None,
                offset: None,
            }
        }
        other => DriverError::query(other.to_string()),
    }
}

fn map_connect(e: &libsql::Error) -> DriverError {
    let message = e.to_string();
    if message.contains("401") || message.contains("Unauthorized") || message.contains("403") {
        DriverError::Auth { message }
    } else if message.contains("certificate") || message.contains("tls") {
        DriverError::Tls { message }
    } else {
        DriverError::Connect { message }
    }
}

#[derive(Clone, Copy)]
struct Via(SocketAddr);

impl hyper::service::Service<hyper::Uri> for Via {
    type Response = tokio::net::TcpStream;
    type Error = std::io::Error;
    type Future = Pin<Box<dyn Future<Output = std::io::Result<Self::Response>> + Send>>;

    fn poll_ready(&mut self, _: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, _: hyper::Uri) -> Self::Future {
        let addr = self.0;
        Box::pin(async move {
            let tcp = tokio::net::TcpStream::connect(addr).await?;
            tcp.set_nodelay(true)?;
            Ok(tcp)
        })
    }
}

pub async fn connect_remote(
    url: &str,
    token: Option<&str>,
    via: Option<SocketAddr>,
) -> Result<RemoteConnection> {
    let mut builder =
        libsql::Builder::new_remote(url.trim().to_owned(), token.unwrap_or_default().to_owned());
    if let Some(addr) = via {
        let https = hyper_rustls::HttpsConnectorBuilder::new()
            .with_native_roots()
            .map_err(|e| DriverError::Tls {
                message: e.to_string(),
            })?
            .https_or_http()
            .enable_http1()
            .wrap_connector(Via(addr));
        builder = builder.connector(https);
    }
    let db = builder.build().await.map_err(|e| map_connect(&e))?;
    let conn = db.connect().map_err(|e| map_connect(&e))?;
    let version = query_rows(&conn, "SELECT sqlite_version()", &[])
        .await
        .map_err(|e| map_connect(&e))?
        .first()
        .and_then(|r| r.first().cloned().flatten())
        .unwrap_or_default();
    let database = url
        .split("://")
        .nth(1)
        .unwrap_or(url)
        .split(['/', '?', '.'])
        .next()
        .unwrap_or_default()
        .to_owned();
    Ok(RemoteConnection {
        conn: Mutex::new(conn),
        _db: db,
        cancel: Arc::new(watch::channel(0).0),
        server: ServerInfo {
            product: "libSQL".into(),
            version,
            database,
        },
    })
}

fn values(params: &[Option<String>]) -> Vec<Value> {
    params
        .iter()
        .map(|p| p.clone().map_or(Value::Null, Value::Text))
        .collect()
}

pub(crate) fn text(v: Value) -> Option<String> {
    match v {
        Value::Null => None,
        Value::Integer(i) => Some(i.to_string()),
        Value::Real(f) => Some(f.to_string()),
        Value::Text(t) => Some(t),
        Value::Blob(b) => {
            let mut s = String::with_capacity(2 + b.len() * 2);
            s.push_str("\\x");
            for byte in b {
                let _ = write!(s, "{byte:02x}");
            }
            Some(s)
        }
    }
}

pub(crate) async fn query_rows(
    conn: &libsql::Connection,
    sql: &str,
    params: &[Option<String>],
) -> libsql::Result<Vec<Row>> {
    let mut rows = conn.query(sql, values(params)).await?;
    let n = rows.column_count();
    let mut out = Vec::new();
    while let Some(row) = rows.next().await? {
        out.push(
            (0..n)
                .map(|i| row.get_value(i).map(text))
                .collect::<libsql::Result<Row>>()?,
        );
    }
    Ok(out)
}

fn command(sql: &str) -> Option<String> {
    let mut rest = sql.trim_start();
    loop {
        rest = if rest.starts_with("--") {
            rest.split_once('\n').map_or("", |(_, r)| r)
        } else if let Some(r) = rest.strip_prefix("/*") {
            r.split_once("*/").map_or("", |(_, r)| r)
        } else {
            break;
        }
        .trim_start();
    }
    rest.split(|c: char| c.is_whitespace() || c == ';' || c == '(')
        .next()
        .filter(|w| !w.is_empty())
        .map(str::to_uppercase)
}

fn returns_rows(command: Option<&str>, sql: &str) -> bool {
    command.is_some_and(|c| QUERIES.contains(&c))
        || sql
            .to_ascii_lowercase()
            .split(|c: char| !c.is_ascii_alphanumeric() && c != '_')
            .any(|w| w == "returning")
}

async fn run(
    conn: &libsql::Connection,
    sql: &str,
    params: &[Option<String>],
    out: mpsc::Sender<Chunk>,
) -> Result<Outcome> {
    let command = command(sql);
    if !returns_rows(command.as_deref(), sql) {
        let n = conn
            .execute(sql, values(params))
            .await
            .map_err(|e| map_error(&e))?;
        return Ok(Outcome {
            command,
            rows_affected: Some(n),
        });
    }
    let mut rows = conn
        .query(sql, values(params))
        .await
        .map_err(|e| map_error(&e))?;
    let n = rows.column_count();
    let mut sink = RowBatcher::new(out);
    sink.columns(
        (0..n)
            .map(|i| Column {
                name: rows.column_name(i).unwrap_or_default().to_owned(),
                type_name: None,
            })
            .collect(),
    )
    .await?;
    let mut count = 0u64;
    while let Some(row) = rows.next().await.map_err(|e| map_error(&e))? {
        let cells = (0..n)
            .map(|i| row.get_value(i).map(text))
            .collect::<libsql::Result<Row>>()
            .map_err(|e| map_error(&e))?;
        count += 1;
        sink.push(cells).await?;
    }
    sink.flush().await?;
    let writes = !matches!(command.as_deref(), Some(c) if QUERIES.contains(&c));
    Ok(Outcome {
        rows_affected: writes.then_some(count),
        command,
    })
}

impl RemoteConnection {
    async fn run(
        &self,
        sql: &str,
        params: &[Option<String>],
        out: mpsc::Sender<Chunk>,
    ) -> Result<Outcome> {
        let conn = self.conn.lock().await;
        let mut stop = self.cancel.subscribe();
        tokio::select! {
            biased;
            r = run(&conn, sql, params, out) => r,
            _ = stop.changed() => Err(DriverError::Cancelled),
        }
    }
}

#[async_trait::async_trait]
impl Connection for RemoteConnection {
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
        Arc::new(Abandon(self.cancel.clone()))
    }

    async fn execute(&self, sql: &str, out: mpsc::Sender<Chunk>) -> Result<Outcome> {
        self.run(sql, &[], out).await
    }

    async fn execute_bound(
        &self,
        sql: &str,
        params: &[Option<String>],
        out: mpsc::Sender<Chunk>,
    ) -> Result<Outcome> {
        self.run(sql, params, out).await
    }

    async fn schema(&self) -> Result<Schema> {
        let conn = self.conn.lock().await;
        schema::load(&conn).await
    }

    async fn ddl(&self, object: &nd_driver::ObjectRef) -> Result<String> {
        use nd_drv_sqlite::ddl;
        let params = ddl::params(object)?.map(Some);
        let conn = self.conn.lock().await;
        let rows = query_rows(&conn, ddl::QUERY, &params)
            .await
            .map_err(|e| map_error(&e))?
            .into_iter()
            .map(|r| {
                let mut r = r.into_iter();
                let kind = r.next().flatten().unwrap_or_default();
                (kind, r.next().flatten().unwrap_or_default())
            })
            .collect();
        ddl::script(object, rows)
    }

    async fn dependencies(
        &self,
        object: &nd_driver::ObjectRef,
    ) -> Result<Vec<nd_driver::Dependency>> {
        let conn = self.conn.lock().await;
        let rows = query_rows(&conn, nd_drv_sqlite::ddl::DEPS, &[])
            .await
            .map_err(|e| map_error(&e))?;
        Ok(nd_driver::by_text(object, &rows))
    }
}

struct Abandon(Arc<watch::Sender<u64>>);

#[async_trait::async_trait]
impl Canceller for Abandon {
    async fn cancel(&self) {
        self.0.send_modify(|n| *n = n.wrapping_add(1));
    }
}
