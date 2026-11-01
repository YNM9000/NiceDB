use scylla::value::{CqlValue, Row};
use serde_json::{Map, Value};

pub(crate) fn big_int(bytes: &[u8]) -> String {
    if bytes.is_empty() {
        return "0".into();
    }
    let negative = bytes[0] & 0x80 != 0;
    let mut mag: Vec<u8> = if negative {
        let mut v: Vec<u8> = bytes.iter().map(|b| !b).collect();
        for b in v.iter_mut().rev() {
            let (n, carry) = b.overflowing_add(1);
            *b = n;
            if !carry {
                break;
            }
        }
        v
    } else {
        bytes.to_vec()
    };
    let mut digits = Vec::new();
    while mag.iter().any(|b| *b != 0) {
        let mut rem = 0u32;
        for b in &mut mag {
            let cur = (rem << 8) | u32::from(*b);
            *b = u8::try_from(cur / 10).unwrap_or(0);
            rem = cur % 10;
        }
        digits.push(char::from(b'0' + u8::try_from(rem).unwrap_or(0)));
    }
    if digits.is_empty() {
        digits.push('0');
    }
    if negative {
        digits.push('-');
    }
    digits.iter().rev().collect()
}

pub(crate) fn decimal(unscaled: &str, scale: i32) -> String {
    let (sign, digits) = unscaled
        .strip_prefix('-')
        .map_or(("", unscaled), |d| ("-", d));
    if scale <= 0 {
        let zeros = usize::try_from(-scale).unwrap_or(0);
        return format!(
            "{sign}{digits}{}",
            "0".repeat(if digits == "0" { 0 } else { zeros })
        );
    }
    let scale = usize::try_from(scale).unwrap_or(0);
    let padded = format!("{digits:0>width$}", width = scale + 1);
    let (int, frac) = padded.split_at(padded.len() - scale);
    format!("{sign}{int}.{frac}")
}

fn civil(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = u32::try_from(doy - (153 * mp + 2) / 5 + 1).unwrap_or(1);
    let m = u32::try_from(if mp < 10 { mp + 3 } else { mp - 9 }).unwrap_or(1);
    (yoe + era * 400 + i64::from(m <= 2), m, d)
}

pub(crate) fn date(days_since_epoch: i64) -> String {
    let (y, m, d) = civil(days_since_epoch);
    format!("{y:04}-{m:02}-{d:02}")
}

pub(crate) fn timestamp(ms: i64) -> String {
    let days = ms.div_euclid(86_400_000);
    let rem = ms.rem_euclid(86_400_000);
    format!(
        "{} {:02}:{:02}:{:02}.{:03}Z",
        date(days),
        rem / 3_600_000,
        rem / 60_000 % 60,
        rem / 1000 % 60,
        rem % 1000
    )
}

pub(crate) fn time(nanos: i64) -> String {
    let s = nanos / 1_000_000_000;
    format!(
        "{:02}:{:02}:{:02}.{:09}",
        s / 3600,
        s / 60 % 60,
        s % 60,
        nanos % 1_000_000_000
    )
}

fn wide(n: i64) -> Value {
    if n.unsigned_abs() < (1 << 53) {
        Value::from(n)
    } else {
        Value::from(n.to_string())
    }
}

fn float(f: f64) -> Value {
    serde_json::Number::from_f64(f).map_or_else(|| Value::from(f.to_string()), Value::Number)
}

pub(crate) fn to_json(v: &CqlValue) -> Value {
    match v {
        CqlValue::Ascii(s) | CqlValue::Text(s) => Value::from(s.as_str()),
        CqlValue::Boolean(b) => Value::Bool(*b),
        CqlValue::Blob(b) => Value::from(format!("0x{}", hex::encode(b))),
        CqlValue::Counter(c) => wide(c.0),
        CqlValue::Decimal(d) => {
            let (bytes, scale) = d.as_signed_be_bytes_slice_and_exponent();
            Value::from(decimal(&big_int(bytes), scale))
        }
        CqlValue::Date(d) => Value::from(date(i64::from(d.0) - (1i64 << 31))),
        CqlValue::Double(f) => float(*f),
        CqlValue::Float(f) => float(f64::from(*f)),
        CqlValue::Duration(d) => {
            Value::from(format!("{}mo{}d{}ns", d.months, d.days, d.nanoseconds))
        }
        CqlValue::Int(i) => Value::from(*i),
        CqlValue::BigInt(i) => wide(*i),
        CqlValue::SmallInt(i) => Value::from(*i),
        CqlValue::TinyInt(i) => Value::from(*i),
        CqlValue::Timestamp(t) => Value::from(timestamp(t.0)),
        CqlValue::Time(t) => Value::from(time(t.0)),
        CqlValue::Inet(ip) => Value::from(ip.to_string()),
        CqlValue::Uuid(u) => Value::from(u.to_string()),
        CqlValue::Timeuuid(u) => Value::from(uuid::Uuid::from(*u).to_string()),
        CqlValue::Varint(v) => Value::from(big_int(v.as_signed_bytes_be_slice())),
        CqlValue::List(l) | CqlValue::Set(l) | CqlValue::Vector(l) => {
            Value::Array(l.iter().map(to_json).collect())
        }
        CqlValue::Map(pairs) => Value::Object(
            pairs
                .iter()
                .map(|(k, v)| {
                    let key = match to_json(k) {
                        Value::String(s) => s,
                        other => other.to_string(),
                    };
                    (key, to_json(v))
                })
                .collect::<Map<_, _>>(),
        ),
        CqlValue::UserDefinedType { fields, .. } => Value::Object(
            fields
                .iter()
                .map(|(n, v)| (n.clone(), v.as_ref().map_or(Value::Null, to_json)))
                .collect(),
        ),
        CqlValue::Tuple(items) => Value::Array(
            items
                .iter()
                .map(|v| v.as_ref().map_or(Value::Null, to_json))
                .collect(),
        ),
        _ => Value::Null,
    }
}

pub(crate) fn row_document(names: &[String], row: &Row) -> String {
    let mut out = String::from("{");
    for (i, (name, v)) in names.iter().zip(&row.columns).enumerate() {
        if i > 0 {
            out.push(',');
        }
        out.push_str(&Value::from(name.as_str()).to_string());
        out.push(':');
        out.push_str(&v.as_ref().map_or(Value::Null, to_json).to_string());
    }
    out.push('}');
    out
}
