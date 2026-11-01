mod catalog;
mod clickhouse;
mod cockroach;
mod diagnose;
mod duckdb;
mod expr;
mod finding;
mod firebird;
mod model;
mod mongo;
mod mssql;
mod mysql_json;
mod mysql_tree;
mod oracle;
mod parented;
mod pg;
mod sqlite;
mod sqltext;
mod tidb;
mod wide;

pub use catalog::{Catalog, ColumnFacts, TableFacts};
pub use diagnose::diagnose;
pub use finding::{Finding, Rule, Severity};
pub use model::{Buffers, Detail, Engine, Node, NodeKind, Plan, Spill, SpillKind, Tree};
pub use mongo::diagnose as diagnose_mongo;
pub use sqltext::{annotate, strip_explain};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    PostgresJson,
    MysqlTree,
    MysqlJson,
    MariadbJson,
    SqliteQueryPlan,
    MssqlXml,
    DuckdbJson,
    CockroachText,
    ClickhouseJson,
    MongoJson,
    CassandraTrace,
    ElasticProfile,
    TidbTable,
    OracleXplan,
    FirebirdPlan,
    HanaPlanTable,
    Db2Explain,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum PlanError {
    #[error("the plan is empty")]
    Empty,
    #[error("the plan is not valid JSON: {0}")]
    Json(String),
    #[error("the plan has an unexpected shape: {0}")]
    Shape(&'static str),
}

pub type Result<T> = std::result::Result<T, PlanError>;

pub fn parse(format: Format, raw: &str) -> Result<Plan> {
    if raw.trim().is_empty() {
        return Err(PlanError::Empty);
    }
    match format {
        Format::PostgresJson => pg::parse(raw),
        Format::MysqlTree => mysql_tree::parse(raw),
        Format::MysqlJson => mysql_json::parse(raw, Engine::Mysql),
        Format::MariadbJson => mysql_json::parse(raw, Engine::Mariadb),
        Format::SqliteQueryPlan => sqlite::parse(raw),
        Format::MssqlXml => mssql::parse(raw),
        Format::DuckdbJson => duckdb::parse(raw),
        Format::CockroachText => cockroach::parse(raw),
        Format::ClickhouseJson => clickhouse::parse(raw),
        Format::MongoJson => mongo::parse(raw),
        Format::CassandraTrace => wide::parse_trace(raw),
        Format::ElasticProfile => wide::parse_profile(raw),
        Format::TidbTable => tidb::parse(raw),
        Format::OracleXplan => oracle::parse(raw),
        Format::FirebirdPlan => firebird::parse(raw),
        Format::HanaPlanTable => parented::parse(Engine::Hana, raw),
        Format::Db2Explain => parented::parse(Engine::Db2, raw),
    }
}
