use std::fmt::Write;

use oracledb::{
    JsonValue, Metadata, OracleIntervalDS, OracleIntervalYM, OracleNumber, OracleTimestamp, Row,
    Vector, VectorData,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Reader {
    Text,
    Number,
    Float,
    Double,
    Date,
    Timestamp,
    TimestampTz,
    IntervalDs,
    IntervalYm,
    Bytes,
    Boolean,
    Json,
    Vector,
    Opaque(&'static str),
}

pub(crate) fn reader(m: &Metadata) -> Reader {
    let t = m.db_type();
    if t == oracledb::DB_TYPE_NUMBER || t == oracledb::DB_TYPE_BINARY_INTEGER {
        Reader::Number
    } else if t == oracledb::DB_TYPE_BINARY_FLOAT {
        Reader::Float
    } else if t == oracledb::DB_TYPE_BINARY_DOUBLE {
        Reader::Double
    } else if t == oracledb::DB_TYPE_DATE {
        Reader::Date
    } else if t == oracledb::DB_TYPE_TIMESTAMP || t == oracledb::DB_TYPE_TIMESTAMP_LTZ {
        Reader::Timestamp
    } else if t == oracledb::DB_TYPE_TIMESTAMP_TZ {
        Reader::TimestampTz
    } else if t == oracledb::DB_TYPE_INTERVAL_DS {
        Reader::IntervalDs
    } else if t == oracledb::DB_TYPE_INTERVAL_YM {
        Reader::IntervalYm
    } else if t == oracledb::DB_TYPE_RAW
        || t == oracledb::DB_TYPE_LONG_RAW
        || t == oracledb::DB_TYPE_BLOB
    {
        Reader::Bytes
    } else if t == oracledb::DB_TYPE_BOOLEAN {
        Reader::Boolean
    } else if t == oracledb::DB_TYPE_JSON {
        Reader::Json
    } else if t == oracledb::DB_TYPE_VECTOR {
        Reader::Vector
    } else if t == oracledb::DB_TYPE_BFILE {
        Reader::Opaque("(BFILE)")
    } else if t == oracledb::DB_TYPE_OBJECT || t == oracledb::DB_TYPE_XMLTYPE {
        Reader::Opaque("(object)")
    } else if t == oracledb::DB_TYPE_CURSOR {
        Reader::Opaque("(cursor)")
    } else {
        Reader::Text
    }
}

pub(crate) fn cell(row: &Row, i: usize, r: Reader) -> Result<Option<String>, oracledb::Error> {
    Ok(match r {
        Reader::Text => row.get::<Option<String>>(i)?,
        Reader::Number => row.get::<Option<OracleNumber>>(i)?.map(|n| n.to_string()),
        Reader::Float => row.get::<Option<f32>>(i)?.map(|f| f.to_string()),
        Reader::Double => row.get::<Option<f64>>(i)?.map(|f| f.to_string()),
        Reader::Date => row.get::<Option<OracleTimestamp>>(i)?.map(|t| date(&t)),
        Reader::Timestamp => row
            .get::<Option<OracleTimestamp>>(i)?
            .map(|t| timestamp(&t, false)),
        Reader::TimestampTz => row
            .get::<Option<OracleTimestamp>>(i)?
            .map(|t| timestamp(&t, true)),
        Reader::IntervalDs => row
            .get::<Option<OracleIntervalDS>>(i)?
            .map(|v| interval_ds(&v)),
        Reader::IntervalYm => row.get::<Option<OracleIntervalYM>>(i)?.map(interval_ym),
        Reader::Bytes => row.get::<Option<Vec<u8>>>(i)?.map(|b| hex(&b)),
        Reader::Boolean => row.get::<Option<bool>>(i)?.map(|b| b.to_string()),
        Reader::Json => row.get::<Option<JsonValue>>(i)?.map(|j| {
            let mut s = String::new();
            json(&j, &mut s);
            s
        }),
        Reader::Vector => row.get::<Option<Vector>>(i)?.map(|v| vector(&v)),
        Reader::Opaque(label) => match row.get::<Option<String>>(i) {
            Ok(None) => None,
            _ => Some(label.to_owned()),
        },
    })
}

pub(crate) fn date(t: &OracleTimestamp) -> String {
    let day = format!("{:04}-{:02}-{:02}", t.year(), t.month(), t.day());
    if (t.hour(), t.minute(), t.second()) == (0, 0, 0) {
        day
    } else {
        format!("{day} {:02}:{:02}:{:02}", t.hour(), t.minute(), t.second())
    }
}

pub(crate) fn timestamp(t: &OracleTimestamp, zone: bool) -> String {
    let mut s = format!(
        "{:04}-{:02}-{:02} {:02}:{:02}:{:02}.{:06}",
        t.year(),
        t.month(),
        t.day(),
        t.hour(),
        t.minute(),
        t.second(),
        t.nanoseconds() / 1000
    );
    let nanos = t.nanoseconds() % 1000;
    if nanos != 0 {
        let _ = write!(s, "{nanos:03}");
    }
    if zone {
        s.push_str(" +00:00");
    }
    s
}

pub(crate) fn interval_ds(v: &OracleIntervalDS) -> String {
    let negative =
        v.days() < 0 || v.hours() < 0 || v.minutes() < 0 || v.seconds() < 0 || v.nanoseconds() < 0;
    format!(
        "{}{:02} {:02}:{:02}:{:02}.{:06}",
        if negative { '-' } else { '+' },
        v.days().unsigned_abs(),
        v.hours().unsigned_abs(),
        v.minutes().unsigned_abs(),
        v.seconds().unsigned_abs(),
        v.nanoseconds().unsigned_abs() / 1000
    )
}

pub(crate) fn interval_ym(v: OracleIntervalYM) -> String {
    let negative = v.years() < 0 || v.months() < 0;
    format!(
        "{}{:02}-{:02}",
        if negative { '-' } else { '+' },
        v.years().unsigned_abs(),
        v.months().unsigned_abs()
    )
}

pub(crate) fn hex(b: &[u8]) -> String {
    let mut s = String::with_capacity(2 + b.len() * 2);
    s.push_str("\\x");
    for byte in b {
        let _ = write!(s, "{byte:02x}");
    }
    s
}

fn json_string(v: &str, out: &mut String) {
    out.push('"');
    for c in v.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if u32::from(c) < 0x20 => {
                let _ = write!(out, "\\u{:04x}", u32::from(c));
            }
            c => out.push(c),
        }
    }
    out.push('"');
}

pub(crate) fn json(v: &JsonValue, out: &mut String) {
    match v {
        JsonValue::Null => out.push_str("null"),
        JsonValue::Boolean(b) => out.push_str(if *b { "true" } else { "false" }),
        JsonValue::Number(n) => out.push_str(&n.to_string()),
        JsonValue::BinaryDouble(f) => out.push_str(&f.to_string()),
        JsonValue::BinaryFloat(f) => out.push_str(&f.to_string()),
        JsonValue::String(s) => json_string(s, out),
        JsonValue::Timestamp(t) => json_string(&timestamp(t, t.is_utc()), out),
        JsonValue::IntervalDS(i) => json_string(&interval_ds(i), out),
        JsonValue::IntervalYM(i) => json_string(&interval_ym(*i), out),
        JsonValue::Raw(b) | JsonValue::JsonId(b) => json_string(&hex(b), out),
        JsonValue::Vector(vec) => out.push_str(&vector(vec)),
        JsonValue::JsonArray(items) => {
            out.push('[');
            for (i, item) in items.iter().enumerate() {
                if i > 0 {
                    out.push_str(", ");
                }
                json(item, out);
            }
            out.push(']');
        }
        JsonValue::JsonObject(map) => {
            let mut keys: Vec<&String> = map.keys().collect();
            keys.sort();
            out.push('{');
            for (i, k) in keys.into_iter().enumerate() {
                if i > 0 {
                    out.push_str(", ");
                }
                json_string(k, out);
                out.push_str(": ");
                if let Some(v) = map.get(k) {
                    json(v, out);
                }
            }
            out.push('}');
        }
    }
}

fn numbers(d: &VectorData) -> Vec<String> {
    match d {
        VectorData::Float32(v) => v.iter().map(ToString::to_string).collect(),
        VectorData::Float64(v) => v.iter().map(ToString::to_string).collect(),
        VectorData::Int8(v) => v.iter().map(ToString::to_string).collect(),
        VectorData::Binary(v) => v.iter().map(ToString::to_string).collect(),
    }
}

pub(crate) fn vector(v: &Vector) -> String {
    match v {
        Vector::Dense(d) => format!("[{}]", numbers(d).join(", ")),
        Vector::Sparse(s) => format!(
            "[{}, [{}], [{}]]",
            s.num_dimensions(),
            s.indices()
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join(", "),
            numbers(s.values()).join(", ")
        ),
    }
}
