use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    Integer,
    Decimal,
    Float,
    Boolean,
    Json,
    Text,
}

pub fn kind_of(type_name: Option<&str>) -> Kind {
    let Some(t) = type_name else {
        return Kind::Text;
    };
    if t.trim() == "bit" {
        return Kind::Boolean;
    }
    let t = t.trim().to_ascii_lowercase();
    let base = t.split(['(', ' ']).next().unwrap_or_default();
    if t.ends_with("[]") || t.starts_with('_') {
        return Kind::Text;
    }
    match base {
        "tinyint" if t.starts_with("tinyint(1)") => Kind::Boolean,
        "int2" | "int4" | "int8" | "smallint" | "integer" | "int" | "bigint" | "mediumint"
        | "tinyint" | "serial" | "bigserial" | "smallserial" | "oid" | "year" | "hugeint"
        | "uhugeint" | "utinyint" | "usmallint" | "uinteger" | "ubigint" => Kind::Integer,
        "numeric" | "decimal" | "money" | "smallmoney" => Kind::Decimal,
        "float4" | "float8" | "real" | "double" | "float" => Kind::Float,
        "bool" | "boolean" => Kind::Boolean,
        "json" | "jsonb" => Kind::Json,
        _ => Kind::Text,
    }
}
