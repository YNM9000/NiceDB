use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::Path;

use nd_driver::{Column, Row};
use serde::Deserialize;

use crate::kind::{Kind, kind_of};
use crate::parquet_io::ParquetSink;
use crate::xlsx::XlsxSink;
use crate::{FileFormat, Result, unique_names};

#[derive(Debug, Clone, Deserialize, specta::Type)]
pub struct WriteOptions {
    pub header: bool,
}

pub trait Sink: Send {
    fn rows(&mut self, rows: &[Row]) -> Result<usize>;
    fn finish(self: Box<Self>) -> Result<()>;
}

pub fn create(
    path: &Path,
    format: FileFormat,
    columns: &[Column],
    options: &WriteOptions,
) -> Result<Box<dyn Sink>> {
    let kinds: Vec<Kind> = columns
        .iter()
        .map(|c| kind_of(c.type_name.as_deref()))
        .collect();
    let names: Vec<String> = columns.iter().map(|c| c.name.clone()).collect();
    Ok(match format {
        FileFormat::Csv | FileFormat::Tsv => {
            let delimiter = if format == FileFormat::Tsv {
                b'\t'
            } else {
                b','
            };
            let mut w = csv::WriterBuilder::new()
                .delimiter(delimiter)
                .from_writer(BufWriter::new(File::create(path)?));
            if options.header {
                w.write_record(&names)?;
            }
            Box::new(TextSink(w))
        }
        FileFormat::Json => Box::new(JsonSink::new(path, &names, kinds)?),
        FileFormat::Xlsx => Box::new(XlsxSink::new(path, &names, kinds, options.header)?),
        FileFormat::Parquet => {
            let unique = unique_names(names.iter().map(String::as_str));
            Box::new(ParquetSink::new(path, &unique, &kinds)?)
        }
    })
}

struct TextSink(csv::Writer<BufWriter<File>>);

impl Sink for TextSink {
    fn rows(&mut self, rows: &[Row]) -> Result<usize> {
        for r in rows {
            self.0
                .write_record(r.iter().map(|v| v.as_deref().unwrap_or("")))?;
        }
        Ok(rows.len())
    }

    fn finish(mut self: Box<Self>) -> Result<()> {
        self.0.flush()?;
        Ok(())
    }
}

struct JsonSink {
    out: BufWriter<File>,
    keys: Vec<String>,
    kinds: Vec<Kind>,
    first: bool,
}

impl JsonSink {
    fn new(path: &Path, names: &[String], kinds: Vec<Kind>) -> Result<Self> {
        let keys = unique_names(names.iter().map(String::as_str))
            .iter()
            .map(serde_json::to_string)
            .collect::<std::result::Result<_, _>>()?;
        let mut out = BufWriter::new(File::create(path)?);
        out.write_all(b"[")?;
        Ok(Self {
            out,
            keys,
            kinds,
            first: true,
        })
    }
}

fn is_json_number(s: &str) -> bool {
    let b = s.strip_prefix('-').unwrap_or(s).as_bytes();
    let int_end = b
        .iter()
        .position(|c| !c.is_ascii_digit())
        .unwrap_or(b.len());
    if int_end == 0 || (int_end > 1 && b[0] == b'0') {
        return false;
    }
    let rest = &b[int_end..];
    match rest.split_first() {
        None => true,
        Some((b'.', frac)) => !frac.is_empty() && frac.iter().all(u8::is_ascii_digit),
        _ => false,
    }
}

fn value(kind: Kind, v: &str) -> Result<String> {
    Ok(match kind {
        Kind::Integer | Kind::Decimal | Kind::Float if is_json_number(v) => v.to_owned(),
        Kind::Boolean if matches!(v, "t" | "true" | "1") => "true".into(),
        Kind::Boolean if matches!(v, "f" | "false" | "0") => "false".into(),
        Kind::Json if serde_json::from_str::<serde::de::IgnoredAny>(v).is_ok() => v.to_owned(),
        _ => serde_json::to_string(v)?,
    })
}

impl Sink for JsonSink {
    fn rows(&mut self, rows: &[Row]) -> Result<usize> {
        for r in rows {
            self.out
                .write_all(if self.first { b"\n{" } else { b",\n{" })?;
            self.first = false;
            for (i, key) in self.keys.iter().enumerate() {
                if i > 0 {
                    self.out.write_all(b",")?;
                }
                self.out.write_all(key.as_bytes())?;
                self.out.write_all(b":")?;
                match r.get(i).and_then(Option::as_deref) {
                    None => self.out.write_all(b"null")?,
                    Some(v) => {
                        let kind = self.kinds.get(i).copied().unwrap_or(Kind::Text);
                        self.out.write_all(value(kind, v)?.as_bytes())?;
                    }
                }
            }
            self.out.write_all(b"}")?;
        }
        Ok(rows.len())
    }

    fn finish(mut self: Box<Self>) -> Result<()> {
        self.out.write_all(b"\n]\n")?;
        self.out.flush()?;
        Ok(())
    }
}
