use std::fmt::Write as _;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, specta::Type)]
pub struct KeyScan {
    pub pattern: String,
    pub cursor: String,
    pub count: u32,
    pub kind: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, specta::Type)]
pub struct KeyInfo {
    pub key: String,
    pub kind: String,
    #[specta(type = Option<f64>)]
    pub ttl_ms: Option<u64>,
    #[specta(type = Option<f64>)]
    pub bytes: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, specta::Type)]
pub struct KeyPage {
    pub keys: Vec<KeyInfo>,
    pub cursor: String,
    #[specta(type = Option<f64>)]
    pub total: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, specta::Type)]
pub struct KeyRead {
    pub key: String,
    pub cursor: String,
    pub count: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
pub struct KeyEntry {
    pub name: String,
    pub value: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, specta::Type)]
#[serde(tag = "shape", rename_all = "snake_case")]
pub enum KeyData {
    Text {
        text: String,
        binary: bool,
        truncated: bool,
    },
    Entries {
        entries: Vec<KeyEntry>,
    },
    Missing,
}

#[derive(Debug, Clone, PartialEq, Serialize, specta::Type)]
pub struct KeyValue {
    pub key: String,
    pub kind: String,
    #[specta(type = Option<f64>)]
    pub ttl_ms: Option<u64>,
    #[specta(type = Option<f64>)]
    pub length: Option<u64>,
    pub data: KeyData,
    pub next: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Deserialize, specta::Type)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum KeyWrite {
    SetString {
        key: String,
        value: String,
    },
    HashSet {
        key: String,
        field: String,
        value: String,
    },
    HashDelete {
        key: String,
        fields: Vec<String>,
    },
    ListSet {
        key: String,
        #[specta(type = f64)]
        index: i64,
        value: String,
    },
    ListPush {
        key: String,
        values: Vec<String>,
        head: bool,
    },
    ListRemove {
        key: String,
        #[specta(type = f64)]
        index: i64,
        value: String,
    },
    SetAdd {
        key: String,
        members: Vec<String>,
    },
    SetRemove {
        key: String,
        members: Vec<String>,
    },
    ZsetAdd {
        key: String,
        member: String,
        score: f64,
    },
    ZsetRemove {
        key: String,
        members: Vec<String>,
    },
    StreamAdd {
        key: String,
        fields: Vec<KeyEntry>,
    },
    StreamDelete {
        key: String,
        ids: Vec<String>,
    },
    JsonSet {
        key: String,
        value: String,
    },
    Expire {
        key: String,
        #[specta(type = Option<f64>)]
        ttl_ms: Option<u64>,
    },
    Rename {
        key: String,
        to: String,
    },
    Delete {
        keys: Vec<String>,
    },
    Create {
        key: String,
        kind: String,
        first: KeyEntry,
        #[specta(type = Option<f64>)]
        ttl_ms: Option<u64>,
    },
}

impl KeyWrite {
    pub fn key(&self) -> &str {
        match self {
            Self::Delete { keys } => keys.first().map_or("", String::as_str),
            Self::SetString { key, .. }
            | Self::HashSet { key, .. }
            | Self::HashDelete { key, .. }
            | Self::ListSet { key, .. }
            | Self::ListPush { key, .. }
            | Self::ListRemove { key, .. }
            | Self::SetAdd { key, .. }
            | Self::SetRemove { key, .. }
            | Self::ZsetAdd { key, .. }
            | Self::ZsetRemove { key, .. }
            | Self::StreamAdd { key, .. }
            | Self::StreamDelete { key, .. }
            | Self::JsonSet { key, .. }
            | Self::Expire { key, .. }
            | Self::Rename { key, .. }
            | Self::Create { key, .. } => key,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
pub struct Report {
    pub sections: Vec<ReportSection>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
pub struct ReportSection {
    pub id: String,
    pub group: ReportGroup,
    pub columns: Vec<String>,
    pub rows: Vec<Vec<String>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "snake_case")]
pub enum ReportGroup {
    Info,
    Clients,
    Slowlog,
    Memory,
    Grid,
}

pub fn quote_arg(arg: &str) -> String {
    let safe = !arg.is_empty()
        && arg.chars().all(|c| {
            c.is_alphanumeric()
                || matches!(
                    c,
                    ':' | '_' | '-' | '.' | '*' | '/' | '@' | '+' | '$' | '#' | ','
                )
        });
    if safe {
        return arg.to_owned();
    }
    let mut out = String::with_capacity(arg.len() + 2);
    out.push('"');
    for c in arg.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if c.is_control() && (c as u32) < 0x80 => {
                let _ = write!(out, "\\x{:02x}", c as u32);
            }
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

pub fn command_line<S: AsRef<str>>(args: &[S]) -> String {
    args.iter()
        .map(|a| quote_arg(a.as_ref()))
        .collect::<Vec<_>>()
        .join(" ")
}
