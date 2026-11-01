use nd_driver::DriverError;
use nd_net::TunnelError;
use serde::Serialize;

#[derive(Debug, Clone, thiserror::Error, Serialize, specta::Type)]
#[serde(tag = "code", content = "detail", rename_all = "snake_case")]
pub enum CoreError {
    #[error(transparent)]
    Driver(DriverError),
    #[error(transparent)]
    Tunnel(TunnelError),
    #[error("keychain: {0}")]
    Secrets(String),
    #[error("store: {0}")]
    Store(String),
    #[error("not found: {0}")]
    NotFound(String),
    #[error("this engine keeps one database per connection")]
    SingleDatabase,
    #[error("a statement is already running on this connection")]
    Busy,
    #[error("production connection is read-only until writes are unlocked")]
    ReadOnly,
    #[error("writes on production need the connection name typed")]
    NeedsConfirmation,
    #[error("the typed name does not match the connection")]
    NameMismatch,
    #[error("risky statements need confirmation")]
    Risky(Vec<crate::RiskFound>),
    #[error("a transaction opened from the editor is still open")]
    TransactionOpen,
    #[error("the transaction failed and can only be rolled back")]
    TransactionFailed,
    #[error("statement {} cannot run in a dry run: {}", .index + 1, .sql)]
    NotDryRunnable { index: u32, sql: String },
    #[error("table has no primary or unique key")]
    NoKey,
    #[error("binary column {0} takes hex values (\\x… or 0x…)")]
    InvalidValue(String),
    #[error("no execution plan for this engine")]
    PlanUnavailable,
    #[error("results cannot be sorted by the server: {0:?}")]
    Unsortable(nd_sql::Unsortable),
    #[error("package: {0:?}")]
    Package(crate::PackageFault),
    #[error("file: {0}")]
    Io(String),
    #[error("tool: {0:?}")]
    Tool(crate::ToolFault),
    #[error("init statement {} failed: {}", .0.index + 1, .0.error)]
    Init(Box<crate::InitFailure>),
    #[error("parameter: {0}")]
    Param(nd_sql::ParamError),
    #[error("variable: {0}")]
    Variable(nd_store::VarError),
    #[error("a model is not opened: open a connection that follows it")]
    Model,
    #[error("this engine does not speak SQL")]
    NotSql,
    #[error("state lock poisoned")]
    Poisoned,
}

impl From<DriverError> for CoreError {
    fn from(e: DriverError) -> Self {
        Self::Driver(e)
    }
}

impl From<TunnelError> for CoreError {
    fn from(e: TunnelError) -> Self {
        Self::Tunnel(e)
    }
}

impl From<nd_store::Error> for CoreError {
    fn from(e: nd_store::Error) -> Self {
        Self::Store(e.to_string())
    }
}

impl From<nd_secrets::Error> for CoreError {
    fn from(e: nd_secrets::Error) -> Self {
        Self::Secrets(e.to_string())
    }
}
