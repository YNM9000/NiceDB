use std::fs::File;
use std::path::Path;
use std::sync::Arc;

use nd_driver::Row;
use parquet::basic::{Compression, LogicalType, Repetition, Type as Physical};
use parquet::column::writer::ColumnWriter;
use parquet::data_type::ByteArray;
use parquet::file::properties::WriterProperties;
use parquet::file::reader::{FileReader, SerializedFileReader};
use parquet::file::writer::SerializedFileWriter;
use parquet::record::Field;
use parquet::schema::types::Type;

use crate::kind::Kind;
use crate::read::{Batch, Batcher, Record, Unit};
use crate::write::Sink;
use crate::{Error, Result};

const GROUP_ROWS: usize = 50_000;

fn hex(bytes: &[u8]) -> String {
    use std::fmt::Write as _;
    let mut s = String::with_capacity(2 + bytes.len() * 2);
    s.push_str("\\x");
    for b in bytes {
        let _ = write!(s, "{b:02x}");
    }
    s
}

fn cell(f: &Field) -> Option<String> {
    Some(match f {
        Field::Null => return None,
        Field::Bool(b) => b.to_string(),
        Field::Byte(n) => n.to_string(),
        Field::Short(n) => n.to_string(),
        Field::Int(n) => n.to_string(),
        Field::Long(n) => n.to_string(),
        Field::UByte(n) => n.to_string(),
        Field::UShort(n) => n.to_string(),
        Field::UInt(n) => n.to_string(),
        Field::ULong(n) => n.to_string(),
        Field::Float16(n) => n.to_string(),
        Field::Float(n) => n.to_string(),
        Field::Double(n) => n.to_string(),
        Field::Str(s) => s.clone(),
        Field::Bytes(b) => {
            std::str::from_utf8(b.data()).map_or_else(|_| hex(b.data()), str::to_owned)
        }
        Field::Date(d) => crate::civil::date(i64::from(*d)),
        Field::TimestampMillis(ms) => crate::civil::timestamp_micros(ms.saturating_mul(1000)),
        Field::TimestampMicros(us) => crate::civil::timestamp_micros(*us),
        Field::TimeMillis(ms) => {
            let s = ms / 1000;
            format!(
                "{:02}:{:02}:{:02}.{:03}",
                s / 3600,
                s % 3600 / 60,
                s % 60,
                ms % 1000
            )
        }
        Field::TimeMicros(us) => {
            let s = us / 1_000_000;
            format!(
                "{:02}:{:02}:{:02}.{:06}",
                s / 3600,
                s % 3600 / 60,
                s % 60,
                us % 1_000_000
            )
        }
        other @ (Field::Decimal(_)
        | Field::Group(_)
        | Field::ListInternal(_)
        | Field::MapInternal(_)) => other.to_string(),
    })
}

pub fn read(path: &Path, size: usize, sink: &mut dyn FnMut(Batch) -> bool) -> Result<()> {
    let reader = SerializedFileReader::new(File::open(path)?)?;
    let meta = reader.metadata().file_metadata();
    let total = u64::try_from(meta.num_rows()).unwrap_or(0);
    let names: Vec<String> = meta
        .schema_descr()
        .root_schema()
        .get_fields()
        .iter()
        .map(|f| f.name().to_owned())
        .collect();
    if names.is_empty() {
        return Err(Error::NoColumns);
    }
    let width = names.len();
    let mut out = Batcher::new(sink, size, Some(total), Unit::Rows);
    if !out.columns(names, Vec::new()) {
        return Ok(());
    }
    for (i, row) in reader.get_row_iter(None)?.enumerate() {
        let row = row?;
        let mut values: Vec<Option<String>> = row.get_column_iter().map(|(_, f)| cell(f)).collect();
        values.resize(width, None);
        let n = i as u64 + 1;
        if !out.push(Record { line: n, values }, || n) {
            return Ok(());
        }
    }
    out.flush(total);
    Ok(())
}

fn field(name: &str, kind: Kind) -> Result<Arc<Type>> {
    let (physical, logical) = match kind {
        Kind::Integer => (Physical::INT64, None),
        Kind::Float => (Physical::DOUBLE, None),
        Kind::Boolean => (Physical::BOOLEAN, None),
        Kind::Json => (Physical::BYTE_ARRAY, Some(LogicalType::Json)),
        Kind::Decimal | Kind::Text => (Physical::BYTE_ARRAY, Some(LogicalType::String)),
    };
    Ok(Arc::new(
        Type::primitive_type_builder(name, physical)
            .with_repetition(Repetition::OPTIONAL)
            .with_logical_type(logical)
            .build()?,
    ))
}

pub struct ParquetSink {
    writer: SerializedFileWriter<File>,
    names: Vec<String>,
    buf: Vec<Row>,
}

impl ParquetSink {
    pub fn new(path: &Path, names: &[String], kinds: &[Kind]) -> Result<Self> {
        let fields = names
            .iter()
            .zip(kinds)
            .map(|(n, k)| field(n, *k))
            .collect::<Result<Vec<_>>>()?;
        let schema = Arc::new(
            Type::group_type_builder("schema")
                .with_fields(fields)
                .build()?,
        );
        let props = Arc::new(
            WriterProperties::builder()
                .set_compression(Compression::SNAPPY)
                .build(),
        );
        let writer = SerializedFileWriter::new(File::create(path)?, schema, props)?;
        Ok(Self {
            writer,
            names: names.to_vec(),
            buf: Vec::new(),
        })
    }

    fn group(&mut self) -> Result<()> {
        if self.buf.is_empty() {
            return Ok(());
        }
        let rows = std::mem::take(&mut self.buf);
        let mut group = self.writer.next_row_group()?;
        let mut c = 0;
        while let Some(mut column) = group.next_column()? {
            let cells = rows.iter().map(|r| r.get(c).and_then(Option::as_deref));
            let defs: Vec<i16> = cells.clone().map(|v| i16::from(v.is_some())).collect();
            let present = cells.flatten();
            let fail = |v: &str, what| Error::Value {
                column: self.names.get(c).cloned().unwrap_or_default(),
                value: v.chars().take(40).collect(),
                expected: what,
            };
            match column.untyped() {
                ColumnWriter::Int64ColumnWriter(w) => {
                    let values = present
                        .map(|v| v.parse::<i64>().map_err(|_| fail(v, "integer")))
                        .collect::<Result<Vec<_>>>()?;
                    w.write_batch(&values, Some(&defs), None)?;
                }
                ColumnWriter::DoubleColumnWriter(w) => {
                    let values = present
                        .map(|v| v.parse::<f64>().map_err(|_| fail(v, "number")))
                        .collect::<Result<Vec<_>>>()?;
                    w.write_batch(&values, Some(&defs), None)?;
                }
                ColumnWriter::BoolColumnWriter(w) => {
                    let values = present
                        .map(|v| match v {
                            "t" | "true" | "1" => Ok(true),
                            "f" | "false" | "0" => Ok(false),
                            _ => Err(fail(v, "boolean")),
                        })
                        .collect::<Result<Vec<_>>>()?;
                    w.write_batch(&values, Some(&defs), None)?;
                }
                ColumnWriter::ByteArrayColumnWriter(w) => {
                    let values: Vec<ByteArray> = present
                        .map(|v| ByteArray::from(v.as_bytes().to_vec()))
                        .collect();
                    w.write_batch(&values, Some(&defs), None)?;
                }
                _ => return Err(fail("", "supported column")),
            }
            column.close()?;
            c += 1;
        }
        group.close()?;
        Ok(())
    }
}

impl Sink for ParquetSink {
    fn rows(&mut self, rows: &[Row]) -> Result<usize> {
        self.buf.extend_from_slice(rows);
        if self.buf.len() >= GROUP_ROWS {
            self.group()?;
        }
        Ok(rows.len())
    }

    fn finish(mut self: Box<Self>) -> Result<()> {
        self.group()?;
        self.writer.close()?;
        Ok(())
    }
}
