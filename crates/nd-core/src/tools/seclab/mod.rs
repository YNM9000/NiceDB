mod accounts;
mod data;
mod network;
mod version;

use nd_driver::{Connection, Row};
use serde::Serialize;

use super::{Engine, Principals};
use crate::guard::{Access, collect};
use crate::{Core, CoreError, Result};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, specta::Type)]
#[serde(rename_all = "snake_case")]
pub enum SecSeverity {
    High,
    Medium,
    Low,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "snake_case")]
pub enum SecCategory {
    Accounts,
    Network,
    Version,
    Data,
    Hygiene,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "snake_case")]
pub enum SecVerdict {
    Found,
    Passed,
    Skipped,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "snake_case")]
pub enum SecSkip {
    Engine,
    Privilege,
    NotDetectable,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
pub struct SecSubject {
    pub name: String,
    pub detail: Option<String>,
}

#[derive(Debug, Clone, Serialize, specta::Type)]
pub struct SecFinding {
    pub id: String,
    pub category: SecCategory,
    pub verdict: SecVerdict,
    pub severity: Option<SecSeverity>,
    pub reason: Option<SecSkip>,
    pub subjects: Vec<SecSubject>,
    pub fix: Option<String>,
    pub note: Option<String>,
}

#[derive(Debug, Clone, Serialize, specta::Type)]
pub struct SecurityReport {
    pub product: String,
    pub version: String,
    pub database: String,
    pub checks: Vec<SecFinding>,
    pub versions_snapshot: String,
    pub versions_sources: Vec<String>,
}

pub(crate) enum Outcome {
    Passed,
    Found(Found),
    Skip(SecSkip),
}

pub(crate) struct Found {
    pub severity: SecSeverity,
    pub subjects: Vec<SecSubject>,
    pub fix: Option<String>,
    pub note: Option<String>,
}

pub(crate) fn found(severity: SecSeverity, subjects: Vec<SecSubject>, fix: Vec<String>) -> Outcome {
    Outcome::Found(Found {
        severity,
        subjects,
        fix: (!fix.is_empty()).then(|| fix.join("\n")),
        note: None,
    })
}

pub(crate) fn subject(name: impl Into<String>, detail: Option<String>) -> SecSubject {
    SecSubject {
        name: name.into(),
        detail,
    }
}

const POSTGRES: &[Engine] = &[Engine::Postgres];
const MYSQL: &[Engine] = &[Engine::Mysql, Engine::Mariadb];
const SERVERS: &[Engine] = &[
    Engine::Postgres,
    Engine::Cockroach,
    Engine::Mysql,
    Engine::Mariadb,
    Engine::Mssql,
    Engine::Clickhouse,
];
const NOT_COCKROACH: &[Engine] = &[
    Engine::Postgres,
    Engine::Mysql,
    Engine::Mariadb,
    Engine::Mssql,
    Engine::Clickhouse,
];
const PUBLIC_GRANTS: &[Engine] = &[Engine::Postgres, Engine::Cockroach, Engine::Mssql];
const VERSIONED: &[Engine] = &[
    Engine::Postgres,
    Engine::Mysql,
    Engine::Mariadb,
    Engine::Mssql,
    Engine::Sqlite,
];
const EVERY: &[Engine] = &[
    Engine::Postgres,
    Engine::Cockroach,
    Engine::Mysql,
    Engine::Mariadb,
    Engine::Mssql,
    Engine::Sqlite,
    Engine::Libsql,
    Engine::Duckdb,
    Engine::Clickhouse,
];
const BACKUPS: &[Engine] = SERVERS;

struct Check {
    id: &'static str,
    category: SecCategory,
    engines: &'static [Engine],
}

const CHECKS: &[Check] = &[
    Check {
        id: "superusers",
        category: SecCategory::Accounts,
        engines: SERVERS,
    },
    Check {
        id: "default_accounts",
        category: SecCategory::Accounts,
        engines: SERVERS,
    },
    Check {
        id: "anonymous_accounts",
        category: SecCategory::Accounts,
        engines: MYSQL,
    },
    Check {
        id: "weak_passwords",
        category: SecCategory::Accounts,
        engines: SERVERS,
    },
    Check {
        id: "public_grants",
        category: SecCategory::Accounts,
        engines: PUBLIC_GRANTS,
    },
    Check {
        id: "ddl_rights",
        category: SecCategory::Accounts,
        engines: SERVERS,
    },
    Check {
        id: "hba_trust",
        category: SecCategory::Network,
        engines: POSTGRES,
    },
    Check {
        id: "hba_md5",
        category: SecCategory::Network,
        engines: POSTGRES,
    },
    Check {
        id: "hba_open",
        category: SecCategory::Network,
        engines: POSTGRES,
    },
    Check {
        id: "ssl",
        category: SecCategory::Network,
        engines: NOT_COCKROACH,
    },
    Check {
        id: "listen_all",
        category: SecCategory::Network,
        engines: &[Engine::Postgres, Engine::Mysql, Engine::Mariadb],
    },
    Check {
        id: "server_version",
        category: SecCategory::Version,
        engines: VERSIONED,
    },
    Check {
        id: "sensitive_columns",
        category: SecCategory::Data,
        engines: EVERY,
    },
    Check {
        id: "logging",
        category: SecCategory::Hygiene,
        engines: NOT_COCKROACH,
    },
    Check {
        id: "backup_recent",
        category: SecCategory::Hygiene,
        engines: BACKUPS,
    },
    Check {
        id: "risky_features",
        category: SecCategory::Hygiene,
        engines: NOT_COCKROACH,
    },
];

pub(crate) async fn rows(conn: &dyn Connection, sql: &str) -> Option<Vec<Row>> {
    collect(conn, sql, None).await.ok().map(|(r, _)| r)
}

pub(crate) async fn scalar(conn: &dyn Connection, sql: &str) -> Option<String> {
    rows(conn, sql)
        .await?
        .first()
        .and_then(|r| r.first().cloned().flatten())
}

pub(crate) fn cell(r: &Row, i: usize) -> String {
    r.get(i).cloned().flatten().unwrap_or_default()
}

pub(crate) fn on(r: &Row, i: usize) -> bool {
    matches!(
        cell(r, i).to_ascii_lowercase().as_str(),
        "true" | "t" | "y" | "yes" | "on" | "1"
    )
}

pub(crate) struct Ctx<'a> {
    pub conn: &'a dyn Connection,
    pub engine: Engine,
    pub principals: Option<&'a Principals>,
}

impl Ctx<'_> {
    pub fn dialect(&self) -> nd_driver::Dialect {
        self.conn.dialect().unwrap_or(nd_driver::Dialect::Postgres)
    }

    pub fn q(&self, name: &str) -> String {
        nd_sql::ident(self.dialect(), name)
    }
}

async fn run(id: &str, cx: &Ctx<'_>) -> Outcome {
    match id {
        "superusers" => accounts::superusers(cx),
        "default_accounts" => accounts::default_accounts(cx).await,
        "anonymous_accounts" => accounts::anonymous(cx),
        "weak_passwords" => accounts::weak_passwords(cx).await,
        "public_grants" => accounts::public_grants(cx),
        "ddl_rights" => accounts::ddl_rights(cx),
        "hba_trust" | "hba_md5" | "hba_open" => network::hba(id, cx).await,
        "ssl" => network::ssl(cx).await,
        "listen_all" => network::listen_all(cx).await,
        "server_version" => version::check(cx),
        "sensitive_columns" => data::sensitive(cx).await,
        "logging" => network::logging(cx).await,
        "backup_recent" => network::backup(cx).await,
        "risky_features" => network::risky(cx).await,
        _ => Outcome::Skip(SecSkip::Engine),
    }
}

fn finish(check: &Check, outcome: Outcome) -> SecFinding {
    let mut f = SecFinding {
        id: check.id.into(),
        category: check.category,
        verdict: SecVerdict::Passed,
        severity: None,
        reason: None,
        subjects: Vec::new(),
        fix: None,
        note: None,
    };
    match outcome {
        Outcome::Passed => {}
        Outcome::Skip(reason) => {
            f.verdict = SecVerdict::Skipped;
            f.reason = Some(reason);
        }
        Outcome::Found(found) => {
            f.verdict = SecVerdict::Found;
            f.severity = Some(found.severity);
            f.subjects = found.subjects;
            f.fix = found.fix;
            f.note = found.note;
        }
    }
    f
}

impl Core {
    pub async fn security_check(&self, session_id: &str) -> Result<SecurityReport> {
        let session = self.session(session_id)?;
        self.authorize(&session, Access::Read)?;
        let principals = self.principals(session_id).await.ok();
        let side = self.side(&session).await?;
        let conn = side.conn()?;
        let engine = Engine::of(conn);
        let cx = Ctx {
            conn,
            engine,
            principals: principals.as_ref(),
        };
        let mut checks = Vec::with_capacity(CHECKS.len());
        for check in CHECKS {
            let outcome = if check.engines.contains(&engine) {
                run(check.id, &cx).await
            } else {
                Outcome::Skip(SecSkip::Engine)
            };
            checks.push(finish(check, outcome));
        }
        let server = conn.server();
        let list = version::bundled();
        Ok(SecurityReport {
            product: server.product.clone(),
            version: server.version.clone(),
            database: server.database.clone(),
            checks,
            versions_snapshot: list.snapshot.clone(),
            versions_sources: list
                .products
                .values()
                .flat_map(|p| p.sources.clone())
                .collect(),
        })
    }
}

impl Core {
    pub fn install_seclab_versions(
        &self,
        json: &str,
        signature: &str,
        keep: bool,
    ) -> Result<String> {
        let snapshot = version::install(json)
            .map_err(|message| CoreError::Driver(nd_driver::DriverError::Io { message }))?;
        if keep {
            self.store(|s| s.set_seclab_versions(json, signature))?;
        }
        Ok(snapshot)
    }

    pub fn saved_seclab_versions(&self) -> Option<(String, String)> {
        self.store(nd_store::Store::seclab_versions).ok().flatten()
    }
}
