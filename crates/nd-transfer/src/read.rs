use std::io::Read;
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use serde::{Deserialize, Serialize};

use crate::{FileFormat, Result};

#[derive(Debug, Clone, Default, Deserialize, specta::Type)]
pub struct ReadOptions {
    pub header: bool,
    pub sheet: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Record {
    #[allow(clippy::struct_field_names)]
    pub line: u64,
    pub values: Vec<Option<String>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "snake_case")]
pub enum Unit {
    Rows,
    Bytes,
    Tables,
    Pages,
}

#[derive(Debug)]
pub enum Batch {
    Columns {
        names: Vec<String>,
        sheets: Vec<String>,
    },
    Records {
        records: Vec<Record>,
        done: u64,
        total: Option<u64>,
        unit: Unit,
    },
}

pub fn read(
    path: &Path,
    format: FileFormat,
    options: &ReadOptions,
    size: usize,
    sink: &mut dyn FnMut(Batch) -> bool,
) -> Result<()> {
    let size = size.max(1);
    match format {
        FileFormat::Csv => crate::text::read(path, b',', options.header, size, sink),
        FileFormat::Tsv => crate::text::read(path, b'\t', options.header, size, sink),
        FileFormat::Json => crate::json::read(path, size, sink),
        FileFormat::Xlsx => crate::xlsx::read(path, options.sheet.as_deref(), size, sink),
        FileFormat::Parquet => crate::parquet_io::read(path, size, sink),
    }
}

pub(crate) struct Counted<R> {
    pub inner: R,
    pub seen: Arc<AtomicU64>,
}

impl<R: Read> Read for Counted<R> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let n = self.inner.read(buf)?;
        self.seen.fetch_add(n as u64, Ordering::Relaxed);
        Ok(n)
    }
}

pub(crate) struct Batcher<'a> {
    pub sink: &'a mut dyn FnMut(Batch) -> bool,
    pub buf: Vec<Record>,
    pub size: usize,
    pub total: Option<u64>,
    pub unit: Unit,
    pub stopped: bool,
}

impl<'a> Batcher<'a> {
    pub fn new(
        sink: &'a mut dyn FnMut(Batch) -> bool,
        size: usize,
        total: Option<u64>,
        unit: Unit,
    ) -> Self {
        Self {
            sink,
            buf: Vec::with_capacity(size.min(4096)),
            size,
            total,
            unit,
            stopped: false,
        }
    }

    pub fn columns(&mut self, names: Vec<String>, sheets: Vec<String>) -> bool {
        if names.is_empty() {
            return true;
        }
        if !(self.sink)(Batch::Columns { names, sheets }) {
            self.stopped = true;
        }
        !self.stopped
    }

    pub fn push(&mut self, record: Record, done: impl FnOnce() -> u64) -> bool {
        self.buf.push(record);
        if self.buf.len() >= self.size {
            self.flush(done());
        }
        !self.stopped
    }

    pub fn flush(&mut self, done: u64) {
        if self.buf.is_empty() || self.stopped {
            return;
        }
        let records = std::mem::take(&mut self.buf);
        let go = (self.sink)(Batch::Records {
            records,
            done,
            total: self.total,
            unit: self.unit,
        });
        self.stopped = !go;
    }
}
