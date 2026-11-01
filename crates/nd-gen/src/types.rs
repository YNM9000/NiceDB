use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Serialize, specta::Type)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Shape {
    Int {
        #[specta(type = f64)]
        min: i64,
        #[specta(type = f64)]
        max: i64,
    },
    Decimal {
        precision: u32,
        scale: u32,
    },
    Float,
    Bool,
    Date,
    DateTime,
    Time,
    Uuid,
    Text {
        max: Option<u32>,
    },
    Json,
    Binary,
    Enum {
        values: Vec<String>,
    },
    Other,
}

fn args(t: &str) -> Vec<u32> {
    let Some(open) = t.find('(') else {
        return Vec::new();
    };
    let close = t[open..].find(')').map_or(t.len(), |c| open + c);
    t[open + 1..close]
        .split(',')
        .filter_map(|a| a.trim().parse().ok())
        .collect()
}

pub fn quoted_list(s: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c != '\'' {
            continue;
        }
        let mut v = String::new();
        while let Some(c) = chars.next() {
            if c == '\'' {
                if chars.peek() == Some(&'\'') {
                    chars.next();
                    v.push('\'');
                } else {
                    break;
                }
            } else if c == '\\' {
                if let Some(n) = chars.next() {
                    v.push(n);
                }
            } else {
                v.push(c);
            }
        }
        out.push(v);
    }
    out
}

fn int(bits: u32, unsigned: bool) -> Shape {
    let (min, max) = match (bits, unsigned) {
        (8, false) => (i64::from(i8::MIN), i64::from(i8::MAX)),
        (8, true) => (0, i64::from(u8::MAX)),
        (16, false) => (i64::from(i16::MIN), i64::from(i16::MAX)),
        (16, true) => (0, i64::from(u16::MAX)),
        (24, false) => (-8_388_608, 8_388_607),
        (24, true) => (0, 16_777_215),
        (32, false) => (i64::from(i32::MIN), i64::from(i32::MAX)),
        (32, true) => (0, i64::from(u32::MAX)),
        (_, true) => (0, i64::MAX),
        _ => (i64::MIN, i64::MAX),
    };
    Shape::Int { min, max }
}

fn unwrapped(type_name: &str) -> &str {
    let mut t = type_name.trim();
    for wrap in ["Nullable(", "LowCardinality("] {
        if let Some(inner) = t.strip_prefix(wrap).and_then(|r| r.strip_suffix(')')) {
            t = unwrapped(inner);
        }
    }
    t
}

fn clickhouse_int(t: &str) -> Option<Shape> {
    let (unsigned, bits) = match t.strip_prefix("UInt") {
        Some(b) => (true, b),
        None => (false, t.strip_prefix("Int")?),
    };
    let bits: u32 = bits.parse().ok()?;
    matches!(bits, 8 | 16 | 32 | 64 | 128 | 256).then(|| int(bits.min(64), unsigned))
}

fn clickhouse_named(base: &str, a: &[u32]) -> Option<Shape> {
    let precision = match base {
        "date32" => return Some(Shape::Date),
        "float32" | "float64" => return Some(Shape::Float),
        "decimal32" => 9,
        "decimal64" => 18,
        "decimal128" | "decimal256" => 38,
        _ => return None,
    };
    let scale = a.first().copied().unwrap_or(0).min(precision);
    Some(Shape::Decimal { precision, scale })
}

pub fn shape(type_name: &str) -> Shape {
    let type_name = unwrapped(type_name);
    if let Some(s) = clickhouse_int(type_name) {
        return s;
    }
    let t = type_name.trim().to_ascii_lowercase();
    let head = t.split('(').next().unwrap_or("");
    let base = head
        .split_whitespace()
        .filter(|w| !matches!(*w, "unsigned" | "signed" | "zerofill"))
        .collect::<Vec<_>>()
        .join(" ");
    let base = base.as_str();
    let unsigned = t.contains("unsigned");
    let a = args(&t);
    if let Some(s) = clickhouse_named(base, &a) {
        return s;
    }
    if matches!(base, "enum" | "enum8" | "enum16") {
        return Shape::Enum {
            values: quoted_list(&type_name[type_name.find('(').unwrap_or(0)..]),
        };
    }
    if base == "tinyint" && a.first() == Some(&1) || matches!(base, "bit" | "bool" | "boolean") {
        return Shape::Bool;
    }
    match base {
        "tinyint" | "int1" | "utinyint" => return int(8, unsigned || base == "utinyint"),
        "smallint" | "int2" | "smallserial" | "usmallint" => {
            return int(16, unsigned || base == "usmallint");
        }
        "mediumint" => return int(24, unsigned),
        "int" | "integer" | "int4" | "serial" | "uinteger" => {
            return int(32, unsigned || base == "uinteger");
        }
        "bigint" | "int8" | "bigserial" | "ubigint" | "hugeint" | "int64" => {
            return int(64, unsigned || base == "ubigint");
        }
        "money" | "smallmoney" => {
            return Shape::Decimal {
                precision: 15,
                scale: 2,
            };
        }
        "uuid" | "uniqueidentifier" => return Shape::Uuid,
        "json" | "jsonb" => return Shape::Json,
        "date" => return Shape::Date,
        "year" => return int(16, true),
        _ => {}
    }
    if matches!(base, "numeric" | "decimal" | "number" | "dec") {
        let precision = a.first().copied().unwrap_or(12).clamp(1, 38);
        let scale = a.get(1).copied().unwrap_or(0).min(precision);
        return Shape::Decimal { precision, scale };
    }
    if matches!(
        base,
        "real" | "float" | "double" | "float4" | "float8" | "double precision"
    ) || base.starts_with("double")
    {
        return Shape::Float;
    }
    if base.starts_with("timestamp")
        || base.starts_with("datetime")
        || base == "smalldatetime"
        || base == "timestamptz"
    {
        return Shape::DateTime;
    }
    if base.starts_with("time") {
        return Shape::Time;
    }
    if matches!(
        base,
        "bytea"
            | "blob"
            | "binary"
            | "varbinary"
            | "image"
            | "longblob"
            | "mediumblob"
            | "tinyblob"
    ) {
        return Shape::Binary;
    }
    if base.contains("char")
        || base.contains("text")
        || base.contains("clob")
        || base == "string"
        || base == "fixedstring"
        || base == "citext"
        || base == "name"
    {
        let max = a.first().copied().filter(|n| *n > 0);
        return Shape::Text { max };
    }
    if base.contains("int") {
        return int(64, unsigned);
    }
    Shape::Other
}
