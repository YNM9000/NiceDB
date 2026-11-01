use nd_driver::Dialect;
use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "snake_case")]
pub enum Inferred {
    Integer,
    BigInteger,
    Decimal,
    Float,
    Boolean,
    Date,
    Timestamp,
    TimestampZone,
    Text,
}

fn digits(s: &str) -> bool {
    !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit())
}

fn unsigned(s: &str) -> &str {
    s.strip_prefix(['-', '+']).unwrap_or(s)
}

fn is_decimal(s: &str) -> bool {
    match unsigned(s).split_once('.') {
        Some((a, b)) => digits(a) && digits(b),
        None => false,
    }
}

fn is_date(s: &str) -> bool {
    let b = s.as_bytes();
    b.len() == 10
        && b[4] == b'-'
        && b[7] == b'-'
        && digits(&s[..4])
        && digits(&s[5..7])
        && digits(&s[8..])
        && (1..=12).contains(&s[5..7].parse::<u8>().unwrap_or(0))
        && (1..=31).contains(&s[8..].parse::<u8>().unwrap_or(0))
}

fn timestamp(s: &str) -> Option<bool> {
    if s.len() < 16 || !s.is_char_boundary(10) || !is_date(&s[..10]) {
        return None;
    }
    let rest = s[10..].strip_prefix([' ', 'T'])?;
    let zone_at = rest
        .char_indices()
        .skip(5)
        .find(|(_, c)| matches!(c, 'Z' | 'z' | '+' | '-'))
        .map(|(i, _)| i);
    let (clock, zone) = zone_at.map_or((rest, ""), |i| rest.split_at(i));
    let mut parts = clock.split(':');
    let (h, m) = (parts.next()?, parts.next()?);
    if h.len() != 2 || m.len() != 2 || !digits(h) || !digits(m) {
        return None;
    }
    if let Some(sec) = parts.next() {
        let whole = sec
            .split_once('.')
            .map_or(sec, |(w, f)| if digits(f) { w } else { "" });
        if whole.len() != 2 || !digits(whole) {
            return None;
        }
    }
    if parts.next().is_some() {
        return None;
    }
    let zoned = !zone.is_empty();
    if zoned && !(zone.eq_ignore_ascii_case("z") || zone[1..].replace(':', "").len() == 4) {
        return None;
    }
    Some(zoned)
}

fn boolean(s: &str) -> bool {
    matches!(
        s.to_ascii_lowercase().as_str(),
        "true" | "false" | "t" | "f"
    )
}

fn classify(s: &str) -> Inferred {
    if digits(unsigned(s)) && !(unsigned(s).len() > 1 && unsigned(s).starts_with('0')) {
        return match s.parse::<i64>() {
            Ok(n) if i32::try_from(n).is_ok() => Inferred::Integer,
            Ok(_) => Inferred::BigInteger,
            Err(_) => Inferred::Decimal,
        };
    }
    if is_decimal(s) {
        return Inferred::Decimal;
    }
    if s.parse::<f64>().is_ok_and(f64::is_finite) && s.contains(['e', 'E']) {
        return Inferred::Float;
    }
    if boolean(s) {
        return Inferred::Boolean;
    }
    if is_date(s) {
        return Inferred::Date;
    }
    match timestamp(s) {
        Some(true) => Inferred::TimestampZone,
        Some(false) => Inferred::Timestamp,
        None => Inferred::Text,
    }
}

fn widen(a: Inferred, b: Inferred) -> Inferred {
    use Inferred::{BigInteger, Date, Decimal, Float, Integer, Text, Timestamp, TimestampZone};
    match (a, b) {
        _ if a == b => a,
        (Integer, BigInteger) | (BigInteger, Integer) => BigInteger,
        (Integer | BigInteger | Decimal, Integer | BigInteger | Decimal) => Decimal,
        (Integer | BigInteger | Decimal | Float, Integer | BigInteger | Decimal | Float) => Float,
        (Date | Timestamp, Date | Timestamp) => Timestamp,
        (Date | Timestamp | TimestampZone, Date | Timestamp | TimestampZone) => TimestampZone,
        _ => Text,
    }
}

pub fn infer<'a>(values: impl IntoIterator<Item = Option<&'a str>>) -> (Inferred, usize) {
    let mut out: Option<Inferred> = None;
    let mut longest = 0;
    for v in values.into_iter().flatten() {
        let v = v.trim();
        if v.is_empty() {
            continue;
        }
        longest = longest.max(v.chars().count());
        let t = classify(v);
        out = Some(out.map_or(t, |o| widen(o, t)));
    }
    (out.unwrap_or(Inferred::Text), longest)
}

#[allow(clippy::too_many_lines)]
pub fn sql_type(dialect: Dialect, t: Inferred, longest: usize) -> String {
    let s = match (dialect, t) {
        (Dialect::Postgres, Inferred::Integer) => "integer",
        (Dialect::Postgres, Inferred::BigInteger) => "bigint",
        (Dialect::Postgres, Inferred::Decimal) => "numeric",
        (Dialect::Postgres, Inferred::Float) => "double precision",
        (Dialect::Postgres, Inferred::Boolean) => "boolean",
        (Dialect::Postgres, Inferred::Date) => "date",
        (Dialect::Postgres, Inferred::Timestamp) => "timestamp",
        (Dialect::Postgres, Inferred::TimestampZone) => "timestamptz",
        (Dialect::Postgres, Inferred::Text) => "text",
        (Dialect::Mysql | Dialect::Mssql, Inferred::Integer) => "INT",
        (
            Dialect::Mysql
            | Dialect::Mssql
            | Dialect::Duckdb
            | Dialect::Firebird
            | Dialect::Hana
            | Dialect::Db2
            | Dialect::Ansi
            | Dialect::AnsiBacktick,
            Inferred::BigInteger,
        ) => "BIGINT",
        (
            Dialect::Sqlite
            | Dialect::Duckdb
            | Dialect::Firebird
            | Dialect::Hana
            | Dialect::Db2
            | Dialect::Ansi
            | Dialect::AnsiBacktick,
            Inferred::Integer,
        )
        | (Dialect::Sqlite, Inferred::BigInteger) => "INTEGER",
        (Dialect::Mysql, Inferred::Decimal) => "DECIMAL(30,10)",
        (Dialect::Mssql | Dialect::Duckdb, Inferred::Decimal) => "DECIMAL(38,10)",
        (Dialect::Firebird | Dialect::Hana, Inferred::Decimal) => "DECIMAL(18,6)",
        (Dialect::Sqlite, Inferred::Decimal) => "NUMERIC",
        (Dialect::Mysql | Dialect::Duckdb | Dialect::Hana | Dialect::Db2, Inferred::Float) => {
            "DOUBLE"
        }
        (Dialect::Firebird | Dialect::Ansi | Dialect::AnsiBacktick, Inferred::Float) => {
            "DOUBLE PRECISION"
        }
        (Dialect::Mssql, Inferred::Float) => "FLOAT",
        (Dialect::Sqlite, Inferred::Float) => "REAL",
        (
            Dialect::Mysql
            | Dialect::Sqlite
            | Dialect::Duckdb
            | Dialect::Firebird
            | Dialect::Hana
            | Dialect::Oracle
            | Dialect::Db2
            | Dialect::Ansi
            | Dialect::AnsiBacktick,
            Inferred::Boolean,
        ) => "BOOLEAN",
        (Dialect::Mssql, Inferred::Boolean) => "BIT",
        (
            Dialect::Mysql
            | Dialect::Sqlite
            | Dialect::Mssql
            | Dialect::Duckdb
            | Dialect::Firebird
            | Dialect::Hana
            | Dialect::Oracle
            | Dialect::Db2
            | Dialect::Ansi
            | Dialect::AnsiBacktick,
            Inferred::Date,
        ) => "DATE",
        (Dialect::Mysql, Inferred::Timestamp | Inferred::TimestampZone) => "DATETIME(6)",
        (
            Dialect::Sqlite | Dialect::Hana | Dialect::Db2 | Dialect::Ansi | Dialect::AnsiBacktick,
            Inferred::Timestamp | Inferred::TimestampZone,
        )
        | (Dialect::Duckdb | Dialect::Firebird | Dialect::Oracle, Inferred::Timestamp) => {
            "TIMESTAMP"
        }
        (Dialect::Mssql, Inferred::Timestamp) => "DATETIME2(6)",
        (Dialect::Mssql, Inferred::TimestampZone) => "DATETIMEOFFSET(6)",
        (Dialect::Duckdb, Inferred::TimestampZone) => "TIMESTAMPTZ",
        (Dialect::Firebird | Dialect::Oracle, Inferred::TimestampZone) => {
            "TIMESTAMP WITH TIME ZONE"
        }
        (Dialect::Mysql | Dialect::Firebird, Inferred::Text) if longest <= 255 => "VARCHAR(255)",
        (Dialect::Mssql, Inferred::Text) if longest <= 255 => "NVARCHAR(255)",
        (Dialect::Mssql, Inferred::Text) => "NVARCHAR(MAX)",
        (Dialect::Hana, Inferred::Text) if longest <= 5000 => "NVARCHAR(5000)",
        (Dialect::Mysql | Dialect::Sqlite, Inferred::Text) => "TEXT",
        (Dialect::Duckdb, Inferred::Text) => "VARCHAR",
        (Dialect::Firebird, Inferred::Text) => "BLOB SUB_TYPE TEXT",
        (Dialect::Hana, Inferred::Text) => "NCLOB",
        (Dialect::Oracle, Inferred::Integer) => "NUMBER(10)",
        (Dialect::Oracle, Inferred::BigInteger) => "NUMBER(19)",
        (Dialect::Oracle, Inferred::Decimal) => "NUMBER(38,10)",
        (Dialect::Oracle, Inferred::Float) => "BINARY_DOUBLE",
        (Dialect::Oracle, Inferred::Text) if longest <= 4000 => "VARCHAR2(4000 CHAR)",
        (Dialect::Clickhouse, Inferred::Integer) => "Nullable(Int32)",
        (Dialect::Clickhouse, Inferred::BigInteger) => "Nullable(Int64)",
        (Dialect::Clickhouse, Inferred::Decimal) => "Nullable(Decimal(38, 10))",
        (Dialect::Clickhouse, Inferred::Float) => "Nullable(Float64)",
        (Dialect::Clickhouse, Inferred::Boolean) => "Nullable(Bool)",
        (Dialect::Clickhouse, Inferred::Date) => "Nullable(Date32)",
        (Dialect::Clickhouse, Inferred::Timestamp) => "Nullable(DateTime64(6))",
        (Dialect::Clickhouse, Inferred::TimestampZone) => "Nullable(DateTime64(6, 'UTC'))",
        (Dialect::Clickhouse, Inferred::Text) => "Nullable(String)",
        (Dialect::Db2 | Dialect::Ansi | Dialect::AnsiBacktick, Inferred::Decimal) => {
            "DECIMAL(31,10)"
        }
        (Dialect::Db2 | Dialect::Ansi | Dialect::AnsiBacktick, Inferred::Text)
            if longest <= 4000 =>
        {
            "VARCHAR(4000)"
        }
        (
            Dialect::Oracle | Dialect::Db2 | Dialect::Ansi | Dialect::AnsiBacktick,
            Inferred::Text,
        ) => "CLOB",
        (Dialect::Cql | Dialect::Partiql | Dialect::Opensearch | Dialect::Elasticsearch, _) => {
            native_type(dialect, t)
        }
    };
    s.to_owned()
}

fn native_type(dialect: Dialect, t: Inferred) -> &'static str {
    let search = matches!(dialect, Dialect::Opensearch | Dialect::Elasticsearch);
    match (dialect, t) {
        (Dialect::Cql, Inferred::Integer) => "int",
        (Dialect::Cql, Inferred::BigInteger) => "bigint",
        (Dialect::Cql, Inferred::Decimal) => "decimal",
        (Dialect::Cql, Inferred::Timestamp | Inferred::TimestampZone) => "timestamp",
        (Dialect::Partiql, Inferred::Boolean) => "BOOL",
        (
            Dialect::Partiql,
            Inferred::Integer | Inferred::BigInteger | Inferred::Decimal | Inferred::Float,
        ) => "N",
        (Dialect::Partiql, _) => "S",
        (_, Inferred::Float) => "double",
        (_, Inferred::Decimal) if search => "double",
        (_, Inferred::Integer | Inferred::BigInteger) if search => "long",
        (_, Inferred::Boolean) => "boolean",
        (_, Inferred::Date) => "date",
        (_, Inferred::Timestamp | Inferred::TimestampZone) if search => "date",
        _ => "text",
    }
}
