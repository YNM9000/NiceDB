mod ddl;
mod deps;
mod error;
mod schema;
mod value;

use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;
use std::time::Duration;

use futures_util::TryStreamExt;
use nd_driver::{
    Canceller, Chunk, Column, Connection, Dialect, DriverError, MssqlTarget, Outcome, Result, Row,
    RowBatcher, Schema, ServerInfo, TlsMode,
};
use tiberius::{AuthMethod, Client, Config, EncryptionLevel, Query, QueryItem};
use tokio::net::TcpStream;
use tokio::sync::{Mutex, mpsc, watch};
use tokio_util::compat::{Compat, TokioAsyncWriteCompatExt};

pub use error::map_error;

type Tds = Client<Compat<TcpStream>>;

const WRITES: &[&str] = &["INSERT", "UPDATE", "DELETE", "MERGE"];
const SESSION: &str = "SET DATEFORMAT ymd; SET TEXTSIZE 2147483647";
const CATALOG_SESSION: &str =
    "SET TRANSACTION ISOLATION LEVEL READ UNCOMMITTED; SET LOCK_TIMEOUT 1000";
const LOCK_TIMEOUT: &str = "1222";
const SETTLE: Duration = Duration::from_millis(200);
const ACK: Duration = Duration::from_secs(5);

pub struct MssqlConnection {
    client: Mutex<Tds>,
    catalog: Mutex<Option<Tds>>,
    dial: Dial,
    cancel: Arc<watch::Sender<u64>>,
    server: ServerInfo,
}

#[derive(Clone)]
struct Dial {
    config: Config,
    addr: Addr,
    identity: Option<(String, Arc<nd_driver::Identity>)>,
}

#[derive(Clone)]
enum Addr {
    Host(String, u16),
    Ip(SocketAddr),
}

impl Dial {
    async fn tcp(addr: &Addr) -> Result<TcpStream> {
        let connect = async {
            match addr {
                Addr::Host(host, port) => TcpStream::connect((host.as_str(), *port)).await,
                Addr::Ip(a) => TcpStream::connect(a).await,
            }
        };
        let tcp = tokio::time::timeout(Duration::from_secs(10), connect)
            .await
            .map_err(|_| DriverError::Connect {
                message: "connection timed out".into(),
            })?
            .map_err(|e| DriverError::Connect {
                message: e.to_string(),
            })?;
        tcp.set_nodelay(true).map_err(|e| DriverError::Io {
            message: e.to_string(),
        })?;
        Ok(tcp)
    }

    async fn open(&self) -> Result<Tds> {
        let mut config = self.config.clone();
        let _key = match &self.identity {
            Some((cert, identity)) => {
                let file = nd_driver::KeyFile::of(identity)?;
                config.client_certificate(cert, file.path());
                Some(file)
            }
            None => None,
        };
        let tcp = Self::tcp(&self.addr).await?;
        let mut client = match Box::pin(Client::connect(config.clone(), tcp.compat_write())).await {
            Ok(c) => c,
            Err(tiberius::error::Error::Routing { host, port }) => {
                config.host(&host);
                config.port(port);
                let tcp = Self::tcp(&Addr::Host(host, port)).await?;
                Box::pin(Client::connect(config, tcp.compat_write()))
                    .await
                    .map_err(|e| error::map_connect(&e))?
            }
            Err(e) => return Err(error::map_connect(&e)),
        };
        client
            .simple_query(SESSION)
            .await
            .map_err(|e| error::map_connect(&e))?
            .into_results()
            .await
            .map_err(|e| error::map_connect(&e))?;
        Ok(client)
    }
}

pub async fn connect(
    target: &MssqlTarget,
    (password, key_passphrase): (Option<&str>, Option<&str>),
    via: Option<(IpAddr, u16)>,
) -> Result<MssqlConnection> {
    let mut config = Config::new();
    config.host(&target.host);
    config.port(target.port);
    if !target.database.is_empty() {
        config.database(&target.database);
    }
    config.application_name("NiceDB");
    config.authentication(AuthMethod::sql_server(
        &target.user,
        password.unwrap_or_default(),
    ));
    match (target.tls, target.ca_file.as_deref()) {
        (TlsMode::Disable, _) => {
            config.encryption(EncryptionLevel::Off);
            config.trust_cert();
        }
        (TlsMode::Insecure, _) => {
            config.encryption(EncryptionLevel::Required);
            config.trust_cert();
        }
        (TlsMode::Verify, ca) => {
            config.encryption(EncryptionLevel::Required);
            if let Some(ca) = ca {
                config.trust_cert_ca(ca);
            }
        }
    }
    let mut identity = None;
    if let Some(cert) = &target.client_cert
        && target.tls != TlsMode::Disable
    {
        let loaded = nd_driver::load_identity(cert, key_passphrase)?;
        if nd_driver::key_encrypted(cert)? {
            identity = Some((cert.cert.clone(), Arc::new(loaded)));
        } else {
            config.client_certificate(&cert.cert, &cert.key);
        }
    }
    let addr = match via {
        Some((ip, port)) => Addr::Ip(SocketAddr::new(ip, port)),
        None => Addr::Host(target.host.clone(), target.port),
    };
    let dial = Dial {
        config,
        addr,
        identity,
    };
    let mut client = Box::pin(dial.open()).await?;
    let rows = collect(
        &mut client,
        "SELECT CAST(SERVERPROPERTY('ProductVersion') AS nvarchar(128)), DB_NAME()",
    )
    .await
    .map_err(|e| error::map_connect(&e))?;
    let cell = |i: usize| rows.first().and_then(|r| r.get(i).cloned().flatten());
    let raw = cell(0).unwrap_or_default();
    let version = match raw.split('.').next() {
        Some("15") => format!("2019 ({raw})"),
        Some("16") => format!("2022 ({raw})"),
        Some("17") => format!("2025 ({raw})"),
        _ => raw,
    };
    Ok(MssqlConnection {
        client: Mutex::new(client),
        catalog: Mutex::new(None),
        dial,
        cancel: Arc::new(watch::channel(0).0),
        server: ServerInfo {
            product: "SQL Server".into(),
            version,
            database: cell(1).unwrap_or_default(),
        },
    })
}

pub(crate) async fn collect(
    client: &mut Tds,
    sql: &str,
) -> std::result::Result<Vec<Row>, tiberius::error::Error> {
    let mut rows = Vec::new();
    let mut stream = client.simple_query(sql).await?;
    while let Some(item) = stream.try_next().await? {
        if let QueryItem::Row(row) = item {
            rows.push(row.into_iter().map(value::text).collect());
        }
    }
    Ok(rows)
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
    rest.split(|c: char| c.is_whitespace() || c == '(' || c == ';')
        .next()
        .filter(|w| !w.is_empty())
        .map(str::to_uppercase)
}

enum Stop {
    Interrupted,
    Failed(DriverError),
}

async fn stream(
    client: &mut Tds,
    sql: &str,
    params: Option<&[Option<String>]>,
    out: mpsc::Sender<Chunk>,
    stop: &mut watch::Receiver<u64>,
) -> std::result::Result<(), Stop> {
    let err = |e: tiberius::error::Error| Stop::Failed(error::map_error(&e, sql));
    let gone = |_| Stop::Interrupted;
    let open = async {
        match params {
            None => client.simple_query(sql).await,
            Some(p) => {
                let mut q = Query::new(sql);
                for v in p {
                    q.bind(v.as_deref());
                }
                q.query(client).await
            }
        }
    };
    let mut results = tokio::select! {
        biased;
        r = open => r.map_err(err)?,
        _ = stop.changed() => return Err(Stop::Interrupted),
    };
    let mut sink = RowBatcher::new(out);
    let mut kinds = Vec::new();
    loop {
        if stop.has_changed().unwrap_or(false) {
            return Err(Stop::Interrupted);
        }
        let item = {
            let next = results.try_next();
            tokio::pin!(next);
            tokio::select! {
                biased;
                r = &mut next => r,
                _ = stop.changed() => {
                    let _ = tokio::time::timeout(SETTLE, &mut next).await;
                    return Err(Stop::Interrupted);
                }
            }
        };
        match item.map_err(err)? {
            None => break,
            Some(QueryItem::Metadata(meta)) => {
                sink.flush().await.map_err(gone)?;
                kinds = meta
                    .columns()
                    .iter()
                    .map(tiberius::Column::column_type)
                    .collect();
                let columns = meta
                    .columns()
                    .iter()
                    .map(|c| Column {
                        name: c.name().to_owned(),
                        type_name: Some(value::type_name(c.column_type()).to_owned()),
                    })
                    .collect();
                sink.columns(columns).await.map_err(gone)?;
            }
            Some(QueryItem::Row(row)) => {
                let cells = row
                    .into_iter()
                    .enumerate()
                    .map(|(i, v)| value::typed_text(v, kinds.get(i).copied()))
                    .collect();
                sink.push(cells).await.map_err(gone)?;
            }
        }
    }
    sink.flush().await.map_err(gone)
}

macro_rules! on_catalog {
    ($conn:expr, |$c:ident| $body:expr) => {{
        let mut slot = $conn.catalog.lock().await;
        let mut tries = 0;
        loop {
            tries += 1;
            if slot.is_none() {
                *slot = Some($conn.open_catalog().await?);
            }
            let Some($c) = slot.as_mut() else {
                continue;
            };
            match $body.await {
                Err(DriverError::Connect { .. } | DriverError::Io { .. }) if tries < 2 => {
                    *slot = None;
                }
                Err(DriverError::Query {
                    code: Some(code), ..
                }) if code == LOCK_TIMEOUT => {
                    drop(slot);
                    let mut main = $conn.client.lock().await;
                    let $c = &mut *main;
                    break $body.await;
                }
                out => break out,
            }
        }
    }};
}

impl MssqlConnection {
    async fn open_catalog(&self) -> Result<Tds> {
        let mut client = Box::pin(self.dial.open()).await?;
        collect(&mut client, CATALOG_SESSION)
            .await
            .map_err(|e| error::map_connect(&e))?;
        Ok(client)
    }

    async fn run(
        &self,
        sql: &str,
        params: Option<&[Option<String>]>,
        out: mpsc::Sender<Chunk>,
    ) -> Result<Outcome> {
        let mut client = self.client.lock().await;
        let mut stop = self.cancel.subscribe();
        match Box::pin(stream(&mut client, sql, params, out, &mut stop)).await {
            Ok(()) => {}
            Err(Stop::Failed(e)) => return Err(e),
            Err(Stop::Interrupted) => {
                self.recover(&mut client).await;
                return Err(DriverError::Cancelled);
            }
        }
        let command = command(sql);
        let writes = command.as_deref().is_some_and(|c| WRITES.contains(&c));
        let rows_affected = if writes {
            collect(&mut client, "SELECT CAST(@@ROWCOUNT AS bigint)")
                .await
                .map_err(|e| error::map_error(&e, sql))?
                .first()
                .and_then(|r| r.first().cloned().flatten())
                .and_then(|n| n.parse().ok())
        } else {
            None
        };
        Ok(Outcome {
            command,
            rows_affected,
        })
    }

    async fn recover(&self, client: &mut Tds) {
        match tokio::time::timeout(ACK, client.cancel_query()).await {
            Ok(Ok(())) => return,
            Ok(Err(e)) => tracing::warn!(error = %e, "sql server cancel failed"),
            Err(_) => tracing::warn!("sql server did not acknowledge the cancel"),
        }
        match Box::pin(self.dial.open()).await {
            Ok(fresh) => *client = fresh,
            Err(e) => tracing::warn!(error = %e, "sql server reconnect failed"),
        }
    }
}

#[async_trait::async_trait]
impl Connection for MssqlConnection {
    fn capabilities(&self) -> nd_driver::Capabilities {
        nd_driver::Capabilities::relational(Dialect::Mssql)
    }

    fn server(&self) -> &ServerInfo {
        &self.server
    }

    fn canceller(&self) -> Arc<dyn Canceller> {
        Arc::new(Attention(self.cancel.clone()))
    }

    async fn execute(&self, sql: &str, out: mpsc::Sender<Chunk>) -> Result<Outcome> {
        Box::pin(self.run(sql, None, out)).await
    }

    async fn execute_bound(
        &self,
        sql: &str,
        params: &[Option<String>],
        out: mpsc::Sender<Chunk>,
    ) -> Result<Outcome> {
        Box::pin(self.run(sql, Some(params), out)).await
    }

    async fn schema(&self) -> Result<Schema> {
        on_catalog!(self, |c| schema::load(c, None))
    }

    async fn outline(&self) -> Result<Option<nd_driver::Outline>> {
        on_catalog!(self, |c| schema::outline(c)).map(Some)
    }

    async fn schema_of(&self, namespaces: &[String]) -> Result<Schema> {
        on_catalog!(self, |c| schema::load(c, Some(namespaces)))
    }

    async fn relation(&self, namespace: &str, name: &str) -> Result<Option<nd_driver::Relation>> {
        on_catalog!(self, |c| schema::relation(c, namespace, name))
    }

    async fn ddl(&self, object: &nd_driver::ObjectRef) -> Result<String> {
        on_catalog!(self, |c| ddl::ddl(c, object))
    }

    async fn dependencies(
        &self,
        object: &nd_driver::ObjectRef,
    ) -> Result<Vec<nd_driver::Dependency>> {
        on_catalog!(self, |c| deps::dependencies(c, object))
    }
}

struct Attention(Arc<watch::Sender<u64>>);

#[async_trait::async_trait]
impl Canceller for Attention {
    async fn cancel(&self) {
        self.0.send_modify(|n| *n = n.wrapping_add(1));
    }
}
