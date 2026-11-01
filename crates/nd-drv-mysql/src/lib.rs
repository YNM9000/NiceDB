mod ddl;
mod deps;
mod error;
mod schema;
mod tls;

use std::fmt::Write as _;
use std::net::IpAddr;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use mysql_async::consts::{ColumnFlags, ColumnType};
use mysql_async::prelude::{Protocol, Queryable};
use mysql_async::{Conn, Opts, OptsBuilder, QueryResult, Value};
use nd_driver::{
    Canceller, Chunk, Column, Connection, Dialect, DriverError, Edition, Flavor, MysqlClues,
    MysqlTarget, Outcome, Result, Row, RowBatcher, Schema, ServerInfo,
};
use tokio::sync::{Mutex, mpsc};

pub use error::map_error;

const BINARY_CHARSET: u16 = 63;
const WRITES: &[&str] = &["INSERT", "UPDATE", "DELETE", "REPLACE", "LOAD"];

pub struct MysqlConnection {
    conn: Mutex<Conn>,
    opts: Opts,
    id: u32,
    killed: Arc<AtomicBool>,
    server: ServerInfo,
    edition: Option<Edition>,
}

async fn clues(conn: &mut Conn, target: &MysqlTarget, version: &str) -> MysqlClues {
    let lower = version.to_ascii_lowercase();
    let mut store = String::new();
    let mut nodes = 0;
    if lower.contains("tidb") {
        store = conn
            .query_first::<Option<String>, _>(
                "SELECT JSON_UNQUOTE(JSON_EXTRACT(@@tidb_config, '$.store'))",
            )
            .await
            .ok()
            .flatten()
            .flatten()
            .unwrap_or_default();
        nodes = conn
            .query_first::<u32, _>("SELECT COUNT(*) FROM information_schema.CLUSTER_INFO")
            .await
            .ok()
            .flatten()
            .unwrap_or(0);
    } else if lower.contains("vitess") || target.host.to_ascii_lowercase().ends_with(".psdb.cloud")
    {
        store = conn
            .query_first::<String, _>("SHOW VITESS_TARGET")
            .await
            .ok()
            .flatten()
            .unwrap_or_default();
    }
    MysqlClues {
        host: target.host.clone(),
        version: version.to_owned(),
        store,
        nodes,
    }
}

pub async fn connect(
    target: &MysqlTarget,
    password: Option<&str>,
    via: Option<(IpAddr, u16)>,
) -> Result<MysqlConnection> {
    connect_with_key(target, password, None, via).await
}

pub async fn connect_with_key(
    target: &MysqlTarget,
    password: Option<&str>,
    key_passphrase: Option<&str>,
    via: Option<(IpAddr, u16)>,
) -> Result<MysqlConnection> {
    let client = target.client_cert.as_ref().map(|c| (c, key_passphrase));
    let mut builder = OptsBuilder::default()
        .ip_or_hostname(target.host.clone())
        .tcp_port(via.map_or(target.port, |(_, port)| port))
        .user(Some(&target.user))
        .pass(password)
        .db_name(Some(&target.database).filter(|d| !d.is_empty()))
        .prefer_socket(false)
        .client_found_rows(true)
        .tcp_keepalive(Some(Duration::from_secs(60)))
        .connect_attribute("program_name", "NiceDB")
        .ssl_opts(tls::ssl_opts(target.tls, target.ca_file.as_deref(), client).await?);
    if let Some((addr, _)) = via {
        builder = builder.resolved_ips(Some(vec![addr]));
    }
    let opts = Opts::from(builder);
    let mut conn = dial(&opts).await?;

    let (version, database): (String, Option<String>) = conn
        .query_first("SELECT VERSION(), DATABASE()")
        .await
        .map_err(|e| error::map_connect(&e))?
        .unwrap_or_default();
    let edition =
        nd_driver::mysql_edition(&clues(&mut conn, target, &version).await, target.flavor);
    let server = ServerInfo {
        product: match &edition {
            Some(e) => e.flavor.product(),
            None if version.contains("MariaDB") => "MariaDB",
            None => "MySQL",
        }
        .into(),
        version: edition
            .as_ref()
            .and_then(Edition::shown_version)
            .unwrap_or(version),
        database: database.unwrap_or_default(),
    };
    Ok(MysqlConnection {
        id: conn.id(),
        conn: Mutex::new(conn),
        opts,
        killed: Arc::new(AtomicBool::new(false)),
        server,
        edition,
    })
}

async fn dial(opts: &Opts) -> Result<Conn> {
    tokio::time::timeout(Duration::from_secs(10), Conn::new(opts.clone()))
        .await
        .map_err(|_| DriverError::Connect {
            message: "connection timed out".into(),
        })?
        .map_err(|e| error::map_connect(&e))
}

#[async_trait::async_trait]
impl Connection for MysqlConnection {
    fn capabilities(&self) -> nd_driver::Capabilities {
        use nd_driver::{Group, Tool};
        let caps = nd_driver::Capabilities::relational(Dialect::Mysql)
            .namespaces(nd_driver::NamespaceLevel::Multiple);
        match self.edition.as_ref().map(|e| e.flavor) {
            Some(Flavor::Tidb) => caps.groups(&[Group::Tables, Group::Views, Group::Sequences]),
            Some(Flavor::Planetscale) => {
                let mut caps = caps.groups(&[Group::Tables, Group::Views]);
                caps.tools
                    .retain(|t| !matches!(t, Tool::Users | Tool::Backup | Tool::Seclab));
                caps
            }
            _ => caps.groups(&[
                Group::Tables,
                Group::Views,
                Group::Functions,
                Group::Sequences,
                Group::Triggers,
            ]),
        }
    }

    fn server(&self) -> &ServerInfo {
        &self.server
    }

    fn edition(&self) -> Option<&Edition> {
        self.edition.as_ref()
    }

    fn canceller(&self) -> Arc<dyn Canceller> {
        Arc::new(MysqlCanceller {
            opts: self.opts.clone(),
            id: self.id,
            killed: self.killed.clone(),
        })
    }

    async fn execute(&self, sql: &str, out: mpsc::Sender<Chunk>) -> Result<Outcome> {
        self.run(sql, None, out).await
    }

    async fn execute_bound(
        &self,
        sql: &str,
        params: &[Option<String>],
        out: mpsc::Sender<Chunk>,
    ) -> Result<Outcome> {
        self.run(sql, Some(params), out).await
    }

    async fn schema(&self) -> Result<Schema> {
        let mut conn = self.conn.lock().await;
        schema::load(&mut conn, &self.server.database).await
    }

    async fn ddl(&self, object: &nd_driver::ObjectRef) -> Result<String> {
        let mut conn = self.conn.lock().await;
        ddl::ddl(&mut conn, object).await
    }

    async fn dependencies(
        &self,
        object: &nd_driver::ObjectRef,
    ) -> Result<Vec<nd_driver::Dependency>> {
        let mut conn = self.conn.lock().await;
        deps::dependencies(&mut conn, object).await
    }
}

impl MysqlConnection {
    async fn run(
        &self,
        sql: &str,
        params: Option<&[Option<String>]>,
        out: mpsc::Sender<Chunk>,
    ) -> Result<Outcome> {
        let mut conn = self.conn.lock().await;
        self.killed.store(false, Ordering::Release);
        let err = |e: mysql_async::Error| error::map_error(&e, sql);
        let affected = match params {
            None => match conn.query_iter(sql).await {
                Ok(r) => stream(r, sql, out).await,
                Err(e) => Err(err(e)),
            },
            Some(p) => {
                let values: Vec<Value> = p
                    .iter()
                    .map(|v| {
                        v.as_ref()
                            .map_or(Value::NULL, |s| Value::Bytes(s.clone().into_bytes()))
                    })
                    .collect();
                match conn.exec_iter(sql, values).await {
                    Ok(r) => stream(r, sql, out).await,
                    Err(e) => Err(err(e)),
                }
            }
        };
        if self.killed.load(Ordering::Acquire) {
            return Err(DriverError::Cancelled);
        }
        let affected = affected?;

        let command = command(sql);
        let writes = command.as_deref().is_some_and(|c| WRITES.contains(&c));
        Ok(Outcome {
            rows_affected: writes.then_some(affected),
            command,
        })
    }
}

fn command(sql: &str) -> Option<String> {
    let mut rest = sql.trim_start();
    loop {
        rest = if rest.starts_with("--") || rest.starts_with('#') {
            rest.split_once('\n').map_or("", |(_, r)| r)
        } else if let Some(r) = rest.strip_prefix("/*") {
            r.split_once("*/").map_or("", |(_, r)| r)
        } else {
            break;
        }
        .trim_start();
    }
    rest.split_whitespace().next().map(str::to_uppercase)
}

async fn stream<P: Protocol>(
    mut result: QueryResult<'_, '_, P>,
    sql: &str,
    out: mpsc::Sender<Chunk>,
) -> Result<u64> {
    let err = |e: mysql_async::Error| error::map_error(&e, sql);
    let mut sink = RowBatcher::new(out);
    loop {
        let mut kinds = Vec::new();
        if let Some(cols) = result.columns().filter(|c| !c.is_empty()) {
            kinds = cols.iter().map(Kind::of).collect();
            let columns = cols
                .iter()
                .map(|c| Column {
                    name: c.name_str().into_owned(),
                    type_name: Some(type_name(c)),
                })
                .collect();
            sink.columns(columns).await?;
        }
        while let Some(row) = result.next().await.map_err(err)? {
            sink.push(text_row(row, &kinds)).await?;
        }
        sink.flush().await?;
        if result.is_empty() {
            return Ok(result.affected_rows());
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    Binary,
    Date,
    Other,
}

impl Kind {
    fn of(c: &mysql_async::Column) -> Self {
        if is_binary_string(c) {
            Self::Binary
        } else if matches!(
            c.column_type(),
            ColumnType::MYSQL_TYPE_DATE | ColumnType::MYSQL_TYPE_NEWDATE
        ) {
            Self::Date
        } else {
            Self::Other
        }
    }
}

fn text_row(row: mysql_async::Row, kinds: &[Kind]) -> Row {
    row.unwrap()
        .into_iter()
        .enumerate()
        .map(|(i, v)| {
            let kind = kinds.get(i).copied().unwrap_or(Kind::Other);
            match v {
                Value::NULL => None,
                Value::Bytes(b) if kind == Kind::Binary => Some(to_hex(&b)),
                Value::Bytes(b) => Some(
                    String::from_utf8(b)
                        .unwrap_or_else(|e| String::from_utf8_lossy(e.as_bytes()).into_owned()),
                ),
                other => Some(binary_text(&other, kind)),
            }
        })
        .collect()
}

fn binary_text(value: &Value, kind: Kind) -> String {
    match *value {
        Value::Int(i) => i.to_string(),
        Value::UInt(u) => u.to_string(),
        Value::Float(f) => f.to_string(),
        Value::Double(d) => d.to_string(),
        Value::Date(year, month, day, ..) if kind == Kind::Date => {
            format!("{year:04}-{month:02}-{day:02}")
        }
        Value::Date(year, month, day, hour, min, sec, micros) => {
            let mut out = format!("{year:04}-{month:02}-{day:02} {hour:02}:{min:02}:{sec:02}");
            if micros > 0 {
                let _ = write!(out, ".{micros:06}");
            }
            out
        }
        Value::Time(negative, days, hour, min, sec, micros) => {
            let hours = days * 24 + u32::from(hour);
            let sign = if negative { "-" } else { "" };
            let mut out = format!("{sign}{hours:02}:{min:02}:{sec:02}");
            if micros > 0 {
                let _ = write!(out, ".{micros:06}");
            }
            out
        }
        _ => value.as_sql(true),
    }
}

fn to_hex(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(2 + bytes.len() * 2);
    s.push_str("0x");
    for b in bytes {
        let _ = write!(s, "{b:02X}");
    }
    s
}

fn is_binary_string(c: &mysql_async::Column) -> bool {
    use ColumnType as T;
    c.character_set() == BINARY_CHARSET
        && matches!(
            c.column_type(),
            T::MYSQL_TYPE_STRING
                | T::MYSQL_TYPE_VAR_STRING
                | T::MYSQL_TYPE_VARCHAR
                | T::MYSQL_TYPE_TINY_BLOB
                | T::MYSQL_TYPE_BLOB
                | T::MYSQL_TYPE_MEDIUM_BLOB
                | T::MYSQL_TYPE_LONG_BLOB
                | T::MYSQL_TYPE_BIT
                | T::MYSQL_TYPE_GEOMETRY
        )
}

fn type_name(c: &mysql_async::Column) -> String {
    use ColumnType as T;
    let binary = c.character_set() == BINARY_CHARSET;
    let flags = c.flags();
    let name = match c.column_type() {
        _ if flags.contains(ColumnFlags::ENUM_FLAG) => "enum",
        _ if flags.contains(ColumnFlags::SET_FLAG) => "set",
        T::MYSQL_TYPE_TINY => "tinyint",
        T::MYSQL_TYPE_SHORT => "smallint",
        T::MYSQL_TYPE_INT24 => "mediumint",
        T::MYSQL_TYPE_LONG => "int",
        T::MYSQL_TYPE_LONGLONG => "bigint",
        T::MYSQL_TYPE_FLOAT => "float",
        T::MYSQL_TYPE_DOUBLE => "double",
        T::MYSQL_TYPE_DECIMAL | T::MYSQL_TYPE_NEWDECIMAL => "decimal",
        T::MYSQL_TYPE_DATE | T::MYSQL_TYPE_NEWDATE => "date",
        T::MYSQL_TYPE_TIME | T::MYSQL_TYPE_TIME2 => "time",
        T::MYSQL_TYPE_DATETIME | T::MYSQL_TYPE_DATETIME2 => "datetime",
        T::MYSQL_TYPE_TIMESTAMP | T::MYSQL_TYPE_TIMESTAMP2 => "timestamp",
        T::MYSQL_TYPE_YEAR => "year",
        T::MYSQL_TYPE_BIT => "bit",
        T::MYSQL_TYPE_JSON => "json",
        T::MYSQL_TYPE_GEOMETRY => "geometry",
        T::MYSQL_TYPE_ENUM => "enum",
        T::MYSQL_TYPE_SET => "set",
        T::MYSQL_TYPE_NULL => "null",
        T::MYSQL_TYPE_STRING if binary => "binary",
        T::MYSQL_TYPE_STRING => "char",
        T::MYSQL_TYPE_VAR_STRING | T::MYSQL_TYPE_VARCHAR if binary => "varbinary",
        T::MYSQL_TYPE_VAR_STRING | T::MYSQL_TYPE_VARCHAR => "varchar",
        T::MYSQL_TYPE_TINY_BLOB
        | T::MYSQL_TYPE_BLOB
        | T::MYSQL_TYPE_MEDIUM_BLOB
        | T::MYSQL_TYPE_LONG_BLOB
            if binary =>
        {
            "blob"
        }
        T::MYSQL_TYPE_TINY_BLOB
        | T::MYSQL_TYPE_BLOB
        | T::MYSQL_TYPE_MEDIUM_BLOB
        | T::MYSQL_TYPE_LONG_BLOB => "text",
        _ => "unknown",
    };
    let numeric = matches!(
        name,
        "tinyint" | "smallint" | "mediumint" | "int" | "bigint" | "float" | "double" | "decimal"
    );
    if numeric && flags.contains(ColumnFlags::UNSIGNED_FLAG) {
        format!("{name} unsigned")
    } else {
        name.into()
    }
}

struct MysqlCanceller {
    opts: Opts,
    id: u32,
    killed: Arc<AtomicBool>,
}

#[async_trait::async_trait]
impl Canceller for MysqlCanceller {
    async fn cancel(&self) {
        self.killed.store(true, Ordering::Release);
        let r = async {
            let mut conn = dial(&self.opts).await?;
            conn.query_drop(format!("KILL QUERY {}", self.id))
                .await
                .map_err(|e| error::map_error(&e, ""))?;
            conn.disconnect()
                .await
                .map_err(|e| error::map_error(&e, ""))
        }
        .await;
        if let Err(e) = r {
            tracing::warn!(error = %e, "mysql cancel request failed");
        }
    }
}
