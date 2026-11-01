use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use crate::read::{Batch, Batcher, Counted, Record, Unit};
use crate::{Error, Result};

fn sniff(path: &Path) -> Result<u8> {
    let mut line = Vec::new();
    BufReader::new(File::open(path)?).read_until(b'\n', &mut line)?;
    let (mut quoted, mut commas, mut semis) = (false, 0, 0);
    for b in line {
        match b {
            b'"' => quoted = !quoted,
            b',' if !quoted => commas += 1,
            b';' if !quoted => semis += 1,
            _ => {}
        }
    }
    Ok(if semis > commas { b';' } else { b',' })
}

const HIGH: [char; 32] = [
    '€', '\u{81}', '‚', 'ƒ', '„', '…', '†', '‡', 'ˆ', '‰', 'Š', '‹', 'Œ', '\u{8d}', 'Ž', '\u{8f}',
    '\u{90}', '‘', '’', '“', '”', '•', '–', '—', '˜', '™', 'š', '›', 'œ', '\u{9d}', 'ž', 'Ÿ',
];

fn decode(bytes: &[u8]) -> String {
    if let Ok(s) = std::str::from_utf8(bytes) {
        return s.to_owned();
    }
    bytes
        .iter()
        .map(|&b| match b {
            0x80..=0x9f => HIGH[usize::from(b - 0x80)],
            _ => char::from(b),
        })
        .collect()
}

pub fn read(
    path: &Path,
    delimiter: u8,
    header: bool,
    size: usize,
    sink: &mut dyn FnMut(Batch) -> bool,
) -> Result<()> {
    let delimiter = if delimiter == b',' {
        sniff(path)?
    } else {
        delimiter
    };
    let total = std::fs::metadata(path)?.len();
    let seen = Arc::new(AtomicU64::new(0));
    let file = Counted {
        inner: File::open(path)?,
        seen: seen.clone(),
    };
    let mut reader = csv::ReaderBuilder::new()
        .delimiter(delimiter)
        .has_headers(false)
        .flexible(true)
        .from_reader(file);
    let mut out = Batcher::new(sink, size, Some(total), Unit::Bytes);
    let mut record = csv::ByteRecord::new();
    let mut width = 0;
    let mut first = true;
    while reader.read_byte_record(&mut record)? {
        let line = record.position().map_or(0, csv::Position::line);
        let mut values: Vec<Option<String>> = record.iter().map(|f| Some(decode(f))).collect();
        if first {
            first = false;
            if let Some(Some(v)) = values.first_mut()
                && let Some(rest) = v.strip_prefix('\u{feff}')
            {
                *v = rest.to_owned();
            }
            width = values.len();
            let names = if header {
                values
                    .iter()
                    .map(|v| v.clone().unwrap_or_default())
                    .collect()
            } else {
                (1..=width).map(|i| format!("col_{i}")).collect()
            };
            if width == 0 {
                return Err(Error::NoColumns);
            }
            if !out.columns(names, Vec::new()) {
                return Ok(());
            }
            if header {
                continue;
            }
        }
        if values.len() == 1 && values[0].as_deref() == Some("") && width > 1 {
            continue;
        }
        values.resize(width.max(values.len()), None);
        if !out.push(Record { line, values }, || seen.load(Ordering::Relaxed)) {
            return Ok(());
        }
    }
    if first {
        return Err(Error::NoColumns);
    }
    out.flush(total);
    Ok(())
}
