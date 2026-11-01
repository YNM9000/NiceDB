mod civil;
mod infer;
mod json;
mod kind;
mod parquet_io;
mod read;
mod text;
mod write;
mod xlsx;

use std::path::Path;

use serde::{Deserialize, Serialize};

pub use infer::{Inferred, infer, sql_type};
pub use kind::{Kind, kind_of};
pub use read::{Batch, ReadOptions, Record, Unit, read};
pub use write::{Sink, WriteOptions, create};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "snake_case")]
pub enum FileFormat {
    Csv,
    Tsv,
    Json,
    Xlsx,
    Parquet,
}

impl FileFormat {
    pub fn from_path(path: &Path) -> Option<Self> {
        let ext = path.extension()?.to_str()?.to_ascii_lowercase();
        Some(match ext.as_str() {
            "csv" => Self::Csv,
            "tsv" | "tab" => Self::Tsv,
            "json" | "ndjson" | "jsonl" => Self::Json,
            "xlsx" | "xlsm" | "xls" | "ods" => Self::Xlsx,
            "parquet" | "pq" => Self::Parquet,
            _ => return None,
        })
    }

    pub fn extension(self) -> &'static str {
        match self {
            Self::Csv => "csv",
            Self::Tsv => "tsv",
            Self::Json => "json",
            Self::Xlsx => "xlsx",
            Self::Parquet => "parquet",
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("{0}")]
    Io(#[from] std::io::Error),
    #[error("{0}")]
    Csv(#[from] csv::Error),
    #[error("{0}")]
    Json(#[from] serde_json::Error),
    #[error("{0}")]
    Parquet(#[from] parquet::errors::ParquetError),
    #[error("{0}")]
    Xlsx(#[from] rust_xlsxwriter::XlsxError),
    #[error("{0}")]
    Sheet(String),
    #[error("the file has no columns")]
    NoColumns,
    #[error("JSON must be an array of objects or one object per line")]
    JsonShape,
    #[error("column {column}: {value:?} is not a {expected}")]
    Value {
        column: String,
        value: String,
        expected: &'static str,
    },
}

pub type Result<T> = std::result::Result<T, Error>;

pub fn unique_names<'a>(names: impl IntoIterator<Item = &'a str>) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for name in names {
        let base = if name.is_empty() { "column" } else { name };
        let mut candidate = base.to_owned();
        let mut n = 1;
        while out.iter().any(|x| x == &candidate) {
            n += 1;
            candidate = format!("{base}_{n}");
        }
        out.push(candidate);
    }
    out
}
