use std::fmt::Write as _;

use hdbconnect::{HdbValue, TypeId};
use nd_driver::{DriverError, Result};

pub fn type_name(t: TypeId) -> String {
    match t {
        TypeId::INT => "INTEGER".into(),
        TypeId::DECIMAL | TypeId::FIXED8 | TypeId::FIXED12 | TypeId::FIXED16 => "DECIMAL".into(),
        TypeId::DAYDATE => "DATE".into(),
        TypeId::SECONDTIME => "TIME".into(),
        TypeId::LONGDATE => "TIMESTAMP".into(),
        TypeId::STRING => "VARCHAR".into(),
        TypeId::NSTRING => "NVARCHAR".into(),
        TypeId::BSTRING => "VARBINARY".into(),
        other => format!("{other:?}"),
    }
}

pub fn hex(b: &[u8]) -> String {
    let mut out = String::with_capacity(2 + b.len() * 2);
    out.push_str("\\x");
    for byte in b {
        let _ = write!(out, "{byte:02x}");
    }
    out
}

fn timestamp(iso: &str) -> String {
    let spaced = iso.replacen('T', " ", 1);
    match spaced.split_once('.') {
        Some((whole, fraction)) => {
            let fraction = fraction.trim_end_matches('0');
            if fraction.is_empty() {
                whole.to_owned()
            } else {
                format!("{whole}.{fraction}")
            }
        }
        None => spaced,
    }
}

fn lob(e: hdbconnect::HdbError) -> DriverError {
    DriverError::query(e.to_string())
}

pub fn text(v: HdbValue<'static>) -> Result<Option<String>> {
    Ok(Some(match v {
        HdbValue::NULL => return Ok(None),
        HdbValue::STRING(s) => s,
        HdbValue::STR(s) => s.to_owned(),
        HdbValue::DBSTRING(b) => String::from_utf8_lossy(&b).into_owned(),
        HdbValue::BINARY(b) | HdbValue::GEOMETRY(b) | HdbValue::POINT(b) => hex(&b),
        HdbValue::SYNC_CLOB(c) => c.into_string().map_err(lob)?,
        HdbValue::SYNC_NCLOB(c) => c.into_string().map_err(lob)?,
        HdbValue::SYNC_BLOB(b) => hex(&b.into_bytes().map_err(lob)?),
        HdbValue::LONGDATE(d) => timestamp(&d.to_string()),
        HdbValue::SECONDDATE(d) => timestamp(&d.to_string()),
        HdbValue::ARRAY(items) => {
            let mut cells = Vec::with_capacity(items.len());
            for item in items {
                cells.push(text(item)?.unwrap_or_else(|| "NULL".into()));
            }
            format!("[{}]", cells.join(", "))
        }
        other => other.to_string(),
    }))
}
