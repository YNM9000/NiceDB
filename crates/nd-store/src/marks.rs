use serde::{Deserialize, Serialize};

use crate::{Result, Store};

const MARKS: &str = "palette_marks";
const MAX_MARKS: usize = 500;
const MAX_RECENT: usize = 40;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum MarkTarget {
    Table {
        connection_id: String,
        database: String,
        namespace: String,
        name: String,
        view: bool,
    },
    Query {
        connection_id: String,
        database: Option<String>,
        sql: String,
    },
    Item {
        key: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
pub struct Mark {
    pub id: String,
    pub label: String,
    pub pinned: bool,
    pub target: MarkTarget,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(default)]
pub struct Marks {
    pub marks: Vec<Mark>,
    pub recent: Vec<String>,
}

impl Store {
    pub fn marks(&self) -> Result<Marks> {
        Ok(self.get(MARKS)?.unwrap_or_default())
    }

    pub fn set_marks(&self, value: &Marks) -> Result<Marks> {
        let mut kept = value.clone();
        kept.marks.truncate(MAX_MARKS);
        kept.recent.truncate(MAX_RECENT);
        self.put(MARKS, &kept)?;
        Ok(kept)
    }
}
