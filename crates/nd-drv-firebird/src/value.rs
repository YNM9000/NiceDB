use std::fmt::Write;

use rsfbclient_core::SqlType;
use rsfbclient_rust::ColumnInfo;

pub(crate) fn display(v: &SqlType) -> Option<String> {
    match v {
        SqlType::Null => None,
        SqlType::Text(s) => Some(s.clone()),
        SqlType::Integer(i) => Some(i.to_string()),
        SqlType::Int128(i) => Some(i.to_string()),
        SqlType::Floating(f) => Some(f.to_string()),
        SqlType::Boolean(b) => Some(b.to_string()),
        SqlType::Timestamp(t) => {
            let s = t.format("%Y-%m-%d %H:%M:%S%.f").to_string();
            Some(s.strip_suffix(" 00:00:00").map_or(s.clone(), str::to_owned))
        }
        SqlType::Binary(b) => {
            let mut s = String::with_capacity(2 + b.len() * 2);
            s.push_str("\\x");
            for byte in b {
                let _ = write!(s, "{byte:02x}");
            }
            Some(s)
        }
    }
}

pub(crate) fn type_name(c: &ColumnInfo) -> String {
    let scale = -i32::from(c.scale);
    let exact = |base: &str, precision: u8| {
        if scale > 0 {
            format!("NUMERIC({precision},{scale})")
        } else {
            base.to_owned()
        }
    };
    let chars = |len: i16| {
        let len = i32::from(len.max(0));
        if c.subtype & 0xff == 4 { len / 4 } else { len }
    };
    match c.sqltype & !1 {
        452 => format!("CHAR({})", chars(c.length)),
        448 => format!("VARCHAR({})", chars(c.length)),
        500 => exact("SMALLINT", 4),
        496 => exact("INTEGER", 9),
        580 => exact("BIGINT", 18),
        32752 => exact("INT128", 38),
        482 => "FLOAT".into(),
        480 | 530 => "DOUBLE PRECISION".into(),
        510 => "TIMESTAMP".into(),
        570 => "DATE".into(),
        560 => "TIME".into(),
        32754 | 32748 => "TIMESTAMP WITH TIME ZONE".into(),
        32756 | 32750 => "TIME WITH TIME ZONE".into(),
        32760 => "DECFLOAT(16)".into(),
        32762 => "DECFLOAT(34)".into(),
        32764 => "BOOLEAN".into(),
        520 if c.subtype == 1 => "BLOB SUB_TYPE TEXT".into(),
        520 => "BLOB".into(),
        other => format!("TYPE {other}"),
    }
}

pub(crate) fn param(v: Option<&String>) -> SqlType {
    v.map_or(SqlType::Null, |s| SqlType::Text(s.clone()))
}
