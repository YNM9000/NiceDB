#![forbid(unsafe_code)]

mod cli;
mod cluster;
mod keys;
mod reply;
mod report;

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use nd_driver::{
    Canceller, Capabilities, Chunk, ColumnInfo, CommandClass, Commands, Dependency, DriverError,
    Family, Group, KeyPage, KeyRead, KeyScan, KeyValue, KeyWrite, Language, Namespace,
    NamespaceLevel, Noun, ObjectRef, Outcome, RedisMode, RedisTarget, Relation, Report, Result,
    ResultShape, RowBatcher, Schema, SchemaKind, ServerInfo, TlsMode, Tool, TreeShape, Vocabulary,
};
use redis::aio::MultiplexedConnection;
use redis::{
    AsyncConnectionConfig, Client, ConnectionAddr, IntoConnectionInfo, ProtocolVersion,
    RedisConnectionInfo, TlsCertificates, Value,
};
use tokio::sync::{Notify, mpsc};
use zeroize::Zeroizing;

pub use cli::{classify, parse, split};

const SCHEMA_SAMPLE: usize = 20_000;
const SCHEMA_PREFIXES: usize = 1000;
type Tally = (u64, HashMap<String, u64>);

pub const SEPARATOR: char = ':';

pub struct RedisConnection {
    conn: MultiplexedConnection,
    cluster: Option<Arc<cluster::Cluster>>,
    sentinel: Option<Arc<SentinelLink>>,
    db: u16,
    server: ServerInfo,
    modules: Vec<String>,
    cancel: Arc<Cancel>,
}

struct Cancel {
    stop: Notify,
    client: Client,
    config: AsyncConnectionConfig,
    id: Option<i64>,
    running: Mutex<bool>,
}

struct RedisCanceller(Arc<Cancel>);

#[async_trait::async_trait]
impl Canceller for RedisCanceller {
    async fn cancel(&self) {
        let running = self.0.running.lock().is_ok_and(|r| *r);
        self.0.stop.notify_waiters();
        let (true, Some(id)) = (running, self.0.id) else {
            return;
        };
        if let Ok(mut side) = self
            .0
            .client
            .get_multiplexed_async_connection_with_config(&self.0.config)
            .await
        {
            let _ = redis::cmd("CLIENT")
                .arg("UNBLOCK")
                .arg(id)
                .arg("ERROR")
                .query_async::<Value>(&mut side)
                .await;
        }
    }
}

pub(crate) fn err(e: &redis::RedisError) -> DriverError {
    use redis::ErrorKind;
    let message = e.detail().map_or_else(|| e.to_string(), str::to_owned);
    let code = e.code().map(str::to_owned);
    if matches!(code.as_deref(), Some("WRONGPASS" | "NOAUTH"))
        || e.kind() == ErrorKind::AuthenticationFailed
    {
        return DriverError::Auth {
            message: e.to_string(),
        };
    }
    if e.is_connection_refusal() {
        return DriverError::Connect {
            message: e.to_string(),
        };
    }
    if e.is_io_error() || e.is_connection_dropped() || e.is_timeout() {
        let lower = e.to_string().to_ascii_lowercase();
        if lower.contains("certificate") || lower.contains("tls") {
            return DriverError::Tls {
                message: e.to_string(),
            };
        }
        return DriverError::Io {
            message: e.to_string(),
        };
    }
    match e.kind() {
        ErrorKind::Server(_) | ErrorKind::Extension => DriverError::Query {
            message,
            code,
            detail: None,
            hint: None,
            offset: None,
        },
        _ => DriverError::query(e.to_string()),
    }
}

pub(crate) fn rerr(e: redis::RedisError) -> DriverError {
    err(&e)
}

fn tls_error(message: String) -> DriverError {
    DriverError::Tls { message }
}

fn product(info: &str) -> (String, String) {
    let fields: HashMap<&str, &str> = info
        .lines()
        .filter_map(|l| l.trim().split_once(':'))
        .collect();
    if let Some(v) = fields.get("valkey_version") {
        return ("Valkey".into(), (*v).to_owned());
    }
    let version = fields.get("redis_version").copied().unwrap_or_default();
    if fields
        .get("server_name")
        .is_some_and(|n| n.eq_ignore_ascii_case("valkey"))
    {
        return ("Valkey".into(), version.to_owned());
    }
    ("Redis".into(), version.to_owned())
}

#[derive(Clone)]
struct Dialer {
    tls: TlsMode,
    root: Option<Vec<u8>>,
    identity: Option<(Vec<u8>, Vec<u8>)>,
    user: String,
    password: Option<Zeroizing<String>>,
    db: u16,
    protocol: ProtocolVersion,
    config: AsyncConnectionConfig,
}

impl Dialer {
    fn new(t: &RedisTarget, password: Option<&str>, key_passphrase: Option<&str>) -> Result<Self> {
        let secure = t.tls != TlsMode::Disable;
        let root = match t.ca_file.as_deref().filter(|f| !f.is_empty() && secure) {
            Some(path) => Some(std::fs::read(path).map_err(|e| tls_error(format!("{path}: {e}")))?),
            None => None,
        };
        let identity = match t.client_cert.as_ref().filter(|_| secure) {
            Some(c) => {
                let id = nd_driver::load_identity(c, key_passphrase)?;
                Some((id.certs_pem(), id.key_pem()))
            }
            None => None,
        };
        Ok(Self {
            tls: t.tls,
            root,
            identity,
            user: t.user.clone(),
            password: password
                .filter(|p| !p.is_empty())
                .map(|p| Zeroizing::new(p.to_owned())),
            db: if t.mode == RedisMode::Cluster {
                0
            } else {
                t.database
            },
            protocol: ProtocolVersion::RESP3,
            config: AsyncConnectionConfig::new()
                .set_connection_timeout(Some(Duration::from_secs(10)))
                .set_response_timeout(None),
        })
    }

    fn open(&self, host: &str, port: u16, auth: bool) -> Result<Client> {
        let addr = match self.tls {
            TlsMode::Disable => ConnectionAddr::Tcp(host.to_owned(), port),
            mode => ConnectionAddr::TcpTls {
                host: host.to_owned(),
                port,
                insecure: mode == TlsMode::Insecure,
                tls_params: None,
            },
        };
        let mut settings = RedisConnectionInfo::default()
            .set_db(i64::from(self.db))
            .set_protocol(self.protocol);
        if auth {
            if !self.user.is_empty() {
                settings = settings.set_username(&self.user);
            }
            if let Some(p) = &self.password {
                settings = settings.set_password(p.as_str());
            }
        }
        let info = addr
            .into_connection_info()
            .map_err(rerr)?
            .set_redis_settings(settings);
        if self.tls == TlsMode::Disable || (self.root.is_none() && self.identity.is_none()) {
            return Client::open(info).map_err(rerr);
        }
        Client::build_with_tls(
            info,
            TlsCertificates {
                client_tls: self
                    .identity
                    .as_ref()
                    .map(|(cert, key)| redis::ClientTlsConfig {
                        client_cert: cert.clone(),
                        client_key: key.clone(),
                    }),
                root_cert: self.root.clone(),
            },
        )
        .map_err(rerr)
    }

    async fn dial(&mut self, host: &str, port: u16) -> Result<(Client, MultiplexedConnection)> {
        let mut last = None;
        for protocol in [ProtocolVersion::RESP3, ProtocolVersion::RESP2] {
            self.protocol = protocol;
            let client = self.open(host, port, true)?;
            match client
                .get_multiplexed_async_connection_with_config(&self.config)
                .await
            {
                Ok(conn) => return Ok((client, conn)),
                Err(e) if protocol == ProtocolVersion::RESP3 && resp3_refused(&e) => {
                    last = Some(e);
                }
                Err(e) => return Err(err(&e)),
            }
        }
        Err(last.map_or_else(|| DriverError::query("no connection"), |e| err(&e)))
    }
}

impl cluster::Opener for Dialer {
    fn client(&self, host: &str, port: u16) -> Result<Client> {
        self.open(host, port, true)
    }

    fn config(&self) -> &AsyncConnectionConfig {
        &self.config
    }
}

fn sentinel_addr(text: &str, port: u16) -> Option<cluster::Addr> {
    let text = text.trim();
    if text.is_empty() {
        return None;
    }
    match text.rsplit_once(':') {
        Some((h, p)) if !h.contains(':') || h.ends_with(']') => {
            Some((h.trim_matches(['[', ']']).to_owned(), p.parse().ok()?))
        }
        _ => Some((text.trim_matches(['[', ']']).to_owned(), port)),
    }
}

pub(crate) fn master_addr(v: &Value) -> Option<cluster::Addr> {
    let parts = reply::list(v);
    let host = parts.first().and_then(reply::scalar)?;
    let port = parts.get(1).and_then(reply::scalar)?.parse().ok()?;
    Some((host, port))
}

pub(crate) struct SentinelLink {
    target: RedisTarget,
    dialer: Dialer,
    pub(crate) name: String,
    pub(crate) at: cluster::Addr,
}

impl SentinelLink {
    pub(crate) async fn ask(&self, cmd: &redis::Cmd) -> Result<(Value, cluster::Addr)> {
        ask_sentinels(&self.target, &self.dialer, cmd).await
    }
}

async fn ask_sentinels(
    t: &RedisTarget,
    dialer: &Dialer,
    cmd: &redis::Cmd,
) -> Result<(Value, cluster::Addr)> {
    let mut sentinel = dialer.clone();
    sentinel.db = 0;
    sentinel.protocol = ProtocolVersion::RESP2;
    let mut last = DriverError::Connect {
        message: "no Sentinel given".into(),
    };
    for (host, port) in t.host.split(',').filter_map(|h| sentinel_addr(h, t.port)) {
        for auth in [false, true] {
            let asked = async {
                let client = sentinel.open(&host, port, auth)?;
                let mut conn = client
                    .get_multiplexed_async_connection_with_config(&sentinel.config)
                    .await
                    .map_err(rerr)?;
                cmd.query_async::<Value>(&mut conn).await.map_err(rerr)
            };
            match asked.await {
                Ok(reply) => return Ok((reply, (host, port))),
                Err(e @ DriverError::Auth { .. }) if !auth => last = e,
                Err(e) => {
                    last = e;
                    break;
                }
            }
        }
    }
    Err(last)
}

async fn discover(t: &RedisTarget, name: &str, dialer: &Dialer) -> Result<cluster::Addr> {
    let mut cmd = redis::cmd("SENTINEL");
    cmd.arg("GET-MASTER-ADDR-BY-NAME").arg(name);
    let (reply, (host, port)) = ask_sentinels(t, dialer, &cmd).await?;
    master_addr(&reply).ok_or_else(|| DriverError::Connect {
        message: format!("the Sentinel at {host}:{port} knows no master named {name}"),
    })
}

fn demoted(name: &str) -> DriverError {
    DriverError::Io {
        message: format!("the Sentinels moved master {name}: this server is a replica now"),
    }
}

pub async fn connect(
    t: &RedisTarget,
    password: Option<&str>,
    key_passphrase: Option<&str>,
    via: Option<(std::net::IpAddr, u16)>,
) -> Result<RedisConnection> {
    if via.is_some() && t.mode != RedisMode::Single {
        return Err(DriverError::Connect {
            message: "Cluster and Sentinel reach every node directly: turn off the SSH tunnel and the proxy".into(),
        });
    }
    let mut dialer = Dialer::new(t, password, key_passphrase)?;
    let (host, port) = match (&t.mode, via) {
        (_, Some((ip, port))) => (ip.to_string(), port),
        (RedisMode::Sentinel { master }, None) => discover(t, master.trim(), &dialer).await?,
        (_, None) => (t.host.clone(), t.port),
    };
    let (client, mut conn) = dialer.dial(&host, port).await?;
    let cluster = if t.mode == RedisMode::Cluster {
        let slots = redis::cmd("CLUSTER")
            .arg("SLOTS")
            .query_async::<Value>(&mut conn)
            .await
            .map_err(|e| DriverError::Connect {
                message: format!("{host}:{port} is not a cluster node: {}", err(&e)),
            })?;
        let map =
            cluster::SlotMap::from_slots(&slots, &host).ok_or_else(|| DriverError::Connect {
                message: format!("{host}:{port} has no slots assigned yet"),
            })?;
        Some(Arc::new(cluster::Cluster::new(
            (host.clone(), port),
            map,
            Box::new(dialer.clone()),
        )))
    } else {
        None
    };
    let mut c = finish(t, client, dialer.config.clone(), conn).await?;
    c.cluster = cluster;
    if let RedisMode::Sentinel { master } = &t.mode {
        c.server.database = format!(
            "{} · {} {host}:{port}",
            RedisTarget::name(t.database),
            master.trim()
        );
        c.sentinel = Some(Arc::new(SentinelLink {
            target: t.clone(),
            dialer,
            name: master.trim().to_owned(),
            at: (host, port),
        }));
    }
    Ok(c)
}

fn resp3_refused(e: &redis::RedisError) -> bool {
    e.kind() == redis::ErrorKind::RESP3NotSupported
        || e.to_string().to_ascii_uppercase().contains("HELLO")
}

async fn finish(
    t: &RedisTarget,
    client: Client,
    config: AsyncConnectionConfig,
    mut conn: MultiplexedConnection,
) -> Result<RedisConnection> {
    let info: String = redis::cmd("INFO")
        .arg("server")
        .query_async(&mut conn)
        .await
        .map_err(rerr)?;
    let (product, version) = product(&info);
    let id: Option<i64> = redis::cmd("CLIENT")
        .arg("ID")
        .query_async(&mut conn)
        .await
        .ok();
    let _ = redis::cmd("CLIENT")
        .arg("SETNAME")
        .arg("NiceDB")
        .query_async::<Value>(&mut conn)
        .await;
    let modules = redis::cmd("MODULE")
        .arg("LIST")
        .query_async::<Value>(&mut conn)
        .await
        .map(|v| module_names(&v))
        .unwrap_or_default();
    let db = t.database;
    Ok(RedisConnection {
        conn,
        cluster: None,
        sentinel: None,
        db,
        server: ServerInfo {
            product,
            version,
            database: RedisTarget::name(db),
        },
        modules,
        cancel: Arc::new(Cancel {
            stop: Notify::new(),
            client,
            config,
            id,
            running: Mutex::new(false),
        }),
    })
}

fn is_master(role: &Value) -> bool {
    reply::list(role)
        .first()
        .and_then(reply::scalar)
        .is_some_and(|r| r.eq_ignore_ascii_case("master"))
}

pub(crate) fn module_names(v: &Value) -> Vec<String> {
    reply::list(v)
        .iter()
        .filter_map(|m| {
            reply::pairs(m)
                .into_iter()
                .find(|(k, _)| reply::scalar(k).as_deref() == Some("name"))
                .and_then(|(_, v)| reply::scalar(v))
        })
        .collect()
}

pub fn capabilities() -> Capabilities {
    Capabilities {
        family: Family::KeyValue,
        language: Language::Redis,
        schema: SchemaKind::Inferred,
        results: ResultShape::Keys,
        tree: TreeShape {
            databases: true,
            namespaces: NamespaceLevel::Multiple,
            groups: vec![Group::Tables],
            server: false,
        },
        words: Vocabulary {
            table: Noun::Key,
            column: Noun::Field,
        },
        transactions: false,
        editable: true,
        explain: false,
        er: false,
        ddl: false,
        sort: false,
        params: false,
        tools: vec![Tool::Server],
        cancel: nd_driver::Cancel::Server,
    }
}

impl RedisConnection {
    fn conn(&self) -> MultiplexedConnection {
        self.conn.clone()
    }

    pub(crate) fn slot_map(&self) -> Option<cluster::SlotMap> {
        self.cluster.as_ref().map(|c| c.snapshot())
    }

    pub fn has_module(&self, name: &str) -> bool {
        self.modules.iter().any(|m| m.eq_ignore_ascii_case(name))
    }

    pub fn has_json(&self) -> bool {
        self.has_module("ReJSON") || self.has_module("json")
    }

    async fn cancellable<T>(&self, work: impl Future<Output = Result<T>>) -> Result<T> {
        let stop = self.cancel.stop.notified();
        if let Ok(mut r) = self.cancel.running.lock() {
            *r = true;
        }
        let out = tokio::select! {
            r = work => r,
            () = stop => Err(DriverError::Cancelled),
        };
        if let Ok(mut r) = self.cancel.running.lock() {
            *r = false;
        }
        out
    }

    async fn query(&self, cmd: &redis::Cmd) -> Result<Value> {
        if let Some(cluster) = &self.cluster {
            return cluster.query(cmd).await;
        }
        let mut conn = self.conn();
        cmd.query_async(&mut conn).await.map_err(|e| self.moved(&e))
    }

    fn moved(&self, e: &redis::RedisError) -> DriverError {
        match &self.sentinel {
            Some(s) if e.code() == Some("READONLY") => demoted(&s.name),
            _ => err(e),
        }
    }

    pub(crate) fn sentinel(&self) -> Option<&SentinelLink> {
        self.sentinel.as_deref()
    }

    async fn pipe_key(&self, key: &str, pipe: &redis::Pipeline) -> Result<Vec<Value>> {
        if let Some(cluster) = &self.cluster {
            return cluster.pipe(key, pipe).await;
        }
        let mut conn = self.conn();
        pipe.query_async(&mut conn)
            .await
            .map_err(|e| self.moved(&e))
    }

    async fn pipe_each(
        &self,
        names: &[String],
        width: usize,
        add: impl Fn(&mut redis::Pipeline, &str),
    ) -> Vec<Value> {
        let build = |list: &[&String]| {
            let mut pipe = redis::pipe();
            pipe.ignore_errors();
            for n in list {
                add(&mut pipe, n);
            }
            pipe
        };
        let Some(cluster) = &self.cluster else {
            let mut conn = self.conn();
            return build(&names.iter().collect::<Vec<_>>())
                .query_async::<Vec<Value>>(&mut conn)
                .await
                .unwrap_or_default();
        };
        let map = cluster.snapshot();
        let mut groups: Vec<(cluster::Addr, Vec<usize>)> = Vec::new();
        for (i, n) in names.iter().enumerate() {
            let at = map
                .master(cluster::slot(n.as_bytes()))
                .cloned()
                .unwrap_or_default();
            match groups.iter_mut().find(|(a, _)| *a == at) {
                Some((_, list)) => list.push(i),
                None => groups.push((at, vec![i])),
            }
        }
        let mut out = vec![Value::Nil; names.len() * width];
        for (at, list) in groups {
            let keys: Vec<&String> = list.iter().map(|i| &names[*i]).collect();
            let Ok(mut conn) = cluster.node(&at).await else {
                continue;
            };
            let replies = build(&keys)
                .query_async::<Vec<Value>>(&mut conn)
                .await
                .unwrap_or_default();
            for (k, i) in list.iter().enumerate() {
                for j in 0..width {
                    if let Some(v) = replies.get(k * width + j) {
                        out[i * width + j] = v.clone();
                    }
                }
            }
        }
        out
    }

    async fn scan_page(&self, cursor: &str, args: &[&str]) -> Result<(String, Vec<String>)> {
        let mut cmd = redis::cmd("SCAN");
        let read = |page: &Value| {
            let parts = reply::list(page);
            let next = parts
                .first()
                .and_then(reply::scalar)
                .unwrap_or_else(|| "0".into());
            let names = parts
                .get(1)
                .map(|k| reply::list(k).iter().filter_map(reply::scalar).collect())
                .unwrap_or_default();
            (next, names)
        };
        let Some(cluster) = &self.cluster else {
            cmd.arg(cursor).arg(args);
            let page = self.cancellable(self.query(&cmd)).await?;
            return Ok(read(&page));
        };
        let masters = cluster.masters();
        let (node, at) = cluster::parse_cursor(cursor);
        let Some(addr) = masters.get(node) else {
            return Ok(("0".into(), Vec::new()));
        };
        cmd.arg(&at).arg(args);
        let page = self.cancellable(cluster.on(addr, &cmd)).await?;
        let (next, names) = read(&page);
        Ok((cluster::next_cursor(node, &next, masters.len()), names))
    }

    async fn dbsize(&self) -> Option<u64> {
        let count = |v: &Value| reply::scalar(v).and_then(|n| n.parse::<u64>().ok());
        let Some(cluster) = &self.cluster else {
            return self
                .query(&redis::cmd("DBSIZE"))
                .await
                .ok()
                .as_ref()
                .and_then(count);
        };
        let mut total = 0;
        for at in cluster.masters() {
            total += cluster
                .on(&at, &redis::cmd("DBSIZE"))
                .await
                .ok()
                .as_ref()
                .and_then(count)?;
        }
        Some(total)
    }

    async fn database_count(&self) -> u16 {
        if self.cluster.is_some() {
            return 1;
        }
        let reply = self
            .query(redis::cmd("CONFIG").arg("GET").arg("databases"))
            .await
            .ok();
        reply
            .as_ref()
            .and_then(|v| reply::pairs(v).first().and_then(|(_, n)| reply::scalar(n)))
            .and_then(|n| n.parse().ok())
            .unwrap_or(16)
    }

    async fn prefixes(&self) -> Result<Vec<Relation>> {
        let total = self.dbsize().await.unwrap_or(0);
        let mut seen: HashMap<String, Tally> = HashMap::new();
        let mut cursor = String::from("0");
        let mut sampled = 0usize;
        loop {
            let (next, names) = self.scan_page(&cursor, &["COUNT", "1000"]).await?;
            cursor = next;
            let kinds = self.kinds(&names).await;
            for (name, kind) in names.iter().zip(kinds) {
                let head = name.split(SEPARATOR).next().unwrap_or(name).to_owned();
                let entry = seen.entry(head).or_default();
                entry.0 += 1;
                *entry.1.entry(kind).or_default() += 1;
            }
            sampled += names.len();
            if cursor == "0" || sampled >= SCHEMA_SAMPLE {
                break;
            }
        }
        #[allow(clippy::cast_precision_loss)]
        let scale = if sampled > 0 && cursor != "0" {
            total as f64 / sampled as f64
        } else {
            1.0
        };
        let mut list: Vec<(String, Tally)> = seen.into_iter().collect();
        list.sort_by(|a, b| b.1.0.cmp(&a.1.0).then_with(|| a.0.cmp(&b.0)));
        list.truncate(SCHEMA_PREFIXES);
        list.sort_by(|a, b| a.0.cmp(&b.0));
        Ok(list
            .into_iter()
            .map(|(name, (n, kinds))| {
                let mut kinds: Vec<(String, u64)> = kinds.into_iter().collect();
                kinds.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
                #[allow(clippy::cast_precision_loss)]
                let estimated = (n as f64 * scale).round();
                Relation {
                    name,
                    columns: kinds
                        .into_iter()
                        .map(|(kind, count)| ColumnInfo {
                            name: kind,
                            type_name: count.to_string(),
                            nullable: true,
                            primary_key: false,
                        })
                        .collect(),
                    indexes: Vec::new(),
                    foreign_keys: Vec::new(),
                    estimated_rows: Some(estimated),
                    detail: None,
                }
            })
            .collect())
    }

    async fn kinds(&self, names: &[String]) -> Vec<String> {
        if names.is_empty() {
            return Vec::new();
        }
        self.pipe_each(names, 1, |pipe, n| {
            pipe.cmd("TYPE").arg(n);
        })
        .await
        .iter()
        .map(|k| reply::scalar(k).unwrap_or_default())
        .collect()
    }
}

#[async_trait::async_trait]
impl nd_driver::Connection for RedisConnection {
    fn capabilities(&self) -> Capabilities {
        capabilities()
    }

    fn server(&self) -> &ServerInfo {
        &self.server
    }

    fn canceller(&self) -> Arc<dyn Canceller> {
        Arc::new(RedisCanceller(self.cancel.clone()))
    }

    async fn execute(&self, text: &str, out: mpsc::Sender<Chunk>) -> Result<Outcome> {
        let args = cli::parse(text)?;
        let Some(first) = args.first() else {
            return Err(DriverError::query("empty command"));
        };
        let name = String::from_utf8_lossy(first).to_ascii_uppercase();
        if cli::REFUSED.contains(&name.as_str()) {
            return Err(DriverError::Query {
                message: format!("{name} is not sent from the editor"),
                code: Some("NICEDB".into()),
                detail: None,
                hint: Some(if name == "SELECT" {
                    "pick the database from the menu above the editor".into()
                } else {
                    "it would take over the connection the app shares".into()
                }),
                offset: None,
            });
        }
        let mut cmd = redis::Cmd::new();
        for a in &args {
            cmd.arg(a.as_slice());
        }
        let value = self.cancellable(self.query(&cmd)).await?;
        let write = cli::classify(text).write;
        let (columns, rows) = reply::table(&value);
        let mut batch = RowBatcher::new(out);
        batch.columns(columns).await?;
        for row in rows {
            batch.push(row).await?;
        }
        batch.flush().await?;
        let rows_affected = match value {
            Value::Int(n) if write => u64::try_from(n).ok(),
            _ => None,
        };
        Ok(Outcome {
            command: Some(name),
            rows_affected,
        })
    }

    async fn execute_bound(
        &self,
        _sql: &str,
        _params: &[Option<String>],
        _out: mpsc::Sender<Chunk>,
    ) -> Result<Outcome> {
        Err(DriverError::query("Redis commands take no parameters"))
    }

    async fn schema(&self) -> Result<Schema> {
        let count = self.database_count().await.max(self.db + 1);
        let mut ns = Namespace::new(RedisTarget::name(self.db));
        ns.tables = self.prefixes().await?;
        Ok(Schema {
            databases: (0..count).map(RedisTarget::name).collect(),
            current_database: RedisTarget::name(self.db),
            namespaces: vec![ns],
        })
    }

    async fn ddl(&self, _object: &ObjectRef) -> Result<String> {
        Err(DriverError::query("Redis keys have no DDL"))
    }

    async fn dependencies(&self, _object: &ObjectRef) -> Result<Vec<Dependency>> {
        Ok(Vec::new())
    }

    fn commands(&self, text: &str) -> Commands {
        cli::split(text)
    }

    fn classify(&self, command: &str) -> CommandClass {
        cli::classify(command)
    }

    async fn ping(&self) -> Result<()> {
        self.query(&redis::cmd("PING")).await?;
        if let Some(s) = &self.sentinel {
            let role = self.query(&redis::cmd("ROLE")).await?;
            if !is_master(&role) {
                return Err(demoted(&s.name));
            }
        }
        Ok(())
    }

    async fn scan_keys(&self, scan: &KeyScan) -> Result<KeyPage> {
        keys::scan(self, scan).await
    }

    async fn read_key(&self, read: &KeyRead) -> Result<KeyValue> {
        keys::read(self, read).await
    }

    async fn write_key(&self, write: &KeyWrite) -> Result<Outcome> {
        keys::write(self, write).await
    }

    fn describe_key_write(&self, write: &KeyWrite) -> String {
        keys::describe(write)
    }

    async fn report(&self) -> Result<Report> {
        report::report(self).await
    }
}
