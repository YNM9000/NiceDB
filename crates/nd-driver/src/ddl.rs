use serde::Deserialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, specta::Type)]
#[serde(rename_all = "snake_case")]
pub enum DdlKind {
    Table,
    View,
    Routine,
    Trigger,
    Index,
    Sequence,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, specta::Type)]
pub struct ObjectRef {
    pub kind: DdlKind,
    pub namespace: String,
    pub name: String,
    pub table: Option<String>,
    pub signature: Option<String>,
}

impl ObjectRef {
    pub fn new(kind: DdlKind, namespace: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            kind,
            namespace: namespace.into(),
            name: name.into(),
            table: None,
            signature: None,
        }
    }

    #[must_use]
    pub fn on(mut self, table: impl Into<String>) -> Self {
        self.table = Some(table.into());
        self
    }

    #[must_use]
    pub fn with_signature(mut self, signature: impl Into<String>) -> Self {
        self.signature = Some(signature.into());
        self
    }

    pub fn not_found(&self) -> crate::DriverError {
        crate::DriverError::query(format!("{} not found", self.name))
    }
}
