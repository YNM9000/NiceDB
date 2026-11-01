use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use nd_driver::{
    Capabilities, Connection, Dependency, Dialect, DriverError, Namespace, NamespaceSize,
    ObjectRef, Schema, ServerInfo, SshAuth, Target,
};
use nd_net::{Route, Tunnel};
use nd_store::{ConnectionProfile, Environment, SessionOptions, Template, UnlockMode};
use serde::{Deserialize, Serialize};
use zeroize::Zeroizing;

use crate::guard::quiet;
use crate::tx::CommitMode;
use crate::{Core, CoreError, Result};

#[derive(Clone, Default, Deserialize, specta::Type)]
pub struct Secrets {
    pub password: Option<String>,
    pub ssh: Option<String>,
    #[serde(default)]
    pub proxy: Option<String>,
    #[serde(default)]
    pub tls_key: Option<String>,
    #[serde(default)]
    pub jumps: Vec<Option<String>>,
}

fn masked(v: Option<&String>) -> Option<&'static str> {
    v.map(|_| "***")
}

impl std::fmt::Debug for Secrets {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Secrets")
            .field("password", &masked(self.password.as_ref()))
            .field("ssh", &masked(self.ssh.as_ref()))
            .field("proxy", &masked(self.proxy.as_ref()))
            .field("tls_key", &masked(self.tls_key.as_ref()))
            .field(
                "jumps",
                &self
                    .jumps
                    .iter()
                    .map(|j| masked(j.as_ref()))
                    .collect::<Vec<_>>(),
            )
            .finish()
    }
}

#[derive(Debug, Clone, Serialize, specta::Type)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum VarPreview {
    Value { text: String },
    Problem { error: nd_store::VarError },
}

pub const MAX_JUMPS: usize = 8;
pub const LAZY_FROM: u64 = 2000;

pub(crate) fn secret_names() -> Vec<String> {
    let mut out: Vec<String> = ["password", "ssh", "proxy", "tls-key"]
        .into_iter()
        .map(str::to_owned)
        .collect();
    out.extend((0..MAX_JUMPS).map(|i| format!("ssh-jump-{i}")));
    out
}

#[derive(Default)]
pub(crate) struct Creds {
    pub password: Option<Zeroizing<String>>,
    pub tls_key: Option<Zeroizing<String>>,
    pub db2_home: Option<String>,
}

#[derive(Debug, Clone, Serialize, specta::Type)]
pub struct SessionInfo {
    pub session_id: String,
    pub connection_id: String,
    pub dialect: Option<Dialect>,
    pub caps: Capabilities,
    pub server: ServerInfo,
    pub environment: Environment,
    pub transaction_lost: bool,
    pub edition: Option<nd_driver::Edition>,
}

#[derive(Debug, Clone, Serialize, specta::Type)]
pub struct InitFailure {
    pub index: u32,
    pub sql: String,
    pub error: DriverError,
}

pub(crate) struct Profile {
    pub name: String,
    pub environment: Environment,
    pub options: SessionOptions,
}

pub(crate) struct Session {
    pub connection_id: String,
    pub profile: Mutex<Profile>,
    pub conn: Box<dyn Connection>,
    pub caps: Capabilities,
    pub busy: AtomicBool,
    pub unlocked_until: Mutex<Option<Instant>>,
    pub server_read_only: AtomicBool,
    pub untimed: AtomicBool,
    pub in_transaction: AtomicBool,
    pub(crate) tx: Mutex<crate::tx::Tx>,
    pub side: tokio::sync::Mutex<Option<crate::tools::Side>>,
    pub target: Target,
    pub(crate) tunnel: Option<Arc<Tunnel>>,
    pub last_used: Mutex<Instant>,
    pub last_ping: Mutex<Instant>,
    pub lost: AtomicBool,
    pub tx_lost: AtomicBool,
    pub retry: Mutex<(Instant, Duration)>,
}

pub(crate) const RETRY_FIRST: Duration = Duration::from_secs(5);
pub(crate) const PING_TIMEOUT: Duration = Duration::from_secs(10);

pub(crate) fn dropped(e: &DriverError) -> bool {
    match e {
        DriverError::Io { .. } | DriverError::Connect { .. } => true,
        DriverError::Query { code: Some(c), .. } => {
            c.starts_with("08") || matches!(c.as_str(), "57P01" | "57P02" | "57P03" | "70100")
        }
        DriverError::Query { message, .. } => {
            let m = message.to_ascii_lowercase();
            [
                "connection closed",
                "broken pipe",
                "connection reset",
                "gone away",
                "lost connection",
            ]
            .iter()
            .any(|w| m.contains(w))
        }
        _ => false,
    }
}

pub(crate) trait Sql {
    fn sql(&self) -> std::result::Result<Dialect, NotSql>;
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct NotSql;

impl From<NotSql> for CoreError {
    fn from(_: NotSql) -> Self {
        CoreError::NotSql
    }
}

impl From<NotSql> for DriverError {
    fn from(_: NotSql) -> Self {
        DriverError::query("this engine does not speak SQL")
    }
}

impl Sql for dyn Connection + '_ {
    fn sql(&self) -> std::result::Result<Dialect, NotSql> {
        self.dialect().ok_or(NotSql)
    }
}

impl Sql for Box<dyn Connection> {
    fn sql(&self) -> std::result::Result<Dialect, NotSql> {
        self.as_ref().sql()
    }
}

pub(crate) async fn initialize(conn: &dyn Connection, init: &str) -> Result<()> {
    let ranges: Vec<std::ops::Range<usize>> = match conn.dialect() {
        Some(d) => nd_sql::split(init, d)
            .into_iter()
            .map(|s| s.range)
            .collect(),
        None => conn.commands(init),
    };
    for (i, range) in ranges.into_iter().enumerate() {
        let sql = &init[range];
        if let Err(error) = quiet(conn, sql).await {
            return Err(CoreError::Init(Box::new(InitFailure {
                index: u32::try_from(i).unwrap_or(u32::MAX),
                sql: sql.to_owned(),
                error,
            })));
        }
    }
    Ok(())
}

impl Session {
    pub(crate) fn options(&self) -> SessionOptions {
        self.profile
            .lock()
            .map(|p| p.options.clone())
            .unwrap_or_default()
    }

    pub(crate) fn touch(&self) {
        if let Ok(mut t) = self.last_used.lock() {
            *t = Instant::now();
        }
    }

    pub(crate) fn idle_for(&self) -> Duration {
        self.last_used
            .lock()
            .map_or(Duration::ZERO, |t| t.elapsed())
    }

    pub(crate) fn quiet_for(&self) -> Duration {
        let ping = self
            .last_ping
            .lock()
            .map_or(Duration::ZERO, |t| t.elapsed());
        ping.min(self.idle_for())
    }

    pub(crate) fn pinged(&self) {
        if let Ok(mut t) = self.last_ping.lock() {
            *t = Instant::now();
        }
    }

    pub(crate) fn networked(&self) -> bool {
        self.target.remote().is_some()
            || matches!(
                self.target,
                Target::Libsql(nd_driver::LibsqlTarget::Remote { .. })
            )
    }

    pub(crate) async fn ping(&self) -> std::result::Result<(), DriverError> {
        tokio::time::timeout(PING_TIMEOUT, self.conn.ping())
            .await
            .unwrap_or_else(|_| {
                Err(DriverError::Io {
                    message: "no answer from the server".into(),
                })
            })
    }

    pub(crate) async fn gone(&self, error: &DriverError) -> bool {
        if !self.networked() {
            return false;
        }
        if dropped(error) {
            return true;
        }
        matches!(error, DriverError::Query { .. }) && self.ping().await.is_err_and(|e| dropped(&e))
    }

    pub(crate) fn info(&self, session_id: &str) -> SessionInfo {
        SessionInfo {
            session_id: session_id.to_owned(),
            connection_id: self.connection_id.clone(),
            dialect: self.conn.dialect(),
            caps: self.caps.clone(),
            server: self.conn.server().clone(),
            environment: self.environment(),
            transaction_lost: false,
            edition: self.conn.edition().cloned(),
        }
    }

    pub(crate) fn mark_lost(&self) {
        if self.tx_open() {
            self.tx_lost.store(true, Ordering::Release);
            self.ended();
        }
        self.lost.store(true, Ordering::Release);
    }
}

pub(crate) async fn connect_target(
    target: &Target,
    creds: &Creds,
    tunnel: Option<&Tunnel>,
) -> Result<Box<dyn Connection>> {
    let via = tunnel.map(|tn| (tn.local.ip(), tn.local.port()));
    let (password, key) = (
        creds.password.as_deref().map(String::as_str),
        creds.tls_key.as_deref().map(String::as_str),
    );
    Ok(match target {
        Target::Sqlite(t) => Box::new(nd_drv_sqlite::connect(t).await?),
        Target::Postgres(t) => {
            Box::new(nd_drv_postgres::connect_with_key(t, password, key, via, false).await?)
        }
        Target::Mysql(t) => Box::new(nd_drv_mysql::connect_with_key(t, password, key, via).await?),
        Target::Sqlserver(t) => Box::new(nd_drv_mssql::connect(t, (password, key), via).await?),
        Target::Cockroach(t) => {
            Box::new(nd_drv_postgres::connect_with_key(t, password, key, via, true).await?)
        }
        Target::Duckdb(t) => Box::new(nd_drv_duckdb::connect(t).await?),
        Target::Libsql(t) => nd_drv_libsql::connect(t, password, tunnel.map(|tn| tn.local)).await?,
        Target::Mongodb(t) => Box::new(nd_drv_mongo::connect(t, password, key, via).await?),
        Target::Clickhouse(t) => Box::new(nd_drv_clickhouse::connect(t, password, key, via).await?),
        Target::Redis(t) => Box::new(nd_drv_redis::connect(t, password, key, via).await?),
        Target::Cassandra(t) => Box::new(nd_drv_cassandra::connect(t, password, key, via).await?),
        Target::Dynamodb(t) => Box::new(nd_drv_dynamo::connect(t, password).await?),
        Target::Elasticsearch(t) => Box::new(nd_drv_elastic::connect(t, password, key, via).await?),
        Target::Firebird(t) => Box::new(nd_drv_firebird::connect(t, password, via).await?),
        Target::Oracle(t) => Box::new(nd_drv_oracle::connect(t, password, via).await?),
        Target::Db2(t) => {
            nd_drv_odbc::connect_db2(t, password, via, creds.db2_home.as_deref()).await?
        }
        Target::Hana(t) => nd_drv_hana::connect(t, password, via).await?,
        Target::Odbc(t) => nd_drv_odbc::connect(t, password).await?,
    })
}

const JUMP_KEYS: [&str; MAX_JUMPS] = [
    "ssh-jump-0",
    "ssh-jump-1",
    "ssh-jump-2",
    "ssh-jump-3",
    "ssh-jump-4",
    "ssh-jump-5",
    "ssh-jump-6",
    "ssh-jump-7",
];

pub(crate) fn secret_key(id: &str, what: &str) -> String {
    format!("{id}/{what}")
}

impl Core {
    pub(crate) async fn secret(&self, id: &str, what: &'static str) -> Result<Option<String>> {
        let (store, k) = (self.secrets.clone(), secret_key(id, what));
        tokio::task::spawn_blocking(move || store.get(&k))
            .await
            .map_err(|e| CoreError::Secrets(e.to_string()))?
            .map_err(Into::into)
    }

    async fn resolve(
        &self,
        id: &str,
        given: Option<String>,
        what: &'static str,
    ) -> Result<Option<String>> {
        match given {
            Some(v) if v.is_empty() => Ok(None),
            Some(v) => Ok(Some(v)),
            None => self.secret(id, what).await,
        }
    }

    fn db2_home(&self, target: &Target) -> Option<String> {
        matches!(target, Target::Db2(_))
            .then(|| self.preferences().drivers.db2_clidriver)
            .filter(|h| !h.trim().is_empty())
    }

    pub(crate) async fn creds(&self, id: &str, target: &Target) -> Result<Creds> {
        Ok(Creds {
            password: if target.needs_password() {
                self.secret(id, "password").await?.map(Zeroizing::new)
            } else {
                None
            },
            tls_key: if target.client_cert().is_some() {
                self.secret(id, "tls-key").await?.map(Zeroizing::new)
            } else {
                None
            },
            db2_home: self.db2_home(target),
        })
    }

    async fn dial(
        &self,
        profile: &ConnectionProfile,
        secrets: Secrets,
    ) -> Result<(Box<dyn Connection>, Option<Arc<Tunnel>>)> {
        let id = profile.id.as_str();
        let target = &profile.target;
        let creds = Creds {
            password: if target.needs_password() {
                self.resolve(id, secrets.password, "password")
                    .await?
                    .map(Zeroizing::new)
            } else {
                None
            },
            tls_key: if target.client_cert().is_some() {
                self.resolve(id, secrets.tls_key, "tls-key")
                    .await?
                    .map(Zeroizing::new)
            } else {
                None
            },
            db2_home: self.db2_home(target),
        };
        let tunnel = match target.remote() {
            Some((host, port)) if target.ssh().is_some() || target.proxy().is_some() => {
                let proxy_secret = match target.proxy() {
                    Some(p) if p.user.as_deref().is_some_and(|u| !u.is_empty()) => {
                        self.resolve(id, secrets.proxy, "proxy").await?
                    }
                    _ => None,
                };
                let (ssh_secret, jump_secrets) = match target.ssh() {
                    Some(ssh) => {
                        let main = if matches!(ssh.auth, SshAuth::Agent) {
                            None
                        } else {
                            self.resolve(id, secrets.ssh, "ssh").await?
                        };
                        let mut jumps = Vec::new();
                        for (i, hop) in ssh.jumps.iter().enumerate().take(MAX_JUMPS) {
                            jumps.push(if matches!(hop.auth, SshAuth::Agent) {
                                None
                            } else {
                                let given = secrets.jumps.get(i).cloned().flatten();
                                self.resolve(id, given, JUMP_KEYS[i]).await?
                            });
                        }
                        (main, jumps)
                    }
                    None => (None, Vec::new()),
                };
                let mut route = match target.ssh() {
                    Some(ssh) => Route::ssh(ssh, ssh_secret.as_deref(), &jump_secrets),
                    None => Route::default(),
                };
                route.proxy = target.proxy().map(|p| (p, proxy_secret.as_deref()));
                let remote = (host.to_owned(), port);
                Some(Arc::new(
                    nd_net::open_route(&route, &self.known_hosts, remote).await?,
                ))
            }
            _ => None,
        };
        let conn = connect_target(target, &creds, tunnel.as_deref()).await?;
        Ok((conn, tunnel))
    }

    pub async fn test_connection(
        &self,
        profile: &ConnectionProfile,
        secrets: Secrets,
    ) -> Result<ServerInfo> {
        let profile = &self.resolved(profile)?;
        let (conn, _tunnel) = self.dial(profile, secrets).await?;
        initialize(conn.as_ref(), &profile.session.init).await?;
        Ok(conn.server().clone())
    }

    fn production_name(&self, id: &str) -> Result<Option<String>> {
        if let Some(p) = self.store(|s| s.connection(id))?
            && p.environment == Environment::Production
        {
            return Ok(Some(p.name));
        }
        if let Some(Some(name)) = Self::lock(&self.trashed)?.get(id) {
            return Ok(Some(name.clone()));
        }
        for session in Self::lock(&self.sessions)?
            .values()
            .filter(|s| s.connection_id == id)
        {
            let p = session.profile.lock().map_err(|_| CoreError::Poisoned)?;
            if p.environment == Environment::Production {
                return Ok(Some(p.name.clone()));
            }
        }
        Ok(None)
    }

    fn admit_environment(
        &self,
        profile: &ConnectionProfile,
        confirm: Option<&str>,
    ) -> Result<Option<String>> {
        if profile.environment == Environment::Production {
            return Ok(None);
        }
        let Some(name) = self.production_name(&profile.id)? else {
            return Ok(None);
        };
        self.confirmed(&name, confirm)?;
        Ok(Some(format!(
            "-- environment production -> {}",
            profile.environment.as_str()
        )))
    }

    pub async fn save_connection(
        &self,
        profile: &ConnectionProfile,
        secrets: Secrets,
    ) -> Result<()> {
        self.save_connection_confirmed(profile, secrets, None).await
    }

    pub async fn save_connection_confirmed(
        &self,
        profile: &ConnectionProfile,
        secrets: Secrets,
        confirm: Option<&str>,
    ) -> Result<()> {
        let lowered = self.admit_environment(profile, confirm)?;
        self.store(|s| s.save_connection(profile))?;
        if let Some(line) = lowered {
            self.audit_as(&profile.id, String::new(), &line, None, None);
        }
        for session in Self::lock(&self.sessions)?
            .values()
            .filter(|s| s.connection_id == profile.id)
        {
            let mut p = session.profile.lock().map_err(|_| CoreError::Poisoned)?;
            p.name.clone_from(&profile.name);
            p.environment = profile.environment;
            p.options.clone_from(&profile.session);
        }
        let store = self.secrets.clone();
        let id = profile.id.clone();
        tokio::task::spawn_blocking(move || -> Result<()> {
            let jumps = secrets
                .jumps
                .into_iter()
                .take(MAX_JUMPS)
                .enumerate()
                .map(|(i, v)| (JUMP_KEYS[i], v));
            let fixed = [
                ("password", secrets.password),
                ("ssh", secrets.ssh),
                ("proxy", secrets.proxy),
                ("tls-key", secrets.tls_key),
            ];
            for (what, value) in fixed.into_iter().chain(jumps) {
                match value {
                    Some(v) if v.is_empty() => store.delete(&secret_key(&id, what))?,
                    Some(v) => store.set(&secret_key(&id, what), &v)?,
                    None => {}
                }
            }
            Ok(())
        })
        .await
        .map_err(|e| CoreError::Secrets(e.to_string()))?
    }

    pub async fn delete_connection(&self, id: &str) -> Result<()> {
        self.store(|s| s.delete_connection(id))?;
        self.wipe_secrets(id).await
    }

    pub fn trash_connection(&self, id: &str) -> Result<()> {
        let production = self.production_name(id)?;
        self.store(|s| s.delete_connection(id))?;
        Self::lock(&self.trashed)?.insert(id.to_owned(), production);
        Ok(())
    }

    pub fn restore_connection(&self, profile: &ConnectionProfile) -> Result<()> {
        self.restore_connection_confirmed(profile, None)
    }

    pub fn restore_connection_confirmed(
        &self,
        profile: &ConnectionProfile,
        confirm: Option<&str>,
    ) -> Result<()> {
        let lowered = self.admit_environment(profile, confirm)?;
        self.store(|s| s.save_connection(profile))?;
        if let Some(line) = lowered {
            self.audit_as(&profile.id, String::new(), &line, None, None);
        }
        Self::lock(&self.trashed)?.remove(&profile.id);
        Ok(())
    }

    pub async fn purge_connection(&self, id: &str) -> Result<()> {
        if Self::lock(&self.trashed)?.remove(id).is_none() {
            return Ok(());
        }
        self.wipe_secrets(id).await
    }

    pub fn purge_all_trashed(&self) {
        let ids: Vec<String> = match Self::lock(&self.trashed) {
            Ok(mut t) => t.drain().map(|(id, _)| id).collect(),
            Err(_) => return,
        };
        for id in ids {
            for what in secret_names() {
                if let Err(e) = self.secrets.delete(&secret_key(&id, &what)) {
                    tracing::warn!(error = %e, "trashed secret not removed");
                }
            }
        }
    }

    async fn wipe_secrets(&self, id: &str) -> Result<()> {
        let store = self.secrets.clone();
        let id = id.to_owned();
        tokio::task::spawn_blocking(move || -> Result<()> {
            for what in secret_names() {
                store.delete(&secret_key(&id, &what))?;
            }
            Ok(())
        })
        .await
        .map_err(|e| CoreError::Secrets(e.to_string()))?
    }

    pub async fn open_session(&self, connection_id: &str) -> Result<SessionInfo> {
        self.open_session_in(connection_id, None).await
    }

    pub async fn open_session_in(
        &self,
        connection_id: &str,
        database: Option<&str>,
    ) -> Result<SessionInfo> {
        let mut profile = self.profile(connection_id)?;
        if let Some(db) = database {
            profile.target = profile
                .target
                .with_database(db)
                .ok_or(CoreError::SingleDatabase)?;
        }
        let (conn, tunnel) = self.dial(&profile, Secrets::default()).await?;
        self.register(profile, conn, tunnel, None, CommitMode::Auto)
            .await
    }

    pub async fn switch_database(&self, session_id: &str, database: &str) -> Result<SessionInfo> {
        let from = self.session(session_id)?;
        let mut profile = self.profile(&from.connection_id)?;
        profile.target = profile
            .target
            .with_database(database)
            .ok_or(CoreError::SingleDatabase)?;
        let creds = self.creds(&profile.id, &profile.target).await?;
        let tunnel = from.tunnel.clone();
        let conn = connect_target(&profile.target, &creds, tunnel.as_deref()).await?;
        self.register(profile, conn, tunnel, None, CommitMode::Auto)
            .await
    }

    pub async fn reconnect(&self, session_id: &str) -> Result<SessionInfo> {
        let _one = self.reconnecting.lock().await;
        let (connection_id, target, tunnel, mode, lost_tx) = match self.session(session_id) {
            Ok(s) if !s.lost.load(Ordering::Acquire) => return Ok(s.info(session_id)),
            Ok(s) => (
                s.connection_id.clone(),
                s.target.clone(),
                s.tunnel.clone(),
                s.commit_mode(),
                s.tx_open() || s.tx_lost.load(Ordering::Acquire),
            ),
            Err(_) => {
                let d = Self::lock(&self.dormant)?
                    .get(session_id)
                    .cloned()
                    .ok_or_else(|| CoreError::NotFound(session_id.to_owned()))?;
                (d.connection_id, d.target, None, d.mode, d.transaction_lost)
            }
        };
        let mut profile = self.profile(&connection_id)?;
        profile.target = target;
        let (conn, tunnel) = self.redial(&profile, tunnel).await?;
        let mut info = self
            .register(profile, conn, tunnel, Some(session_id.to_owned()), mode)
            .await?;
        Self::lock(&self.dormant)?.remove(session_id);
        info.transaction_lost = lost_tx;
        Ok(info)
    }

    async fn redial(
        &self,
        profile: &ConnectionProfile,
        tunnel: Option<Arc<Tunnel>>,
    ) -> Result<(Box<dyn Connection>, Option<Arc<Tunnel>>)> {
        if let Some(tn) = tunnel {
            let creds = self.creds(&profile.id, &profile.target).await?;
            match connect_target(&profile.target, &creds, Some(&tn)).await {
                Ok(conn) => return Ok((conn, Some(tn))),
                Err(e) => tracing::debug!(error = %e, "tunnel no longer carries the session"),
            }
        }
        self.dial(profile, Secrets::default()).await
    }

    pub(crate) async fn live(&self, session_id: &str) -> Result<Arc<Session>> {
        if let Ok(s) = self.session(session_id)
            && !s.lost.load(Ordering::Acquire)
        {
            return Ok(s);
        }
        let info = self.reconnect(session_id).await?;
        let s = self.session(session_id)?;
        if info.transaction_lost {
            s.tx_lost.store(true, Ordering::Release);
        }
        Ok(s)
    }

    pub(crate) fn profile(&self, connection_id: &str) -> Result<ConnectionProfile> {
        let p = self
            .store(|s| s.connection(connection_id))?
            .ok_or_else(|| CoreError::NotFound(connection_id.to_owned()))?;
        if p.template == Template::Model {
            return Err(CoreError::Model);
        }
        self.resolved(&p)
    }

    pub(crate) fn resolved(&self, p: &ConnectionProfile) -> Result<ConnectionProfile> {
        let vars = self.store(|s| s.variables_for(Some(&p.id), p.folder.as_deref()))?;
        nd_store::resolve(p, &nd_store::lookup_in(&vars)).map_err(CoreError::Variable)
    }

    pub fn preview_vars(
        &self,
        connection_id: Option<&str>,
        folder: Option<&str>,
        texts: &[String],
    ) -> Result<Vec<VarPreview>> {
        let vars = self.store(|s| s.variables_for(connection_id, folder))?;
        let lookup = nd_store::lookup_in(&vars);
        Ok(texts
            .iter()
            .map(|t| match nd_store::expand(t, "", &lookup) {
                Ok(text) => VarPreview::Value { text },
                Err(error) => VarPreview::Problem { error },
            })
            .collect())
    }

    async fn register(
        &self,
        profile: ConnectionProfile,
        conn: Box<dyn Connection>,
        tunnel: Option<Arc<Tunnel>>,
        id: Option<String>,
        mode: CommitMode,
    ) -> Result<SessionInfo> {
        let production = profile.environment == Environment::Production;
        let open = production && self.preferences().safety.unlock_mode == UnlockMode::Open;
        if production
            && !open
            && let Some(sql) = conn.dialect().and_then(|d| nd_sql::read_only(d, true))
        {
            crate::guard::quiet(conn.as_ref(), sql).await?;
        }
        initialize(conn.as_ref(), &profile.session.init).await?;
        let info = SessionInfo {
            session_id: id.unwrap_or_else(|| uuid::Uuid::new_v4().to_string()),
            connection_id: profile.id.clone(),
            dialect: conn.dialect(),
            caps: conn.capabilities(),
            server: conn.server().clone(),
            environment: profile.environment,
            transaction_lost: false,
            edition: conn.edition().cloned(),
        };
        let session = Session {
            connection_id: profile.id,
            profile: Mutex::new(Profile {
                name: profile.name,
                environment: profile.environment,
                options: profile.session,
            }),
            caps: info.caps.clone(),
            conn,
            busy: AtomicBool::new(false),
            unlocked_until: Mutex::new(open.then(|| Instant::now() + crate::guard::UNTIMED_FOR)),
            server_read_only: AtomicBool::new(production && !open),
            untimed: AtomicBool::new(open),
            in_transaction: AtomicBool::new(false),
            tx: Mutex::new(crate::tx::Tx {
                mode,
                ..crate::tx::Tx::default()
            }),
            tx_lost: AtomicBool::new(false),
            side: tokio::sync::Mutex::new(None),
            target: profile.target,
            tunnel,
            last_used: Mutex::new(Instant::now()),
            last_ping: Mutex::new(Instant::now()),
            lost: AtomicBool::new(false),
            retry: Mutex::new((Instant::now(), RETRY_FIRST)),
        };
        Self::lock(&self.sessions)?.insert(info.session_id.clone(), Arc::new(session));
        Ok(info)
    }

    pub fn close_session(&self, session_id: &str) -> Result<()> {
        Self::lock(&self.sessions)?.remove(session_id);
        Self::lock(&self.dormant)?.remove(session_id);
        Ok(())
    }

    pub(crate) fn session(&self, session_id: &str) -> Result<Arc<Session>> {
        Self::lock(&self.sessions)?
            .get(session_id)
            .cloned()
            .ok_or_else(|| CoreError::NotFound(session_id.to_owned()))
    }

    pub async fn schema(&self, session_id: &str) -> Result<Schema> {
        Ok(self.live(session_id).await?.conn.schema().await?)
    }

    pub async fn schema_open(&self, session_id: &str) -> Result<(Schema, Vec<NamespaceSize>)> {
        let session = self.live(session_id).await?;
        let conn = session.conn.as_ref();
        let Some(outline) = conn.outline().await? else {
            return Ok((conn.schema().await?, Vec::new()));
        };
        let total: u64 = outline.namespaces.iter().map(|n| n.relations).sum();
        if total < LAZY_FROM || outline.namespaces.len() < 2 {
            return Ok((conn.schema().await?, Vec::new()));
        }
        let first = outline
            .namespaces
            .iter()
            .find(|n| n.relations > 0)
            .or(outline.namespaces.first())
            .map(|n| n.name.clone())
            .unwrap_or_default();
        let mut read = conn.schema_of(std::slice::from_ref(&first)).await?;
        let mut namespaces = Vec::with_capacity(outline.namespaces.len());
        let mut pending = Vec::new();
        for size in outline.namespaces {
            if let Some(at) = read.namespaces.iter().position(|n| n.name == size.name) {
                namespaces.push(read.namespaces.swap_remove(at));
            } else {
                namespaces.push(Namespace::new(size.name.clone()));
                pending.push(size);
            }
        }
        Ok((
            Schema {
                databases: outline.databases,
                current_database: outline.current_database,
                namespaces,
            },
            pending,
        ))
    }

    pub async fn schema_namespace(&self, session_id: &str, namespace: &str) -> Result<Schema> {
        let session = self.live(session_id).await?;
        Ok(session.conn.schema_of(&[namespace.to_owned()]).await?)
    }

    pub async fn search_path(&self, session_id: &str) -> Result<Option<Vec<String>>> {
        let session = self.session(session_id)?;
        if session.conn.dialect() != Some(Dialect::Postgres) {
            return Ok(None);
        }
        let Ok(_busy) = crate::guard::claim(&session) else {
            return Ok(None);
        };
        let read = crate::guard::collect(
            session.conn.as_ref(),
            "SELECT unnest(current_schemas(false))::text",
            None,
        )
        .await;
        Ok(read.ok().map(|(rows, _)| {
            rows.into_iter()
                .filter_map(|r| r.into_iter().next().flatten())
                .collect()
        }))
    }

    pub async fn ddl(&self, session_id: &str, object: &ObjectRef) -> Result<String> {
        Ok(self.session(session_id)?.conn.ddl(object).await?)
    }

    pub async fn dependencies(
        &self,
        session_id: &str,
        object: &ObjectRef,
    ) -> Result<Vec<Dependency>> {
        Ok(self.session(session_id)?.conn.dependencies(object).await?)
    }
}
