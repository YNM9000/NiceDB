mod data;
mod error;
mod explain;
mod guard;
mod keys;
mod native;
mod offsets;
mod org;
mod params;
mod run;
mod session;
mod tools;
mod tx;
mod upkeep;

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use nd_net::KnownHosts;
use nd_secrets::SecretStore;
use nd_store::Store;

pub use data::{ApplyOutcome, ChangePreview, IdentityColumn, KeyMatch, Page, SummarySource};
pub use error::CoreError;
pub use explain::{EstimateReason, Execution, ExplainMode, ExplainRequest, PlanReport};
pub use guard::{UNLOCK_FOR, WriteAccess, statement_timeout};
pub use nd_gen::{Check as GenCheck, ColumnGen, Gen, Shape as GenShape};
pub use nd_store::{ConfirmMode, UnlockMode};
pub use org::{ExportFormat, PackageFault};
pub use params::{ParamSlot, ParamSpot, ParamsFound};
pub use run::{MAX_ROWS, RiskFound, RunConfirm, RunEvent, RunRequest, RunScope};
pub use session::{InitFailure, LAZY_FROM, MAX_JUMPS, Secrets, SessionInfo, VarPreview};
pub use tools::*;
pub use tx::{CommitMode, PreviewKind, RowPreview, TxState};
pub use upkeep::SessionEvent;

pub type Result<T> = std::result::Result<T, CoreError>;

pub struct Core {
    store: Arc<Mutex<Store>>,
    secrets: Arc<dyn SecretStore>,
    known_hosts: Arc<KnownHosts>,
    sessions: Mutex<HashMap<String, Arc<session::Session>>>,
    runs: Mutex<HashMap<String, run::RunHandle>>,
    jobs: Mutex<HashMap<String, Arc<tools::Job>>>,
    kept_rows: Mutex<HashMap<String, Arc<tools::KeptRows>>>,
    tool_dirs: Mutex<HashMap<tools::ToolFamily, std::path::PathBuf>>,
    trashed: Mutex<HashMap<String, Option<String>>>,
    dormant: Mutex<HashMap<String, upkeep::Dormant>>,
    reconnecting: tokio::sync::Mutex<()>,
}

impl Core {
    pub fn new(store: Store, secrets: Arc<dyn SecretStore>, known_hosts: KnownHosts) -> Self {
        Self {
            store: Arc::new(Mutex::new(store)),
            secrets,
            known_hosts: Arc::new(known_hosts),
            sessions: Mutex::default(),
            runs: Mutex::default(),
            jobs: Mutex::default(),
            kept_rows: Mutex::default(),
            tool_dirs: Mutex::default(),
            trashed: Mutex::default(),
            dormant: Mutex::default(),
            reconnecting: tokio::sync::Mutex::new(()),
        }
    }

    pub fn store<T>(&self, f: impl FnOnce(&Store) -> nd_store::Result<T>) -> Result<T> {
        let guard = self.store.lock().map_err(|_| CoreError::Poisoned)?;
        Ok(f(&guard)?)
    }

    pub fn trust_host_key(&self, host: &str, port: u16, key: &str) -> Result<()> {
        Ok(self.known_hosts.trust(host, port, key)?)
    }

    fn lock<T>(m: &Mutex<T>) -> Result<std::sync::MutexGuard<'_, T>> {
        m.lock().map_err(|_| CoreError::Poisoned)
    }
}
