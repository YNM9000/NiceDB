use duckdb::core::{LogicalTypeHandle, LogicalTypeId as Id};

pub(crate) fn name(t: &LogicalTypeHandle) -> String {
    if let Some(alias) = t.get_alias() {
        return alias;
    }
    let base = match t.id() {
        Id::Boolean => "BOOLEAN",
        Id::Tinyint => "TINYINT",
        Id::Smallint => "SMALLINT",
        Id::Integer => "INTEGER",
        Id::Bigint => "BIGINT",
        Id::UTinyint => "UTINYINT",
        Id::USmallint => "USMALLINT",
        Id::UInteger => "UINTEGER",
        Id::UBigint => "UBIGINT",
        Id::Hugeint => "HUGEINT",
        Id::UHugeint => "UHUGEINT",
        Id::Float => "FLOAT",
        Id::Double => "DOUBLE",
        Id::Timestamp => "TIMESTAMP",
        Id::TimestampS => "TIMESTAMP_S",
        Id::TimestampMs => "TIMESTAMP_MS",
        Id::TimestampNs => "TIMESTAMP_NS",
        Id::TimestampTZ => "TIMESTAMP WITH TIME ZONE",
        Id::Date => "DATE",
        Id::Time => "TIME",
        Id::TimeNs => "TIME_NS",
        Id::TimeTZ => "TIME WITH TIME ZONE",
        Id::Interval => "INTERVAL",
        Id::Varchar | Id::StringLiteral => "VARCHAR",
        Id::Blob => "BLOB",
        Id::Uuid => "UUID",
        Id::Bit => "BIT",
        Id::Enum => "ENUM",
        Id::Bignum => "BIGNUM",
        Id::Geometry => "GEOMETRY",
        Id::Variant => "VARIANT",
        Id::SqlNull => "\"NULL\"",
        Id::IntegerLiteral => "INTEGER_LITERAL",
        Id::Decimal => {
            return format!("DECIMAL({},{})", t.decimal_width(), t.decimal_scale());
        }
        Id::List | Id::Array => return format!("{}[]", name(&t.child(0))),
        Id::Map => return format!("MAP({}, {})", name(&t.child(0)), name(&t.child(1))),
        Id::Struct | Id::Union => {
            let fields: Vec<String> = (0..t.num_children())
                .map(|i| format!("{} {}", t.child_name(i), name(&t.child(i))))
                .collect();
            let kind = if t.id() == Id::Struct {
                "STRUCT"
            } else {
                "UNION"
            };
            return format!("{kind}({})", fields.join(", "));
        }
        _ => "UNKNOWN",
    };
    base.to_owned()
}
