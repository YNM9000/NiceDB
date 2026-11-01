use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "lowercase")]
pub enum Flavor {
    Timescale,
    Neon,
    Supabase,
    Yugabyte,
    Tidb,
    Planetscale,
}

impl Flavor {
    pub const POSTGRES: [Self; 4] = [Self::Timescale, Self::Neon, Self::Supabase, Self::Yugabyte];
    pub const MYSQL: [Self; 2] = [Self::Tidb, Self::Planetscale];

    pub fn product(self) -> &'static str {
        match self {
            Self::Timescale => "TimescaleDB",
            Self::Neon => "Neon",
            Self::Supabase => "Supabase",
            Self::Yugabyte => "YugabyteDB",
            Self::Tidb => "TiDB",
            Self::Planetscale => "PlanetScale",
        }
    }

    pub fn cloud(self) -> bool {
        matches!(self, Self::Neon | Self::Supabase | Self::Planetscale)
    }

    pub fn postgres(self) -> bool {
        Self::POSTGRES.contains(&self)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
pub enum Fact {
    Base(String),
    Endpoint(String),
    Branch(String),
    Project(String),
    Region(String),
    Pooled(Pooler),
    Nodes(u32),
    Store(String),
    Extension { name: String, version: String },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "snake_case")]
pub enum Pooler {
    Transaction,
    Session,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "snake_case")]
pub enum Limit {
    PooledSession,
    ScaleToZero,
    ManagedSchemas,
    ForeignKeysOptional,
    DeployRequests,
    NoRoutines,
    DistributedLocks,
    OwnDumpTool,
    HypertableDump,
    DashboardUsers,
    OwnPlans,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
pub struct Edition {
    pub flavor: Flavor,
    pub version: Option<String>,
    pub facts: Vec<Fact>,
    pub limits: Vec<Limit>,
}

impl Edition {
    pub fn shown_version(&self) -> Option<String> {
        self.version.clone().or_else(|| {
            self.facts.iter().find_map(|f| match f {
                Fact::Base(b) => Some(b.clone()),
                _ => None,
            })
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, specta::Type)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum RelationTag {
    Hypertable { chunks: u32, compressed: u32 },
    ContinuousAggregate,
    Rls { policies: u32 },
}

fn neon_endpoint(host: &str) -> Option<(String, bool, Option<String>)> {
    let host = host.to_ascii_lowercase();
    let rest = host.strip_suffix(".neon.tech")?;
    let mut parts = rest.split('.');
    let first = parts.next()?;
    let (endpoint, pooled) = match first.strip_suffix("-pooler") {
        Some(e) => (e, true),
        None => (first, false),
    };
    let region = parts
        .find(|p| !matches!(*p, "aws" | "azure" | "gcp"))
        .map(str::to_owned);
    Some((endpoint.to_owned(), pooled, region))
}

fn supabase_project(host: &str, user: &str, port: u16) -> Option<(Option<String>, Vec<Fact>)> {
    let host = host.to_ascii_lowercase();
    if let Some(rest) = host.strip_suffix(".supabase.co") {
        let project = rest.strip_prefix("db.").unwrap_or(rest);
        return Some((Some(project.to_owned()), Vec::new()));
    }
    let region = host.strip_suffix(".pooler.supabase.com")?;
    let region = region
        .strip_prefix("aws-0-")
        .or_else(|| region.strip_prefix("aws-1-"))
        .unwrap_or(region);
    let project = user.split_once('.').map(|(_, p)| p.to_owned());
    let mode = if port == 6543 {
        Pooler::Transaction
    } else {
        Pooler::Session
    };
    Some((
        project,
        vec![Fact::Region(region.to_owned()), Fact::Pooled(mode)],
    ))
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PostgresClues {
    pub host: String,
    pub port: u16,
    pub user: String,
    pub banner: String,
    pub server_version: String,
    pub extensions: Vec<(String, String)>,
    pub schemas: Vec<String>,
    pub neon_timeline: String,
    pub yb_nodes: u32,
}

impl PostgresClues {
    fn extension(&self, name: &str) -> Option<&str> {
        self.extensions
            .iter()
            .find(|(n, _)| n == name)
            .map(|(_, v)| v.as_str())
    }
}

pub fn postgres_edition(clues: &PostgresClues, chosen: Option<Flavor>) -> Option<Edition> {
    let neon = neon_endpoint(&clues.host);
    let supabase = supabase_project(&clues.host, &clues.user, clues.port);
    let yb_version = clues.banner.split_whitespace().nth(1).and_then(|v| {
        v.split_once("-YB-")
            .map(|(_, yb)| yb.split("-b").next().unwrap_or(yb).to_owned())
    });
    let detected = if yb_version.is_some() || clues.yb_nodes > 0 {
        Some(Flavor::Yugabyte)
    } else if neon.is_some() || !clues.neon_timeline.is_empty() || clues.extension("neon").is_some()
    {
        Some(Flavor::Neon)
    } else if supabase.is_some()
        || (clues.schemas.iter().any(|s| s == "auth")
            && clues.schemas.iter().any(|s| s == "storage")
            && clues
                .extensions
                .iter()
                .any(|(n, _)| n == "supabase_vault" || n == "pg_graphql"))
    {
        Some(Flavor::Supabase)
    } else if clues.extension("timescaledb").is_some() {
        Some(Flavor::Timescale)
    } else {
        None
    };
    let flavor = detected.or(chosen.filter(|f| f.postgres()))?;
    let base = Fact::Base(format!(
        "PostgreSQL {}",
        clues
            .server_version
            .split(['-', ' '])
            .next()
            .unwrap_or(&clues.server_version)
    ));
    let mut facts = vec![base];
    let mut limits = Vec::new();
    let mut version = None;
    match flavor {
        Flavor::Timescale => {
            version = clues.extension("timescaledb").map(str::to_owned);
            limits.push(Limit::HypertableDump);
        }
        Flavor::Yugabyte => {
            version = yb_version;
            if clues.yb_nodes > 0 {
                facts.push(Fact::Nodes(clues.yb_nodes));
            }
            limits.extend([Limit::DistributedLocks, Limit::OwnDumpTool]);
        }
        Flavor::Neon => {
            if let Some((endpoint, pooled, region)) = neon {
                facts.push(Fact::Endpoint(endpoint));
                if let Some(r) = region {
                    facts.push(Fact::Region(r));
                }
                if pooled {
                    facts.push(Fact::Pooled(Pooler::Transaction));
                    limits.push(Limit::PooledSession);
                }
            }
            if !clues.neon_timeline.is_empty() {
                facts.push(Fact::Branch(clues.neon_timeline.clone()));
            }
            limits.push(Limit::ScaleToZero);
        }
        Flavor::Supabase => {
            if let Some((project, more)) = supabase {
                if let Some(p) = project {
                    facts.push(Fact::Project(p));
                }
                if more.contains(&Fact::Pooled(Pooler::Transaction)) {
                    limits.push(Limit::PooledSession);
                }
                facts.extend(more);
            }
            limits.push(Limit::ManagedSchemas);
        }
        Flavor::Tidb | Flavor::Planetscale => {}
    }
    for name in ["timescaledb", "postgis", "vector", "pg_graphql", "pg_cron"] {
        if let Some(v) = clues.extension(name)
            && !(flavor == Flavor::Timescale && name == "timescaledb")
        {
            facts.push(Fact::Extension {
                name: name.to_owned(),
                version: v.to_owned(),
            });
        }
    }
    Some(Edition {
        flavor,
        version,
        facts,
        limits,
    })
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MysqlClues {
    pub host: String,
    pub version: String,
    pub store: String,
    pub nodes: u32,
}

pub fn mysql_edition(clues: &MysqlClues, chosen: Option<Flavor>) -> Option<Edition> {
    let lower = clues.version.to_ascii_lowercase();
    let host = clues.host.to_ascii_lowercase();
    let detected = if lower.contains("tidb") {
        Some(Flavor::Tidb)
    } else if lower.contains("vitess")
        || lower.contains("planetscale")
        || host.ends_with(".psdb.cloud")
    {
        Some(Flavor::Planetscale)
    } else {
        None
    };
    let flavor = detected.or(chosen.filter(|f| !f.postgres()))?;
    let base = clues
        .version
        .split('-')
        .next()
        .unwrap_or(&clues.version)
        .to_owned();
    let mut facts = vec![Fact::Base(format!("MySQL {base}"))];
    let mut version = None;
    let limits = if flavor == Flavor::Tidb {
        version = clues
            .version
            .split('-')
            .find(|p| {
                p.strip_prefix('v')
                    .is_some_and(|n| n.starts_with(|c: char| c.is_ascii_digit()))
            })
            .map(str::to_owned);
        if clues.nodes > 0 {
            facts.push(Fact::Nodes(clues.nodes));
        }
        vec![Limit::DistributedLocks, Limit::OwnDumpTool, Limit::OwnPlans]
    } else {
        let region = host
            .strip_suffix(".connect.psdb.cloud")
            .filter(|r| *r != "aws" && *r != "gcp")
            .map(str::to_owned);
        if let Some(r) = region {
            facts.push(Fact::Region(r));
        }
        vec![
            Limit::ForeignKeysOptional,
            Limit::DeployRequests,
            Limit::NoRoutines,
            Limit::DashboardUsers,
        ]
    };
    if !clues.store.is_empty() {
        facts.push(Fact::Store(clues.store.clone()));
    }
    Some(Edition {
        flavor,
        version,
        facts,
        limits,
    })
}
