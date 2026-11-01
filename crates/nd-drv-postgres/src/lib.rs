mod cells;
mod ddl;
mod deps;
mod error;
mod schema;
mod timescale;
mod tls;

use std::net::IpAddr;
use std::sync::Arc;
use std::time::Duration;

use futures_util::{StreamExt, pin_mut};
use nd_driver::{
    Canceller, Chunk, Column, Connection, Dialect, Edition, Flavor as Product, Outcome,
    PostgresClues, PostgresTarget, Result, RowBatcher, Schema, ServerInfo, TlsMode,
};
use tokio::sync::mpsc;
use tokio_postgres::types::{Format, IsNull, ToSql, Type, to_sql_checked};
use tokio_postgres::{CancelToken, Client, SimpleQueryMessage};

pub use error::map_error;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Flavor {
    Postgres,
    Cockroach,
}

pub struct PostgresConnection {
    client: Client,
    tls: tls::Tls,
    flavor: Flavor,
    cancel: Cancel,
    server: ServerInfo,
    edition: Option<Edition>,
    version_num: i32,
}

async fn clues(
    client: &Client,
    target: &PostgresTarget,
    banner: String,
) -> Result<(PostgresClues, i32)> {
    let row = client
        .query_one(
            "SELECT current_setting('server_version'), current_setting('server_version_num')::int4,
                    COALESCE(current_setting('neon.timeline_id', true), '')",
            &[],
        )
        .await
        .map_err(error::map_connect)?;
    let extensions = client
        .query(
            "SELECT extname::text, extversion::text FROM pg_extension",
            &[],
        )
        .await
        .map_err(error::map_connect)?
        .iter()
        .map(|r| (r.get(0), r.get(1)))
        .collect();
    let schemas = client
        .query(
            "SELECT nspname::text FROM pg_namespace WHERE nspname IN ('auth', 'storage', 'realtime')",
            &[],
        )
        .await
        .map_err(error::map_connect)?
        .iter()
        .map(|r| r.get(0))
        .collect();
    let yb_nodes = if banner.contains("-YB-") {
        client
            .query_one("SELECT count(*)::int4 FROM yb_servers()", &[])
            .await
            .map_or(0, |r| u32::try_from(r.get::<_, i32>(0)).unwrap_or(0))
    } else {
        0
    };
    let version_num: i32 = row.get(1);
    Ok((
        PostgresClues {
            host: target.host.clone(),
            port: target.port,
            user: target.user.clone(),
            banner,
            server_version: row.get(0),
            extensions,
            schemas,
            neon_timeline: row.get(2),
            yb_nodes,
        },
        version_num,
    ))
}

#[derive(Clone)]
enum Cancel {
    Token(CancelToken),
    Session {
        config: tokio_postgres::Config,
        session_id: String,
    },
}

pub async fn connect(
    target: &PostgresTarget,
    password: Option<&str>,
    via: Option<(IpAddr, u16)>,
) -> Result<PostgresConnection> {
    connect_as(target, (password, None), via, Flavor::Postgres).await
}

pub async fn connect_cockroach(
    target: &PostgresTarget,
    password: Option<&str>,
    via: Option<(IpAddr, u16)>,
) -> Result<PostgresConnection> {
    connect_as(target, (password, None), via, Flavor::Cockroach).await
}

pub async fn connect_with_key(
    target: &PostgresTarget,
    password: Option<&str>,
    key_passphrase: Option<&str>,
    via: Option<(IpAddr, u16)>,
    cockroach: bool,
) -> Result<PostgresConnection> {
    let flavor = if cockroach {
        Flavor::Cockroach
    } else {
        Flavor::Postgres
    };
    connect_as(target, (password, key_passphrase), via, flavor).await
}

async fn dial(cfg: &tokio_postgres::Config, tls: &tls::Tls) -> Result<Client> {
    Ok(match tls {
        tls::Tls::None => {
            let (client, conn) = cfg
                .connect(tokio_postgres::NoTls)
                .await
                .map_err(error::map_connect)?;
            tokio::spawn(async move { log_end(conn.await) });
            client
        }
        tls::Tls::Rustls(make) => {
            let (client, conn) = cfg
                .connect(make.clone())
                .await
                .map_err(error::map_connect)?;
            tokio::spawn(async move { log_end(conn.await) });
            client
        }
    })
}

async fn connect_as(
    target: &PostgresTarget,
    (password, key_passphrase): (Option<&str>, Option<&str>),
    via: Option<(IpAddr, u16)>,
    flavor: Flavor,
) -> Result<PostgresConnection> {
    let mut cfg = tokio_postgres::Config::new();
    cfg.host(&target.host)
        .port(via.map_or(target.port, |(_, port)| port))
        .user(&target.user)
        .dbname(&target.database)
        .application_name("NiceDB")
        .connect_timeout(Duration::from_secs(10))
        .keepalives_idle(Duration::from_secs(60));
    if let Some(pw) = password {
        cfg.password(pw);
    }
    if let Some((addr, _)) = via {
        cfg.hostaddr(addr);
    }
    cfg.ssl_mode(match target.tls {
        TlsMode::Disable => tokio_postgres::config::SslMode::Disable,
        TlsMode::Verify | TlsMode::Insecure => tokio_postgres::config::SslMode::Require,
    });

    let identity = match (&target.client_cert, target.tls) {
        (Some(cert), TlsMode::Verify | TlsMode::Insecure) => {
            Some(nd_driver::load_identity(cert, key_passphrase)?)
        }
        _ => None,
    };
    let tls = tls::Tls::new(target.tls, target.ca_file.as_deref(), identity)?;
    let client = dial(&cfg, &tls).await?;

    let mut edition = None;
    let mut version_num = 0;
    let (server, cancel) = match flavor {
        Flavor::Postgres => {
            let row = client
                .query_one("SELECT version(), current_database()", &[])
                .await
                .map_err(error::map_connect)?;
            let (found, num) = clues(&client, target, row.get(0)).await?;
            version_num = num;
            edition = nd_driver::postgres_edition(&found, target.flavor);
            let server = ServerInfo {
                product: edition
                    .as_ref()
                    .map_or("PostgreSQL", |e| e.flavor.product())
                    .into(),
                version: edition
                    .as_ref()
                    .and_then(Edition::shown_version)
                    .unwrap_or(found.server_version),
                database: row.get(1),
            };
            (server, Cancel::Token(client.cancel_token()))
        }
        Flavor::Cockroach => {
            let row = client
                .query_one(
                    "SELECT version(), current_database(), current_setting('session_id')",
                    &[],
                )
                .await
                .map_err(error::map_connect)?;
            let banner: String = row.get(0);
            let version = banner
                .split_whitespace()
                .find_map(|w| w.strip_prefix('v'))
                .unwrap_or(&banner)
                .to_owned();
            let server = ServerInfo {
                product: "CockroachDB".into(),
                version,
                database: row.get(1),
            };
            let cancel = Cancel::Session {
                config: cfg.clone(),
                session_id: row.get(2),
            };
            (server, cancel)
        }
    };
    Ok(PostgresConnection {
        client,
        tls,
        flavor,
        cancel,
        server,
        edition,
        version_num,
    })
}

fn log_end(r: std::result::Result<(), tokio_postgres::Error>) {
    if let Err(e) = r {
        tracing::debug!(error = %e, "postgres connection closed");
    }
}

const WRITES: &[&str] = &["INSERT", "UPDATE", "DELETE", "MERGE", "COPY"];

fn with_literals(sql: &str, params: &[Option<String>]) -> String {
    let mut out = String::with_capacity(sql.len() + 16);
    let mut last = 0;
    for p in nd_sql::placeholders(sql, Dialect::Postgres) {
        let nd_sql::ParamName::Named(n) = &p.name else {
            continue;
        };
        let Some(value) = n
            .parse::<usize>()
            .ok()
            .and_then(|n| n.checked_sub(1))
            .and_then(|i| params.get(i))
        else {
            continue;
        };
        out.push_str(&sql[last..p.range.start]);
        match value {
            Some(v) => out.push_str(&nd_sql::string(Dialect::Postgres, v)),
            None => out.push_str("NULL"),
        }
        last = p.range.end;
    }
    out.push_str(&sql[last..]);
    out
}

#[derive(Debug)]
struct TextParam<'a>(Option<&'a str>);

impl ToSql for TextParam<'_> {
    fn to_sql(
        &self,
        _: &Type,
        out: &mut bytes::BytesMut,
    ) -> std::result::Result<IsNull, Box<dyn std::error::Error + Sync + Send>> {
        Ok(match self.0 {
            Some(s) => {
                out.extend_from_slice(s.as_bytes());
                IsNull::No
            }
            None => IsNull::Yes,
        })
    }

    fn accepts(_: &Type) -> bool {
        true
    }

    fn encode_format(&self, _: &Type) -> Format {
        Format::Text
    }

    to_sql_checked!();
}

fn outcome(sql: &str, affected: Option<u64>) -> Outcome {
    let command = sql.split_whitespace().next().map(str::to_uppercase);
    let writes = command.as_deref().is_some_and(|c| WRITES.contains(&c));
    Outcome {
        rows_affected: affected.filter(|_| writes),
        command,
    }
}

#[async_trait::async_trait]
impl Connection for PostgresConnection {
    fn capabilities(&self) -> nd_driver::Capabilities {
        let caps = nd_driver::Capabilities::relational(Dialect::Postgres);
        if self.product() == Some(Product::Timescale) {
            let mut groups = caps.tree.groups.clone();
            groups.insert(2, nd_driver::Group::MaterializedViews);
            return caps.groups(&groups);
        }
        caps
    }

    fn server(&self) -> &ServerInfo {
        &self.server
    }

    fn edition(&self) -> Option<&Edition> {
        self.edition.as_ref()
    }

    fn canceller(&self) -> Arc<dyn Canceller> {
        Arc::new(PgCanceller {
            cancel: self.cancel.clone(),
            tls: self.tls.clone(),
        })
    }

    async fn execute(&self, sql: &str, out: mpsc::Sender<Chunk>) -> Result<Outcome> {
        let types: Option<Vec<String>> = match self.client.prepare(sql).await {
            Ok(st) => Some(
                st.columns()
                    .iter()
                    .map(|c| c.type_().name().to_owned())
                    .collect(),
            ),
            Err(e)
                if e.as_db_error()
                    .is_some_and(|d| d.code().code().starts_with("42")) =>
            {
                return Err(error::map_error(&e, sql));
            }
            Err(_) => None,
        };

        let stream = self
            .client
            .simple_query_raw(sql)
            .await
            .map_err(|e| error::map_error(&e, sql))?;
        pin_mut!(stream);
        let mut sink = RowBatcher::new(out);
        let mut affected = None;
        while let Some(msg) = stream.next().await {
            match msg.map_err(|e| error::map_error(&e, sql))? {
                SimpleQueryMessage::RowDescription(cols) => {
                    let columns = cols
                        .iter()
                        .enumerate()
                        .map(|(i, c)| Column {
                            name: c.name().to_owned(),
                            type_name: types.as_ref().and_then(|t| t.get(i).cloned()),
                        })
                        .collect();
                    sink.columns(columns).await?;
                }
                SimpleQueryMessage::Row(row) => {
                    sink.push(
                        (0..row.len())
                            .map(|i| row.get(i).map(str::to_owned))
                            .collect(),
                    )
                    .await?;
                }
                SimpleQueryMessage::CommandComplete(n) => affected = Some(n),
                _ => {}
            }
        }
        sink.flush().await?;
        Ok(outcome(sql, affected))
    }

    async fn execute_bound(
        &self,
        sql: &str,
        params: &[Option<String>],
        out: mpsc::Sender<Chunk>,
    ) -> Result<Outcome> {
        let err = |e: tokio_postgres::Error| error::map_error(&e, sql);
        let st = self.client.prepare(sql).await.map_err(err)?;
        if !st.columns().iter().all(|c| cells::decodable(c.type_())) {
            return self.execute(&with_literals(sql, params), out).await;
        }
        let mut sink = RowBatcher::new(out);
        if !st.columns().is_empty() {
            let columns = st
                .columns()
                .iter()
                .map(|c| Column {
                    name: c.name().to_owned(),
                    type_name: Some(c.type_().name().to_owned()),
                })
                .collect();
            sink.columns(columns).await?;
        }
        let stream = self
            .client
            .query_raw(&st, params.iter().map(|p| TextParam(p.as_deref())))
            .await
            .map_err(err)?;
        pin_mut!(stream);
        while let Some(row) = stream.next().await {
            sink.push(cells::row(&row.map_err(err)?).map_err(err)?)
                .await?;
        }
        sink.flush().await?;
        Ok(outcome(sql, stream.rows_affected()))
    }

    async fn schema(&self) -> Result<Schema> {
        self.schema_in(None).await
    }

    async fn outline(&self) -> Result<Option<nd_driver::Outline>> {
        schema::outline(&self.client, &self.server.database)
            .await
            .map(Some)
    }

    async fn schema_of(&self, namespaces: &[String]) -> Result<Schema> {
        self.schema_in(Some(namespaces)).await
    }

    async fn ddl(&self, object: &nd_driver::ObjectRef) -> Result<String> {
        let found = ddl::ddl(&self.client, self.flavor, self.version_num, object).await?;
        if self.product() == Some(Product::Timescale) {
            return Ok(timescale::ddl(&self.client, object, found).await);
        }
        Ok(found)
    }

    async fn dependencies(
        &self,
        object: &nd_driver::ObjectRef,
    ) -> Result<Vec<nd_driver::Dependency>> {
        deps::dependencies(&self.client, object).await
    }
}

impl PostgresConnection {
    async fn schema_in(&self, only: Option<&[String]>) -> Result<Schema> {
        let mut found =
            schema::load(&self.client, &self.server.database, self.flavor, only).await?;
        match self.product() {
            Some(Product::Timescale) => timescale::enrich(&self.client, &mut found).await,
            Some(Product::Supabase) => schema::mark_supabase(&mut found),
            _ => {}
        }
        Ok(found)
    }

    fn product(&self) -> Option<Product> {
        self.edition.as_ref().map(|e| e.flavor)
    }
}

struct PgCanceller {
    cancel: Cancel,
    tls: tls::Tls,
}

#[async_trait::async_trait]
impl Canceller for PgCanceller {
    async fn cancel(&self) {
        match &self.cancel {
            Cancel::Token(token) => {
                let r = match &self.tls {
                    tls::Tls::None => token.cancel_query(tokio_postgres::NoTls).await,
                    tls::Tls::Rustls(make) => token.cancel_query(make.clone()).await,
                };
                if let Err(e) = r {
                    tracing::warn!(error = %e, "postgres cancel request failed");
                }
            }
            Cancel::Session { config, session_id } => {
                let r = async {
                    let client = dial(config, &self.tls).await?;
                    client
                        .execute(
                            "CANCEL QUERIES IF EXISTS (SELECT query_id FROM [SHOW CLUSTER STATEMENTS] WHERE session_id = $1)",
                            &[session_id],
                        )
                        .await
                        .map_err(|e| error::map_error(&e, ""))
                }
                .await;
                if let Err(e) = r {
                    tracing::warn!(error = %e, "cockroach cancel failed");
                }
            }
        }
    }
}
