mod catalog;
mod ddl;
mod deps;
mod edition;
mod family;
mod identity;
mod keys;
mod private;
mod schema;
mod target;

use std::sync::Arc;

pub use catalog::{CatalogRows, assemble as assemble_catalog, fk_action};
pub use ddl::{DdlKind, ObjectRef};
pub use deps::{
    DepDirection, DepKind, DepVia, Dependency, by_text, mentions, tidy, views_from_engine,
};
pub use edition::{
    Edition, Fact, Flavor, Limit, MysqlClues, Pooler, PostgresClues, RelationTag, mysql_edition,
    postgres_edition,
};
pub use family::{
    Cancel, Capabilities, CommandClass, Commands, Family, Group, Language, NamespaceLevel, Noun,
    ResultShape, SchemaKind, Tool, TreeShape, Vocabulary,
};
pub use identity::{Identity, KeyFile, key_encrypted, key_file, load_identity};
pub use keys::{
    KeyData, KeyEntry, KeyInfo, KeyPage, KeyRead, KeyScan, KeyValue, KeyWrite, Report, ReportGroup,
    ReportSection, command_line, quote_arg,
};
pub use nd_sql::{Dialect, Position, Risk, ident};
pub use private::PrivateDir;
pub use schema::{
    ColumnInfo, FkAction, ForeignKey, Function, Index, Namespace, NamespaceSize, Object, Outline,
    PackedNamespace, PackedRelation, PackedSchema, Relation, Schema, SchemaStart,
};
pub use target::{
    CassandraTarget, ClickhouseTarget, ClientCert, Consistency, Db2Target, DuckdbTarget,
    DynamodbTarget, ElasticTarget, FirebirdTarget, HanaTarget, LibsqlTarget, MongoTarget,
    MssqlTarget, MysqlTarget, OdbcTarget, OracleRole, OracleTarget, PostgresTarget, ProxyConfig,
    ProxyKind, RedisMode, RedisTarget, SqliteTarget, SshAuth, SshConfig, SshHop, Target, TlsMode,
};

use serde::Serialize;
use tokio::sync::mpsc;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
pub struct Column {
    pub name: String,
    pub type_name: Option<String>,
}

pub type Row = Vec<Option<String>>;

#[derive(Debug)]
pub enum Chunk {
    Columns(Vec<Column>),
    Rows(Vec<Row>),
    Documents(Vec<String>),
    Progress(QueryProgress),
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, specta::Type)]
pub struct QueryProgress {
    #[specta(type = f64)]
    pub read_rows: u64,
    #[specta(type = f64)]
    pub read_bytes: u64,
    #[specta(type = Option<f64>)]
    pub total_rows: Option<u64>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, specta::Type)]
pub struct Outcome {
    pub command: Option<String>,
    #[specta(type = Option<f64>)]
    pub rows_affected: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
pub struct ServerInfo {
    pub product: String,
    pub version: String,
    pub database: String,
}

#[derive(Debug, Clone, thiserror::Error, Serialize, specta::Type)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum DriverError {
    #[error("{message}")]
    Connect { message: String },
    #[error("{message}")]
    Auth { message: String },
    #[error("{message}")]
    Tls { message: String },
    #[error("{message}")]
    Query {
        message: String,
        code: Option<String>,
        detail: Option<String>,
        hint: Option<String>,
        offset: Option<u32>,
    },
    #[error("cancelled")]
    Cancelled,
    #[error("statement timed out after {seconds} s")]
    Timeout { seconds: u32 },
    #[error("{message}")]
    Io { message: String },
}

impl DriverError {
    pub fn query(message: impl Into<String>) -> Self {
        Self::Query {
            message: message.into(),
            code: None,
            detail: None,
            hint: None,
            offset: None,
        }
    }
}

pub type Result<T> = std::result::Result<T, DriverError>;

pub struct RowBatcher {
    out: mpsc::Sender<Chunk>,
    buf: Vec<Row>,
    limit: usize,
}

impl RowBatcher {
    const FIRST: usize = 200;
    const STEADY: usize = 4000;

    pub fn new(out: mpsc::Sender<Chunk>) -> Self {
        Self {
            out,
            buf: Vec::with_capacity(Self::FIRST),
            limit: Self::FIRST,
        }
    }

    pub async fn columns(&self, columns: Vec<Column>) -> Result<()> {
        self.out
            .send(Chunk::Columns(columns))
            .await
            .map_err(|_| DriverError::Cancelled)
    }

    pub async fn push(&mut self, row: Row) -> Result<()> {
        self.buf.push(row);
        if self.buf.len() >= self.limit {
            self.flush().await
        } else {
            Ok(())
        }
    }

    pub async fn flush(&mut self) -> Result<()> {
        if self.buf.is_empty() {
            return Ok(());
        }
        self.limit = Self::STEADY;
        let rows = std::mem::replace(&mut self.buf, Vec::with_capacity(Self::STEADY));
        self.out
            .send(Chunk::Rows(rows))
            .await
            .map_err(|_| DriverError::Cancelled)
    }

    pub fn columns_blocking(&self, columns: Vec<Column>) -> Result<()> {
        self.out
            .blocking_send(Chunk::Columns(columns))
            .map_err(|_| DriverError::Cancelled)
    }

    pub fn push_blocking(&mut self, row: Row) -> Result<()> {
        self.buf.push(row);
        if self.buf.len() >= self.limit {
            self.flush_blocking()
        } else {
            Ok(())
        }
    }

    pub fn flush_blocking(&mut self) -> Result<()> {
        if self.buf.is_empty() {
            return Ok(());
        }
        self.limit = Self::STEADY;
        let rows = std::mem::replace(&mut self.buf, Vec::with_capacity(Self::STEADY));
        self.out
            .blocking_send(Chunk::Rows(rows))
            .map_err(|_| DriverError::Cancelled)
    }
}

#[async_trait::async_trait]
pub trait Canceller: Send + Sync {
    async fn cancel(&self);
}

#[async_trait::async_trait]
pub trait Connection: Send + Sync {
    fn capabilities(&self) -> Capabilities;
    fn dialect(&self) -> Option<Dialect> {
        self.capabilities().dialect()
    }
    fn server(&self) -> &ServerInfo;
    fn edition(&self) -> Option<&Edition> {
        None
    }
    fn canceller(&self) -> Arc<dyn Canceller>;
    async fn execute(&self, sql: &str, out: mpsc::Sender<Chunk>) -> Result<Outcome>;
    async fn execute_bound(
        &self,
        sql: &str,
        params: &[Option<String>],
        out: mpsc::Sender<Chunk>,
    ) -> Result<Outcome>;
    async fn schema(&self) -> Result<Schema>;
    async fn outline(&self) -> Result<Option<Outline>> {
        Ok(None)
    }
    async fn schema_of(&self, namespaces: &[String]) -> Result<Schema> {
        let mut schema = self.schema().await?;
        schema.namespaces.retain(|n| namespaces.contains(&n.name));
        Ok(schema)
    }
    async fn relation(&self, namespace: &str, name: &str) -> Result<Option<Relation>> {
        let schema = self.schema().await?;
        Ok(schema
            .namespaces
            .into_iter()
            .find(|n| n.name == namespace)
            .and_then(|n| {
                n.tables
                    .into_iter()
                    .chain(n.views)
                    .chain(n.materialized_views)
                    .chain(n.dictionaries)
                    .chain(n.aliases)
                    .chain(n.data_streams)
                    .find(|t| t.name == name)
            }))
    }
    async fn ddl(&self, object: &ObjectRef) -> Result<String>;
    async fn dependencies(&self, object: &ObjectRef) -> Result<Vec<Dependency>>;
    fn commands(&self, text: &str) -> Commands {
        let trimmed = text.trim();
        if trimmed.is_empty() {
            return Vec::new();
        }
        let start = text.len() - text.trim_start().len();
        std::iter::once(start..start + trimmed.len()).collect()
    }
    fn classify(&self, _command: &str) -> CommandClass {
        CommandClass {
            write: true,
            risk: None,
        }
    }
    async fn set_read_only(&self, _on: bool) -> Result<bool> {
        Ok(false)
    }

    async fn ping(&self) -> Result<()> {
        let (tx, mut rx) = mpsc::channel(4);
        let drain = async { while rx.recv().await.is_some() {} };
        let (done, ()) = tokio::join!(self.execute("SELECT 1", tx), drain);
        done.map(|_| ())
    }
    async fn write_documents(&self, _write: &DocumentWrite) -> Result<Outcome> {
        Err(DriverError::query("this engine does not store documents"))
    }
    fn describe_documents(&self, write: &DocumentWrite) -> String {
        write.as_mongo_command()
    }
    async fn explain_native(&self, _command: &str) -> Result<String> {
        Err(DriverError::query("this engine has no plan"))
    }
    async fn scan_keys(&self, _scan: &KeyScan) -> Result<KeyPage> {
        Err(DriverError::query("this engine does not store keys"))
    }
    async fn read_key(&self, _read: &KeyRead) -> Result<KeyValue> {
        Err(DriverError::query("this engine does not store keys"))
    }
    async fn write_key(&self, _write: &KeyWrite) -> Result<Outcome> {
        Err(DriverError::query("this engine does not store keys"))
    }
    fn describe_key_write(&self, _write: &KeyWrite) -> String {
        String::new()
    }
    async fn report(&self) -> Result<Report> {
        Err(DriverError::query("this engine has no server report"))
    }
    async fn begin_native(&self) -> Result<()> {
        Err(DriverError::query("this engine has no transactions"))
    }
    async fn end_native(&self, _commit: bool) -> Result<()> {
        Err(DriverError::query("this engine has no transactions"))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize, specta::Type)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum DocumentWrite {
    Insert {
        database: String,
        collection: String,
        documents: Vec<String>,
    },
    Replace {
        database: String,
        collection: String,
        id: String,
        document: String,
    },
    Delete {
        database: String,
        collection: String,
        ids: Vec<String>,
    },
}

impl DocumentWrite {
    pub fn as_mongo_command(&self) -> String {
        let (_, collection) = self.collection();
        let name = serde_json::to_string(collection).unwrap_or_default();
        match self {
            Self::Insert { documents, .. } => {
                format!(
                    "{{ \"insert\": {name}, \"documents\": [{}] }}",
                    documents.join(", ")
                )
            }
            Self::Replace { id, document, .. } => format!(
                "{{ \"update\": {name}, \"updates\": [{{ \"q\": {{ \"_id\": {id} }}, \"u\": {document} }}] }}"
            ),
            Self::Delete { ids, .. } => format!(
                "{{ \"delete\": {name}, \"deletes\": [{{ \"q\": {{ \"_id\": {{ \"$in\": [{}] }} }}, \"limit\": 0 }}] }}",
                ids.join(", ")
            ),
        }
    }

    pub fn collection(&self) -> (&str, &str) {
        match self {
            Self::Insert {
                database,
                collection,
                ..
            }
            | Self::Replace {
                database,
                collection,
                ..
            }
            | Self::Delete {
                database,
                collection,
                ..
            } => (database, collection),
        }
    }
}
