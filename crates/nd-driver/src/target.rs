use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(tag = "driver", rename_all = "lowercase")]
pub enum Target {
    Postgres(PostgresTarget),
    Sqlite(SqliteTarget),
    Mysql(MysqlTarget),
    Sqlserver(MssqlTarget),
    Cockroach(PostgresTarget),
    Duckdb(DuckdbTarget),
    Libsql(LibsqlTarget),
    Mongodb(MongoTarget),
    Clickhouse(ClickhouseTarget),
    Redis(RedisTarget),
    Cassandra(CassandraTarget),
    Dynamodb(DynamodbTarget),
    Elasticsearch(ElasticTarget),
    Firebird(FirebirdTarget),
    Oracle(OracleTarget),
    Db2(Db2Target),
    Hana(HanaTarget),
    Odbc(OdbcTarget),
}

impl Target {
    pub fn dialect(&self) -> Option<nd_sql::Dialect> {
        Some(match self {
            Self::Postgres(_) | Self::Cockroach(_) => nd_sql::Dialect::Postgres,
            Self::Sqlite(_) | Self::Libsql(_) => nd_sql::Dialect::Sqlite,
            Self::Mysql(_) => nd_sql::Dialect::Mysql,
            Self::Sqlserver(_) => nd_sql::Dialect::Mssql,
            Self::Duckdb(_) => nd_sql::Dialect::Duckdb,
            Self::Clickhouse(_) => nd_sql::Dialect::Clickhouse,
            Self::Firebird(_) => nd_sql::Dialect::Firebird,
            Self::Oracle(_) => nd_sql::Dialect::Oracle,
            Self::Db2(_) => nd_sql::Dialect::Db2,
            Self::Hana(_) => nd_sql::Dialect::Hana,
            Self::Odbc(_) => nd_sql::Dialect::Ansi,
            Self::Mongodb(_)
            | Self::Redis(_)
            | Self::Cassandra(_)
            | Self::Dynamodb(_)
            | Self::Elasticsearch(_) => return None,
        })
    }

    pub fn ssh(&self) -> Option<&SshConfig> {
        match self {
            Self::Postgres(p) | Self::Cockroach(p) => p.ssh.as_ref(),
            Self::Mysql(m) => m.ssh.as_ref(),
            Self::Sqlserver(m) => m.ssh.as_ref(),
            Self::Mongodb(m) => m.ssh.as_ref(),
            Self::Clickhouse(c) => c.ssh.as_ref(),
            Self::Redis(r) => r.ssh.as_ref(),
            Self::Cassandra(c) => c.ssh.as_ref(),
            Self::Elasticsearch(e) => e.ssh.as_ref(),
            Self::Firebird(f) => f.ssh.as_ref(),
            Self::Oracle(o) => o.ssh.as_ref(),
            Self::Db2(d) => d.ssh.as_ref(),
            Self::Hana(h) => h.ssh.as_ref(),
            Self::Sqlite(_)
            | Self::Duckdb(_)
            | Self::Libsql(_)
            | Self::Dynamodb(_)
            | Self::Odbc(_) => None,
        }
    }

    pub fn proxy(&self) -> Option<&ProxyConfig> {
        match self {
            Self::Postgres(p) | Self::Cockroach(p) => p.proxy.as_ref(),
            Self::Mysql(m) => m.proxy.as_ref(),
            Self::Sqlserver(m) => m.proxy.as_ref(),
            Self::Mongodb(m) => m.proxy.as_ref(),
            Self::Clickhouse(c) => c.proxy.as_ref(),
            Self::Redis(r) => r.proxy.as_ref(),
            Self::Cassandra(c) => c.proxy.as_ref(),
            Self::Elasticsearch(e) => e.proxy.as_ref(),
            Self::Firebird(f) => f.proxy.as_ref(),
            Self::Libsql(LibsqlTarget::Remote { proxy, .. }) => proxy.as_ref(),
            Self::Oracle(o) => o.proxy.as_ref(),
            Self::Db2(d) => d.proxy.as_ref(),
            Self::Hana(h) => h.proxy.as_ref(),
            Self::Sqlite(_)
            | Self::Duckdb(_)
            | Self::Libsql(_)
            | Self::Dynamodb(_)
            | Self::Odbc(_) => None,
        }
    }

    pub fn client_cert(&self) -> Option<&ClientCert> {
        match self {
            Self::Postgres(p) | Self::Cockroach(p) => p.client_cert.as_ref(),
            Self::Mysql(m) => m.client_cert.as_ref(),
            Self::Sqlserver(m) => m.client_cert.as_ref(),
            Self::Mongodb(m) => m.client_cert.as_ref(),
            Self::Clickhouse(c) => c.client_cert.as_ref(),
            Self::Redis(r) => r.client_cert.as_ref(),
            Self::Cassandra(c) => c.client_cert.as_ref(),
            Self::Elasticsearch(e) => e.client_cert.as_ref(),
            Self::Sqlite(_)
            | Self::Duckdb(_)
            | Self::Libsql(_)
            | Self::Dynamodb(_)
            | Self::Firebird(_)
            | Self::Oracle(_)
            | Self::Db2(_)
            | Self::Hana(_)
            | Self::Odbc(_) => None,
        }
    }
}

impl Target {
    pub fn remote(&self) -> Option<(&str, u16)> {
        match self {
            Self::Postgres(p) | Self::Cockroach(p) => Some((&p.host, p.port)),
            Self::Mysql(m) => Some((&m.host, m.port)),
            Self::Sqlserver(m) => Some((&m.host, m.port)),
            Self::Mongodb(m) if m.srv => None,
            Self::Mongodb(m) => Some((&m.host, m.port)),
            Self::Clickhouse(c) => Some((&c.host, c.port)),
            Self::Redis(r) => Some((&r.host, r.port)),
            Self::Cassandra(c) => Some((c.first_host(), c.port)),
            Self::Elasticsearch(e) => Some((&e.host, e.port)),
            Self::Firebird(f) => Some((&f.host, f.port)),
            Self::Oracle(o) if !o.connect_string.trim().is_empty() => None,
            Self::Oracle(o) => Some((&o.host, o.port)),
            Self::Db2(d) => Some((&d.host, d.port)),
            Self::Hana(h) => Some((&h.host, h.port)),
            Self::Libsql(l) => l.endpoint(),
            Self::Sqlite(_) | Self::Duckdb(_) | Self::Dynamodb(_) | Self::Odbc(_) => None,
        }
    }

    pub fn with_database(&self, database: &str) -> Option<Self> {
        let mut next = self.clone();
        match &mut next {
            Self::Postgres(p) | Self::Cockroach(p) => database.clone_into(&mut p.database),
            Self::Mysql(m) => database.clone_into(&mut m.database),
            Self::Sqlserver(m) => database.clone_into(&mut m.database),
            Self::Mongodb(m) => database.clone_into(&mut m.database),
            Self::Clickhouse(c) => database.clone_into(&mut c.database),
            Self::Redis(r) => r.database = RedisTarget::index(database)?,
            Self::Cassandra(c) => database.clone_into(&mut c.database),
            Self::Firebird(f) => database.clone_into(&mut f.database),
            Self::Sqlite(_)
            | Self::Duckdb(_)
            | Self::Libsql(_)
            | Self::Dynamodb(_)
            | Self::Elasticsearch(_)
            | Self::Oracle(_)
            | Self::Db2(_)
            | Self::Hana(_)
            | Self::Odbc(_) => return None,
        }
        Some(next)
    }

    pub fn needs_password(&self) -> bool {
        match self {
            Self::Sqlite(_) | Self::Duckdb(_) => false,
            Self::Mongodb(m) => !m.user.is_empty(),
            Self::Clickhouse(c) => !c.user.is_empty(),
            Self::Cassandra(c) => !c.user.is_empty(),
            Self::Elasticsearch(e) => !e.user.is_empty(),
            Self::Dynamodb(d) => d.profile.trim().is_empty() && !d.user.is_empty(),
            Self::Odbc(o) => !o.user.is_empty(),
            _ => true,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
pub struct OracleTarget {
    pub host: String,
    pub port: u16,
    pub database: String,
    #[serde(default)]
    pub sid: bool,
    #[serde(default)]
    pub connect_string: String,
    pub user: String,
    pub tls: TlsMode,
    pub ca_file: Option<String>,
    #[serde(default)]
    pub role: OracleRole,
    pub ssh: Option<SshConfig>,
    #[serde(default)]
    pub proxy: Option<ProxyConfig>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "lowercase")]
pub enum OracleRole {
    #[default]
    Normal,
    Sysdba,
    Sysoper,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
pub struct Db2Target {
    pub host: String,
    pub port: u16,
    pub database: String,
    pub user: String,
    pub tls: TlsMode,
    pub ca_file: Option<String>,
    pub ssh: Option<SshConfig>,
    #[serde(default)]
    pub proxy: Option<ProxyConfig>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
pub struct HanaTarget {
    pub host: String,
    pub port: u16,
    #[serde(default)]
    pub database: String,
    pub user: String,
    pub tls: TlsMode,
    pub ca_file: Option<String>,
    pub ssh: Option<SshConfig>,
    #[serde(default)]
    pub proxy: Option<ProxyConfig>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
pub struct OdbcTarget {
    #[serde(default)]
    pub dsn: String,
    #[serde(default)]
    pub driver_path: String,
    #[serde(default)]
    pub connect_string: String,
    #[serde(default)]
    pub user: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
pub struct FirebirdTarget {
    pub host: String,
    pub port: u16,
    pub database: String,
    pub user: String,
    #[serde(default)]
    pub role: Option<String>,
    pub ssh: Option<SshConfig>,
    #[serde(default)]
    pub proxy: Option<ProxyConfig>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
pub struct PostgresTarget {
    pub host: String,
    pub port: u16,
    pub database: String,
    pub user: String,
    pub tls: TlsMode,
    pub ca_file: Option<String>,
    pub ssh: Option<SshConfig>,
    #[serde(default)]
    pub proxy: Option<ProxyConfig>,
    #[serde(default)]
    pub client_cert: Option<ClientCert>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[specta(optional)]
    pub flavor: Option<crate::Flavor>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
pub struct MysqlTarget {
    pub host: String,
    pub port: u16,
    pub database: String,
    pub user: String,
    pub tls: TlsMode,
    pub ca_file: Option<String>,
    pub ssh: Option<SshConfig>,
    #[serde(default)]
    pub proxy: Option<ProxyConfig>,
    #[serde(default)]
    pub client_cert: Option<ClientCert>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[specta(optional)]
    pub flavor: Option<crate::Flavor>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
pub struct MssqlTarget {
    pub host: String,
    pub port: u16,
    pub database: String,
    pub user: String,
    pub tls: TlsMode,
    pub ca_file: Option<String>,
    pub ssh: Option<SshConfig>,
    #[serde(default)]
    pub proxy: Option<ProxyConfig>,
    #[serde(default)]
    pub client_cert: Option<ClientCert>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
pub struct MongoTarget {
    pub host: String,
    pub port: u16,
    #[serde(default)]
    pub srv: bool,
    pub database: String,
    pub user: String,
    #[serde(default)]
    pub auth_source: String,
    #[serde(default)]
    pub options: String,
    pub tls: TlsMode,
    pub ca_file: Option<String>,
    pub ssh: Option<SshConfig>,
    #[serde(default)]
    pub proxy: Option<ProxyConfig>,
    #[serde(default)]
    pub client_cert: Option<ClientCert>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
pub struct ClickhouseTarget {
    pub host: String,
    pub port: u16,
    pub database: String,
    pub user: String,
    pub tls: TlsMode,
    pub ca_file: Option<String>,
    pub ssh: Option<SshConfig>,
    #[serde(default)]
    pub proxy: Option<ProxyConfig>,
    #[serde(default)]
    pub client_cert: Option<ClientCert>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
pub struct RedisTarget {
    pub host: String,
    pub port: u16,
    pub database: u16,
    #[serde(default)]
    pub user: String,
    pub tls: TlsMode,
    pub ca_file: Option<String>,
    pub ssh: Option<SshConfig>,
    #[serde(default)]
    pub proxy: Option<ProxyConfig>,
    #[serde(default)]
    pub client_cert: Option<ClientCert>,
    #[serde(default)]
    pub mode: RedisMode,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, specta::Type)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum RedisMode {
    #[default]
    Single,
    Cluster,
    Sentinel {
        master: String,
    },
}

impl RedisTarget {
    pub fn name(index: u16) -> String {
        format!("db{index}")
    }

    pub fn index(name: &str) -> Option<u16> {
        name.trim().trim_start_matches("db").parse().ok()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
pub struct CassandraTarget {
    pub host: String,
    pub port: u16,
    pub database: String,
    pub user: String,
    #[serde(default)]
    pub consistency: Consistency,
    #[serde(default)]
    pub datacenter: String,
    pub tls: TlsMode,
    pub ca_file: Option<String>,
    pub ssh: Option<SshConfig>,
    #[serde(default)]
    pub proxy: Option<ProxyConfig>,
    #[serde(default)]
    pub client_cert: Option<ClientCert>,
}

impl CassandraTarget {
    pub fn hosts(&self) -> impl Iterator<Item = &str> {
        self.host
            .split(',')
            .map(str::trim)
            .filter(|h| !h.is_empty())
    }

    pub fn first_host(&self) -> &str {
        self.hosts().next().unwrap_or_default()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "snake_case")]
pub enum Consistency {
    Any,
    One,
    Two,
    Three,
    Quorum,
    All,
    #[default]
    LocalQuorum,
    EachQuorum,
    LocalOne,
}

impl Consistency {
    pub const ALL: [Self; 9] = [
        Self::Any,
        Self::One,
        Self::Two,
        Self::Three,
        Self::Quorum,
        Self::All,
        Self::LocalQuorum,
        Self::EachQuorum,
        Self::LocalOne,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Self::Any => "ANY",
            Self::One => "ONE",
            Self::Two => "TWO",
            Self::Three => "THREE",
            Self::Quorum => "QUORUM",
            Self::All => "ALL",
            Self::LocalQuorum => "LOCAL_QUORUM",
            Self::EachQuorum => "EACH_QUORUM",
            Self::LocalOne => "LOCAL_ONE",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|c| c.name().eq_ignore_ascii_case(s.trim()))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
pub struct DynamodbTarget {
    pub region: String,
    #[serde(default)]
    pub endpoint: String,
    #[serde(default)]
    pub profile: String,
    #[serde(default)]
    pub user: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
pub struct ElasticTarget {
    pub host: String,
    pub port: u16,
    #[serde(default)]
    pub database: String,
    pub user: String,
    pub tls: TlsMode,
    pub ca_file: Option<String>,
    pub ssh: Option<SshConfig>,
    #[serde(default)]
    pub proxy: Option<ProxyConfig>,
    #[serde(default)]
    pub client_cert: Option<ClientCert>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
pub struct DuckdbTarget {
    pub path: String,
    pub read_only: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(tag = "location", rename_all = "lowercase")]
pub enum LibsqlTarget {
    File {
        path: String,
        read_only: bool,
    },
    Remote {
        url: String,
        #[serde(default)]
        proxy: Option<ProxyConfig>,
    },
}

impl LibsqlTarget {
    pub fn endpoint(&self) -> Option<(&str, u16)> {
        let Self::Remote { url, .. } = self else {
            return None;
        };
        let (scheme, rest) = url.trim().split_once("://")?;
        let fallback = match scheme.to_ascii_lowercase().as_str() {
            "http" | "ws" => 80,
            "libsql" | "https" | "wss" => 443,
            _ => return None,
        };
        let authority = rest.split(['/', '?', '#']).next()?;
        let authority = authority.rsplit_once('@').map_or(authority, |(_, a)| a);
        let (host, port) = match authority.strip_prefix('[') {
            Some(v6) => {
                let (host, after) = v6.split_once(']')?;
                (host, after.strip_prefix(':'))
            }
            None => match authority.rsplit_once(':') {
                Some((h, p)) => (h, Some(p)),
                None => (authority, None),
            },
        };
        let port = match port {
            Some(p) => p.parse().ok()?,
            None => fallback,
        };
        (!host.is_empty()).then_some((host, port))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
pub struct SqliteTarget {
    pub path: String,
    pub read_only: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "lowercase")]
pub enum TlsMode {
    Disable,
    #[default]
    Verify,
    Insecure,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
pub struct SshConfig {
    pub host: String,
    pub port: u16,
    pub user: String,
    pub auth: SshAuth,
    #[serde(default)]
    pub jumps: Vec<SshHop>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
pub struct SshHop {
    pub host: String,
    pub port: u16,
    pub user: String,
    pub auth: SshAuth,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
pub struct ProxyConfig {
    pub kind: ProxyKind,
    pub host: String,
    pub port: u16,
    pub user: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "lowercase")]
pub enum ProxyKind {
    Socks5,
    Http,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
pub struct ClientCert {
    pub cert: String,
    pub key: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(tag = "method", rename_all = "lowercase")]
pub enum SshAuth {
    Password,
    Key { path: String },
    Agent,
}
