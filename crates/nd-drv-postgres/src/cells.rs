use std::fmt::Write as _;

use tokio_postgres::Row;
use tokio_postgres::types::{FromSql, Kind, Type};

struct Raw<'a>(&'a [u8]);

impl<'a> FromSql<'a> for Raw<'a> {
    fn from_sql(
        _: &Type,
        raw: &'a [u8],
    ) -> std::result::Result<Self, Box<dyn std::error::Error + Sync + Send>> {
        Ok(Raw(raw))
    }

    fn accepts(_: &Type) -> bool {
        true
    }
}

pub(crate) fn decodable(ty: &Type) -> bool {
    match ty.kind() {
        Kind::Enum(_) => true,
        Kind::Domain(inner) => decodable(inner),
        _ => {
            matches!(
                *ty,
                Type::BOOL
                    | Type::INT2
                    | Type::INT4
                    | Type::INT8
                    | Type::OID
                    | Type::FLOAT4
                    | Type::FLOAT8
                    | Type::NUMERIC
                    | Type::TEXT
                    | Type::VARCHAR
                    | Type::BPCHAR
                    | Type::NAME
                    | Type::CHAR
                    | Type::UNKNOWN
                    | Type::XML
                    | Type::JSON
                    | Type::JSONB
                    | Type::UUID
                    | Type::BYTEA
                    | Type::DATE
                    | Type::TIME
                    | Type::TIMESTAMP
            ) || matches!(ty.name(), "citext")
        }
    }
}

pub(crate) fn row(row: &Row) -> Result<Vec<Option<String>>, tokio_postgres::Error> {
    let columns = row.columns();
    (0..row.len())
        .map(|i| {
            let raw = row.try_get::<_, Option<Raw<'_>>>(i)?;
            Ok(raw.map(|r| text(columns[i].type_(), r.0)))
        })
        .collect()
}

fn text(ty: &Type, b: &[u8]) -> String {
    if let Kind::Domain(inner) = ty.kind() {
        return text(inner, b);
    }
    match *ty {
        Type::BOOL => if b.first() == Some(&1) { "t" } else { "f" }.into(),
        Type::INT2 | Type::INT4 | Type::INT8 => int(b).to_string(),
        Type::OID => u32::try_from(int(b)).unwrap_or(0).to_string(),
        Type::FLOAT4 => float(
            b.try_into()
                .map_or(f64::NAN, |a| f64::from(f32::from_be_bytes(a))),
        ),
        Type::FLOAT8 => float(b.try_into().map_or(f64::NAN, f64::from_be_bytes)),
        Type::NUMERIC => numeric(b),
        Type::JSONB => String::from_utf8_lossy(b.get(1..).unwrap_or_default()).into_owned(),
        Type::UUID => uuid(b),
        Type::BYTEA => {
            let mut s = String::with_capacity(2 + b.len() * 2);
            s.push_str("\\x");
            for x in b {
                let _ = write!(s, "{x:02x}");
            }
            s
        }
        Type::DATE => date(int(b)),
        Type::TIME => time(int(b)),
        Type::TIMESTAMP => timestamp(int(b)),
        _ => String::from_utf8_lossy(b).into_owned(),
    }
}

fn int(b: &[u8]) -> i64 {
    match b.len() {
        2 => i64::from(i16::from_be_bytes([b[0], b[1]])),
        4 => i64::from(i32::from_be_bytes([b[0], b[1], b[2], b[3]])),
        8 => i64::from_be_bytes([b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7]]),
        _ => 0,
    }
}

fn float(v: f64) -> String {
    if v.is_nan() {
        "NaN".into()
    } else if v.is_infinite() {
        if v > 0.0 { "Infinity" } else { "-Infinity" }.into()
    } else {
        v.to_string()
    }
}

fn uuid(b: &[u8]) -> String {
    let mut s = String::with_capacity(36);
    for (i, x) in b.iter().enumerate() {
        if matches!(i, 4 | 6 | 8 | 10) {
            s.push('-');
        }
        let _ = write!(s, "{x:02x}");
    }
    s
}

fn u16_at(b: &[u8], i: usize) -> u16 {
    b.get(i..i + 2)
        .map_or(0, |x| u16::from_be_bytes([x[0], x[1]]))
}

fn numeric(b: &[u8]) -> String {
    let ndigits = usize::from(u16_at(b, 0));
    let weight = i32::from(i16::from_be_bytes(u16_at(b, 2).to_be_bytes()));
    let sign = u16_at(b, 4);
    let dscale = usize::from(u16_at(b, 6));
    match sign {
        0xC000 => return "NaN".into(),
        0xD000 => return "Infinity".into(),
        0xF000 => return "-Infinity".into(),
        _ => {}
    }
    let digit = |k: i32| -> u16 {
        usize::try_from(k)
            .ok()
            .filter(|k| *k < ndigits)
            .map_or(0, |k| u16_at(b, 8 + k * 2))
    };
    let mut s = String::new();
    if sign == 0x4000 {
        s.push('-');
    }
    if weight < 0 {
        s.push('0');
    } else {
        for k in 0..=weight {
            let d = digit(k);
            if k == 0 {
                let _ = write!(s, "{d}");
            } else {
                let _ = write!(s, "{d:04}");
            }
        }
    }
    if dscale > 0 {
        s.push('.');
        let mut frac = String::with_capacity(dscale + 4);
        let mut k = weight + 1;
        while frac.len() < dscale {
            let _ = write!(frac, "{:04}", digit(k));
            k += 1;
        }
        frac.truncate(dscale);
        s.push_str(&frac);
    }
    s
}

fn civil(days: i64) -> (i64, i64, i64) {
    let z = days + 10_957 + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    (y, m, d)
}

fn date(days: i64) -> String {
    match days {
        i64::MAX | 0x7FFF_FFFF => "infinity".into(),
        i64::MIN | -0x8000_0000 => "-infinity".into(),
        _ => {
            let (y, m, d) = civil(days);
            if y <= 0 {
                format!("{:04}-{m:02}-{d:02} BC", 1 - y)
            } else {
                format!("{y:04}-{m:02}-{d:02}")
            }
        }
    }
}

fn time(micros: i64) -> String {
    let secs = micros.div_euclid(1_000_000);
    let frac = micros.rem_euclid(1_000_000);
    let mut s = format!("{:02}:{:02}:{:02}", secs / 3600, secs / 60 % 60, secs % 60);
    if frac > 0 {
        let f = format!("{frac:06}");
        s.push('.');
        s.push_str(f.trim_end_matches('0'));
    }
    s
}

fn timestamp(micros: i64) -> String {
    match micros {
        i64::MAX => "infinity".into(),
        i64::MIN => "-infinity".into(),
        _ => {
            let day = micros.div_euclid(86_400_000_000);
            let rest = micros.rem_euclid(86_400_000_000);
            let d = date(day);
            match d.strip_suffix(" BC") {
                Some(d) => format!("{d} {} BC", time(rest)),
                None => format!("{d} {}", time(rest)),
            }
        }
    }
}
