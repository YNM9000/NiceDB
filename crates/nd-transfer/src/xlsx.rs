use std::path::{Path, PathBuf};

use calamine::{Data, Reader, open_workbook_auto};
use nd_driver::Row;
use rust_xlsxwriter::{Format, Workbook};

use crate::kind::Kind;
use crate::read::{Batch, Batcher, Record, Unit};
use crate::write::Sink;
use crate::{Error, Result};

const MAX_ROWS: u32 = 1_048_576;
const EXACT: i64 = 1 << 53;

fn cell(d: &Data) -> Option<String> {
    match d {
        Data::Empty | Data::Error(_) => None,
        Data::Int(n) => Some(n.to_string()),
        #[allow(clippy::cast_possible_truncation)]
        Data::Float(f) if f.fract() == 0.0 && f.abs() < 1e15 => Some((*f as i64).to_string()),
        Data::Float(f) => Some(f.to_string()),
        Data::String(s) | Data::DateTimeIso(s) | Data::DurationIso(s) => Some(s.clone()),
        Data::Bool(b) => Some(b.to_string()),
        Data::DateTime(dt) => Some(crate::civil::excel(dt.as_f64())),
    }
}

pub fn read(
    path: &Path,
    sheet: Option<&str>,
    size: usize,
    sink: &mut dyn FnMut(Batch) -> bool,
) -> Result<()> {
    let mut book = open_workbook_auto(path).map_err(|e| Error::Sheet(e.to_string()))?;
    let sheets = book.sheet_names();
    let name = match sheet {
        Some(s) => s.to_owned(),
        None => sheets.first().cloned().ok_or(Error::NoColumns)?,
    };
    let range = book
        .worksheet_range(&name)
        .map_err(|e| Error::Sheet(e.to_string()))?;
    let first_row = range.start().map_or(0, |(r, _)| u64::from(r));
    let total = range.height() as u64;
    let mut rows = range.rows();
    let Some(head) = rows.next() else {
        return Err(Error::NoColumns);
    };
    let names: Vec<String> = head
        .iter()
        .enumerate()
        .map(|(i, d)| cell(d).unwrap_or_else(|| format!("col_{}", i + 1)))
        .collect();
    let width = names.len();
    let mut out = Batcher::new(sink, size, Some(total), Unit::Rows);
    if !out.columns(names, sheets) {
        return Ok(());
    }
    for (i, r) in rows.enumerate() {
        let mut values: Vec<Option<String>> = r.iter().map(cell).collect();
        if values.iter().all(Option::is_none) {
            continue;
        }
        values.resize(width, None);
        let line = first_row + i as u64 + 2;
        if !out.push(Record { line, values }, || i as u64 + 2) {
            return Ok(());
        }
    }
    out.flush(total);
    Ok(())
}

pub struct XlsxSink {
    book: Workbook,
    path: PathBuf,
    kinds: Vec<Kind>,
    row: u32,
}

impl XlsxSink {
    pub fn new(path: &Path, names: &[String], kinds: Vec<Kind>, header: bool) -> Result<Self> {
        let mut book = Workbook::new();
        let sheet = book.add_worksheet_with_constant_memory()?;
        let mut row = 0;
        if header {
            let bold = Format::new().set_bold();
            for (c, name) in names.iter().enumerate() {
                sheet.write_string_with_format(0, col(c), name, &bold)?;
            }
            sheet.set_freeze_panes(1, 0)?;
            row = 1;
        }
        Ok(Self {
            book,
            path: path.to_owned(),
            kinds,
            row,
        })
    }
}

#[allow(clippy::cast_possible_truncation)]
fn col(c: usize) -> u16 {
    c.min(usize::from(u16::MAX)) as u16
}

impl Sink for XlsxSink {
    fn rows(&mut self, rows: &[Row]) -> Result<usize> {
        let sheet = self.book.worksheet_from_index(0)?;
        let room = (MAX_ROWS - self.row) as usize;
        let take = rows.len().min(room);
        for r in &rows[..take] {
            for (c, v) in r.iter().enumerate() {
                let Some(v) = v else { continue };
                let kind = self.kinds.get(c).copied().unwrap_or(Kind::Text);
                let (row, column) = (self.row, col(c));
                match kind {
                    Kind::Integer => match v.parse::<i64>() {
                        #[allow(clippy::cast_precision_loss)]
                        Ok(n) if n.abs() < EXACT => sheet.write_number(row, column, n as f64)?,
                        _ => sheet.write_string(row, column, v)?,
                    },
                    Kind::Float | Kind::Decimal => match v.parse::<f64>() {
                        Ok(f) if f.is_finite() && v.len() <= 15 => {
                            sheet.write_number(row, column, f)?
                        }
                        _ => sheet.write_string(row, column, v)?,
                    },
                    Kind::Boolean => match v.as_str() {
                        "t" | "true" | "1" => sheet.write_boolean(row, column, true)?,
                        "f" | "false" | "0" => sheet.write_boolean(row, column, false)?,
                        _ => sheet.write_string(row, column, v)?,
                    },
                    Kind::Json | Kind::Text => sheet.write_string(row, column, v)?,
                };
            }
            self.row += 1;
        }
        Ok(take)
    }

    fn finish(mut self: Box<Self>) -> Result<()> {
        self.book.save(&self.path)?;
        Ok(())
    }
}
