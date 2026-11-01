#![forbid(unsafe_code)]

mod format;
mod http;
mod schema;
mod tls;

pub use schema::key_parts;

use std::net::IpAddr;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use nd_driver::{
    Canceller, Capabilities, Chunk, ClickhouseTarget, Column, Dependency, Dialect, DriverError,
    Group, NamespaceLevel, ObjectRef, Outcome, QueryProgress, Result, RowBatcher, Schema,
    ServerInfo, TlsMode, Tool,
};
use tokio::sync::{Notify, mpsc};

use crate::http::{Http, Lines, Reply};

const FORMAT: &str = "JSONCompactEachRowWithNamesAndTypes";
const SESSION_TIMEOUT: &str = "1800";
const PROGRESS_FIRST: Duration = Duration::from_millis(300);
const PROGRESS_EVERY: Duration = Duration::from_millis(500);

pub struct ClickhouseConnection {
    http: Arc<Http>,
    database: String,
    session: String,
    server: ServerInfo,
    cancel: Arc<Cancel>,
}

#[derive(Default)]
struct Cancel {
    running: Mutex<Option<String>>,
    stop: Notify,
    http: Mutex<Option<Arc<Http>>>,
}

struct ClickhouseCanceller(Arc<Cancel>);

#[async_trait::async_trait]
impl Canceller for ClickhouseCanceller {
    async fn cancel(&self) {
        let id = self.0.running.lock().ok().and_then(|r| r.clone());
        let http = self.0.http.lock().ok().and_then(|h| h.clone());
        self.0.stop.notify_waiters();
        if let (Some(id), Some(http)) = (id, http) {
            let sql = format!(
                "KILL QUERY WHERE query_id = {} ASYNC",
                nd_sql::literal(Dialect::Clickhouse, Some(&id))
            );
            if let Ok(reply) = http.post_control(&sql).await {
                drop(reply);
            }
        }
    }
}

pub async fn connect(
    target: &ClickhouseTarget,
    password: Option<&str>,
    key_passphrase: Option<&str>,
    via: Option<(IpAddr, u16)>,
) -> Result<ClickhouseConnection> {
    let identity = match (&target.client_cert, target.tls) {
        (Some(c), mode) if mode != TlsMode::Disable => {
            Some(nd_driver::load_identity(c, key_passphrase)?)
        }
        _ => None,
    };
    let cert_auth = identity.is_some() && password.is_none_or(str::is_empty);
    let tls = tls::config(target.tls, target.ca_file.as_deref(), identity)?;
    let http = Arc::new(
        Http::new(&target.host, target.port, via, tls, &target.user, password)
            .certificate_auth(cert_auth),
    );
    let database = if target.database.is_empty() {
        "default".to_owned()
    } else {
        target.database.clone()
    };
    let cancel = Arc::new(Cancel::default());
    if let Ok(mut h) = cancel.http.lock() {
        *h = Some(http.clone());
    }
    let mut conn = ClickhouseConnection {
        http,
        session: uuid::Uuid::new_v4().to_string(),
        server: ServerInfo {
            product: "ClickHouse".into(),
            version: String::new(),
            database: database.clone(),
        },
        database,
        cancel,
    };
    let found = conn
        .rows("SELECT version(), currentDatabase()", true)
        .await?;
    let first = found.into_iter().next().unwrap_or_default();
    let text = |i: usize| first.get(i).cloned().flatten().unwrap_or_default();
    conn.server.version = text(0);
    conn.server.database = text(1);
    Ok(conn)
}

pub fn capabilities() -> Capabilities {
    let mut caps = Capabilities::relational(Dialect::Clickhouse)
        .groups(&[
            Group::Tables,
            Group::Views,
            Group::MaterializedViews,
            Group::Dictionaries,
            Group::Functions,
        ])
        .namespaces(NamespaceLevel::Multiple);
    caps.transactions = false;
    caps.editable = false;
    caps.er = false;
    caps.params = true;
    caps.tools = vec![
        Tool::Activity,
        Tool::Monitor,
        Tool::Space,
        Tool::Users,
        Tool::Compare,
        Tool::Rows,
        Tool::Backup,
        Tool::Transfer,
        Tool::Seclab,
        Tool::Search,
        Tool::Generate,
    ];
    caps
}

impl ClickhouseConnection {
    fn params<'a>(&'a self, query_id: &'a str, session: bool) -> Vec<(&'a str, &'a str)> {
        let mut p = vec![
            ("database", self.database.as_str()),
            ("query_id", query_id),
            ("default_format", FORMAT),
            ("output_format_json_quote_decimals", "1"),
            ("output_format_json_quote_64bit_integers", "1"),
            ("output_format_decimal_trailing_zeros", "1"),
        ];
        if session {
            p.push(("session_id", self.session.as_str()));
            p.push(("session_timeout", SESSION_TIMEOUT));
        }
        p
    }

    pub(crate) async fn rows(&self, sql: &str, session: bool) -> Result<Vec<nd_driver::Row>> {
        let id = uuid::Uuid::new_v4().to_string();
        let reply = self.http.post(&self.params(&id, session), sql).await?;
        let mut lines = Lines::new(reply.body);
        let mut out = Vec::new();
        let mut header = 0;
        while let Some(line) = lines.next().await? {
            if header < 2 {
                header += 1;
                continue;
            }
            out.push(row_or_error(&line)?);
        }
        Ok(out)
    }

    async fn run(&self, sql: &str, out: mpsc::Sender<Chunk>) -> Result<Outcome> {
        let id = uuid::Uuid::new_v4().to_string();
        if let Ok(mut r) = self.cancel.running.lock() {
            r.clone_from(&Some(id.clone()));
        }
        let progress = tokio::spawn(watch(self.http.clone(), id.clone(), out.clone()));
        let stopped = self.cancel.stop.notified();
        let work = self.stream(sql, &id, out);
        let done = tokio::select! {
            r = work => r,
            () = stopped => Err(DriverError::Cancelled),
        };
        progress.abort();
        if let Ok(mut r) = self.cancel.running.lock() {
            *r = None;
        }
        done
    }

    async fn stream(&self, sql: &str, id: &str, out: mpsc::Sender<Chunk>) -> Result<Outcome> {
        let Reply { headers, body } = self.http.post(&self.params(id, true), sql).await?;
        let mut lines = Lines::new(body);
        let header = |name: &str| headers.get(name).and_then(|v| v.to_str().ok());
        let format = header("X-ClickHouse-Format").map(str::to_owned);
        let summary = header("X-ClickHouse-Summary").map(str::to_owned);
        let mut batch = RowBatcher::new(out);
        let Some(format) = format else {
            while lines.next().await?.is_some() {}
            let written = summary
                .as_deref()
                .and_then(|s| format::summary(s, "written_rows"))
                .filter(|n| *n > 0);
            return Ok(Outcome {
                command: verb(sql),
                rows_affected: written,
            });
        };
        let mut count = 0u64;
        if format == FORMAT {
            let names = match lines.next().await? {
                Some(l) => format::texts(&l).ok_or_else(|| line_error(&l))?,
                None => Vec::new(),
            };
            let types = match lines.next().await? {
                Some(l) => format::texts(&l).ok_or_else(|| line_error(&l))?,
                None => Vec::new(),
            };
            let columns = names
                .into_iter()
                .zip(types.into_iter().map(Some).chain(std::iter::repeat(None)))
                .map(|(name, type_name)| Column { name, type_name })
                .collect();
            batch.columns(columns).await?;
            while let Some(line) = lines.next().await? {
                batch.push(row_or_error(&line)?).await?;
                count += 1;
            }
        } else {
            batch
                .columns(vec![Column {
                    name: format,
                    type_name: None,
                }])
                .await?;
            while let Some(line) = lines.next().await? {
                if let Some(e) = format::server_error(&line).filter(|_| line.starts_with("Code: "))
                {
                    return Err(e);
                }
                batch.push(vec![Some(line)]).await?;
                count += 1;
            }
        }
        batch.flush().await?;
        tracing::debug!(rows = count, "clickhouse result read");
        Ok(Outcome::default())
    }
}

fn line_error(line: &str) -> DriverError {
    format::server_error(line).unwrap_or_else(|| DriverError::query(line.trim().to_owned()))
}

fn row_or_error(line: &str) -> Result<nd_driver::Row> {
    format::row(line).ok_or_else(|| line_error(line))
}

fn verb(sql: &str) -> Option<String> {
    let mut rest = sql;
    loop {
        rest = rest.trim_start();
        if rest.starts_with("--") || rest.starts_with('#') {
            rest = rest.split_once('\n').map_or("", |(_, r)| r);
        } else if let Some(r) = rest.strip_prefix("/*") {
            rest = r.split_once("*/").map_or("", |(_, r)| r);
        } else {
            break;
        }
    }
    let word: String = rest.chars().take_while(char::is_ascii_alphabetic).collect();
    (!word.is_empty()).then(|| word.to_ascii_uppercase())
}

async fn watch(http: Arc<Http>, id: String, out: mpsc::Sender<Chunk>) {
    tokio::time::sleep(PROGRESS_FIRST).await;
    let sql = format!(
        "SELECT read_rows, read_bytes, total_rows_approx FROM system.processes WHERE query_id = {} FORMAT TSV",
        nd_sql::literal(Dialect::Clickhouse, Some(&id))
    );
    loop {
        if let Ok(reply) = http.post(&[], &sql).await {
            let mut lines = Lines::new(reply.body);
            if let Ok(Some(line)) = lines.next().await {
                let mut f = line.split('\t').map(|v| v.parse::<u64>().ok());
                if let (Some(Some(read_rows)), Some(Some(read_bytes)), Some(total)) =
                    (f.next(), f.next(), f.next())
                {
                    let p = QueryProgress {
                        read_rows,
                        read_bytes,
                        total_rows: total.filter(|t| *t > 0),
                    };
                    let _ = out.try_send(Chunk::Progress(p));
                }
            }
        }
        tokio::time::sleep(PROGRESS_EVERY).await;
    }
}

fn inline(sql: &str, params: &[Option<String>]) -> Result<String> {
    let marks = nd_sql::placeholders(sql, Dialect::Clickhouse);
    let mut out = String::with_capacity(sql.len() + params.len() * 8);
    let mut at = 0;
    let mut values = params.iter();
    for m in marks.iter().filter(|m| &sql[m.range.clone()] == "?") {
        let Some(v) = values.next() else {
            return Err(DriverError::query("more placeholders than values"));
        };
        out.push_str(&sql[at..m.range.start]);
        out.push_str(&nd_sql::literal(Dialect::Clickhouse, v.as_deref()));
        at = m.range.end;
    }
    out.push_str(&sql[at..]);
    Ok(out)
}

#[async_trait::async_trait]
impl nd_driver::Connection for ClickhouseConnection {
    fn capabilities(&self) -> Capabilities {
        capabilities()
    }

    fn server(&self) -> &ServerInfo {
        &self.server
    }

    fn canceller(&self) -> Arc<dyn Canceller> {
        Arc::new(ClickhouseCanceller(self.cancel.clone()))
    }

    async fn execute(&self, sql: &str, out: mpsc::Sender<Chunk>) -> Result<Outcome> {
        self.run(sql, out).await
    }

    async fn execute_bound(
        &self,
        sql: &str,
        params: &[Option<String>],
        out: mpsc::Sender<Chunk>,
    ) -> Result<Outcome> {
        let sql = inline(sql, params)?;
        self.run(&sql, out).await
    }

    async fn ping(&self) -> Result<()> {
        self.rows("SELECT 1", false).await.map(|_| ())
    }

    async fn set_read_only(&self, on: bool) -> Result<bool> {
        self.http.set_read_only(on);
        if on && self.rows("SELECT 1", false).await.is_err() {
            self.http.set_read_only(false);
            return Ok(false);
        }
        Ok(true)
    }

    async fn schema(&self) -> Result<Schema> {
        schema::load(self).await
    }

    async fn ddl(&self, object: &ObjectRef) -> Result<String> {
        schema::ddl(self, object).await
    }

    async fn dependencies(&self, object: &ObjectRef) -> Result<Vec<Dependency>> {
        schema::dependencies(self, object).await
    }
}
