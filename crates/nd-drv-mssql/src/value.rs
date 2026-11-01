use std::fmt::Write as _;

use tiberius::numeric::Numeric;
use tiberius::time::{Date, DateTime, DateTime2, DateTimeOffset, SmallDateTime, Time};
use tiberius::{ColumnData, ColumnType};

const DAYS_0001_TO_1970: i64 = 719_162;
const DAYS_1900_TO_1970: i64 = 25_567;

pub(crate) fn text(data: ColumnData<'_>) -> Option<String> {
    Some(match data {
        ColumnData::U8(v) => v?.to_string(),
        ColumnData::I16(v) => v?.to_string(),
        ColumnData::I32(v) => v?.to_string(),
        ColumnData::I64(v) => v?.to_string(),
        ColumnData::F32(v) => float(v?, 1e7),
        ColumnData::F64(v) => float(v?, 1e16),
        ColumnData::Bit(v) => if v? { "1" } else { "0" }.to_owned(),
        ColumnData::String(v) => v?.into_owned(),
        ColumnData::Guid(v) => v?.to_string().to_ascii_uppercase(),
        ColumnData::Binary(v) => hex(&v?),
        ColumnData::Numeric(v) => numeric(v?),
        ColumnData::Xml(v) => v?.to_string(),
        ColumnData::DateTime(v) => datetime(v?),
        ColumnData::SmallDateTime(v) => small_datetime(v?),
        ColumnData::Time(v) => time(v?),
        ColumnData::Date(v) => date(v?),
        ColumnData::DateTime2(v) => datetime2(v?),
        ColumnData::DateTimeOffset(v) => datetime_offset(v?),
    })
}

pub(crate) fn typed_text(data: ColumnData<'_>, ty: Option<ColumnType>) -> Option<String> {
    match (data, ty) {
        (ColumnData::F64(v), Some(ColumnType::Money | ColumnType::Money4)) => {
            Some(format!("{:.4}", v?))
        }
        (data, _) => text(data),
    }
}

pub(crate) fn type_name(t: ColumnType) -> &'static str {
    match t {
        ColumnType::Null => "null",
        ColumnType::Bit | ColumnType::Bitn => "bit",
        ColumnType::Int1 => "tinyint",
        ColumnType::Int2 => "smallint",
        ColumnType::Int4 | ColumnType::Intn => "int",
        ColumnType::Int8 => "bigint",
        ColumnType::Datetime4 => "smalldatetime",
        ColumnType::Float4 => "real",
        ColumnType::Float8 | ColumnType::Floatn => "float",
        ColumnType::Money => "money",
        ColumnType::Money4 => "smallmoney",
        ColumnType::Datetime | ColumnType::Datetimen => "datetime",
        ColumnType::Guid => "uniqueidentifier",
        ColumnType::Decimaln => "decimal",
        ColumnType::Numericn => "numeric",
        ColumnType::Daten => "date",
        ColumnType::Timen => "time",
        ColumnType::Datetime2 => "datetime2",
        ColumnType::DatetimeOffsetn => "datetimeoffset",
        ColumnType::BigVarBin => "varbinary",
        ColumnType::BigVarChar => "varchar",
        ColumnType::BigBinary => "binary",
        ColumnType::BigChar => "char",
        ColumnType::NVarchar => "nvarchar",
        ColumnType::NChar => "nchar",
        ColumnType::Xml => "xml",
        ColumnType::Udt => "udt",
        ColumnType::Text => "text",
        ColumnType::Image => "image",
        ColumnType::NText => "ntext",
        ColumnType::SSVariant => "sql_variant",
    }
}

fn float<T>(v: T, large: f64) -> String
where
    T: Copy + Into<f64> + std::fmt::Display + std::fmt::UpperExp,
{
    let abs = v.into().abs();
    if abs != 0.0 && (abs >= large || abs < 1e-4) {
        let s = format!("{v:E}");
        match s.split_once('E') {
            Some((m, e)) if e.starts_with('-') => format!("{m}E{e}"),
            Some((m, e)) => format!("{m}E+{e}"),
            None => s,
        }
    } else {
        v.to_string()
    }
}

fn hex(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(2 + bytes.len() * 2);
    s.push_str("0x");
    for b in bytes {
        let _ = write!(s, "{b:02X}");
    }
    s
}

fn numeric(n: Numeric) -> String {
    let scale = u32::from(n.scale());
    let digits = n.value().unsigned_abs().to_string();
    let sign = if n.value() < 0 { "-" } else { "" };
    if scale == 0 {
        return format!("{sign}{digits}");
    }
    let width = scale as usize + 1;
    let padded = format!("{digits:0>width$}");
    let (int, frac) = padded.split_at(padded.len() - scale as usize);
    format!("{sign}{int}.{frac}")
}

fn civil(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = u32::try_from(doy - (153 * mp + 2) / 5 + 1).unwrap_or(1);
    let month = u32::try_from(if mp < 10 { mp + 3 } else { mp - 9 }).unwrap_or(1);
    let year = yoe + era * 400 + i64::from(month <= 2);
    (year, month, day)
}

fn ymd(days_since_1970: i64) -> String {
    let (y, m, d) = civil(days_since_1970);
    format!("{y:04}-{m:02}-{d:02}")
}

fn date(d: Date) -> String {
    ymd(i64::from(d.days()) - DAYS_0001_TO_1970)
}

fn hms(seconds: u64) -> String {
    format!(
        "{:02}:{:02}:{:02}",
        seconds / 3600,
        seconds / 60 % 60,
        seconds % 60
    )
}

fn time(t: Time) -> String {
    let scale = u32::from(t.scale());
    let per_second = 10u64.pow(scale);
    let whole = t.increments() / per_second;
    let mut s = hms(whole);
    if scale > 0 {
        let frac = t.increments() % per_second;
        let _ = write!(s, ".{frac:0width$}", width = scale as usize);
    }
    s
}

fn datetime(dt: DateTime) -> String {
    let ms = (u64::from(dt.seconds_fragments()) * 10 + 1) / 3;
    format!(
        "{} {}.{:03}",
        ymd(i64::from(dt.days()) - DAYS_1900_TO_1970),
        hms(ms / 1000),
        ms % 1000
    )
}

fn small_datetime(dt: SmallDateTime) -> String {
    format!(
        "{} {}",
        ymd(i64::from(dt.days()) - DAYS_1900_TO_1970),
        hms(u64::from(dt.seconds_fragments()) * 60)
    )
}

fn datetime2(dt: DateTime2) -> String {
    format!("{} {}", date(dt.date()), time(dt.time()))
}

fn datetime_offset(dto: DateTimeOffset) -> String {
    let dt = dto.datetime2();
    let t = dt.time();
    let per_second = 10u64.pow(u32::from(t.scale()));
    let offset = i64::from(dto.offset());
    let utc_seconds = i64::try_from(t.increments() / per_second).unwrap_or(0);
    let total = i64::from(dt.date().days()) * 86_400 + utc_seconds + offset * 60;
    let days = total.div_euclid(86_400);
    let seconds = u64::try_from(total.rem_euclid(86_400)).unwrap_or(0);
    let local = Time::new(
        seconds * per_second + t.increments() % per_second,
        t.scale(),
    );
    let sign = if offset < 0 { '-' } else { '+' };
    let off = offset.unsigned_abs();
    format!(
        "{} {} {sign}{:02}:{:02}",
        ymd(days - DAYS_0001_TO_1970),
        time(local),
        off / 60,
        off % 60
    )
}
