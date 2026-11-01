use serde::{Deserialize, Serialize};

use crate::Dialect;
use crate::dml::{Bound, Builder, ColumnType, TableRef};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "snake_case")]
pub enum Match {
    Exact,
    Contains,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
pub struct Kinds {
    pub text: bool,
    pub number: bool,
    pub other: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
pub struct SearchSpec {
    pub value: String,
    pub mode: Match,
    pub case_sensitive: bool,
    pub kinds: Kinds,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "snake_case")]
pub enum Family {
    Text,
    Integer,
    Decimal,
    Float,
    Other,
    Skip,
}

fn base(type_name: &str) -> String {
    let t = type_name.trim().to_ascii_lowercase();
    let t = t.split('(').next().unwrap_or_default().trim().to_owned();
    t.strip_suffix(" unsigned").map_or(t.clone(), str::to_owned)
}

fn vendor_family(dialect: Dialect, t: &str) -> Family {
    match dialect {
        Dialect::Db2 => match t {
            "varchar" | "char" | "character" | "clob" | "graphic" | "vargraphic" | "dbclob"
            | "long varchar" => Family::Text,
            "smallint" | "integer" | "int" | "bigint" => Family::Integer,
            "decimal" | "numeric" | "decfloat" => Family::Decimal,
            "real" | "double" | "float" => Family::Float,
            "blob" | "binary" | "varbinary" | "xml" => Family::Skip,
            _ => Family::Other,
        },
        _ => match t {
            "varchar" | "char" | "character" | "character varying" | "nvarchar" | "nchar"
            | "text" | "clob" => Family::Text,
            "smallint" | "integer" | "int" | "bigint" | "tinyint" => Family::Integer,
            "decimal" | "numeric" => Family::Decimal,
            "real" | "double" | "double precision" | "float" => Family::Float,
            "blob" | "binary" | "varbinary" | "longvarbinary" => Family::Skip,
            _ => Family::Other,
        },
    }
}

pub fn family(dialect: Dialect, type_name: &str) -> Family {
    let t = base(type_name);
    if t.ends_with("[]") {
        return Family::Other;
    }
    match dialect {
        Dialect::Postgres => match t.as_str() {
            "text" | "varchar" | "character varying" | "character" | "char" | "bpchar"
            | "citext" | "name" | "string" => Family::Text,
            "smallint" | "integer" | "int" | "bigint" | "int2" | "int4" | "int8" | "serial"
            | "bigserial" | "smallserial" => Family::Integer,
            "numeric" | "decimal" => Family::Decimal,
            "real" | "double precision" | "float4" | "float8" | "float" => Family::Float,
            "bytea" | "bytes" | "geometry" | "geography" => Family::Skip,
            _ => Family::Other,
        },
        Dialect::Mysql => match t.as_str() {
            "char" | "varchar" | "tinytext" | "text" | "mediumtext" | "longtext" | "enum"
            | "set" => Family::Text,
            "tinyint" | "smallint" | "mediumint" | "int" | "integer" | "bigint" => Family::Integer,
            "decimal" | "numeric" | "dec" | "fixed" => Family::Decimal,
            "float" | "double" | "real" | "double precision" => Family::Float,
            "binary" | "varbinary" | "tinyblob" | "blob" | "mediumblob" | "longblob" | "bit"
            | "geometry" | "point" | "linestring" | "polygon" | "multipoint"
            | "multilinestring" | "multipolygon" | "geometrycollection" | "geomcollection" => {
                Family::Skip
            }
            _ => Family::Other,
        },
        Dialect::Mssql => match t.as_str() {
            "char" | "varchar" | "nchar" | "nvarchar" | "text" | "ntext" | "sysname" => {
                Family::Text
            }
            "tinyint" | "smallint" | "int" | "bigint" => Family::Integer,
            "decimal" | "numeric" | "money" | "smallmoney" => Family::Decimal,
            "float" | "real" => Family::Float,
            "binary" | "varbinary" | "image" | "timestamp" | "rowversion" | "geography"
            | "geometry" => Family::Skip,
            _ => Family::Other,
        },
        Dialect::Duckdb => match t.as_str() {
            "varchar" | "text" | "string" | "char" | "bpchar" => Family::Text,
            "tinyint" | "smallint" | "integer" | "int" | "bigint" | "hugeint" | "utinyint"
            | "usmallint" | "uinteger" | "ubigint" | "uhugeint" | "int1" | "int2" | "int4"
            | "int8" => Family::Integer,
            "decimal" | "numeric" => Family::Decimal,
            "double" | "float" | "real" | "float4" | "float8" => Family::Float,
            "blob" | "bytea" | "binary" | "varbinary" | "bit" | "bitstring" => Family::Skip,
            _ => Family::Other,
        },
        Dialect::Clickhouse => clickhouse_family(type_name),
        Dialect::Cql | Dialect::Partiql | Dialect::Opensearch | Dialect::Elasticsearch => {
            Family::Skip
        }
        Dialect::Firebird => match t.as_str() {
            "char" | "varchar" => Family::Text,
            "smallint" | "integer" | "bigint" | "int128" => Family::Integer,
            "numeric" | "decimal" | "decfloat" => Family::Decimal,
            "float" | "double precision" | "real" => Family::Float,
            "blob sub_type binary" | "binary" | "varbinary" => Family::Skip,
            _ => Family::Other,
        },
        Dialect::Oracle => match t.as_str() {
            "varchar2" | "nvarchar2" | "varchar" | "char" | "nchar" | "clob" | "nclob" | "long" => {
                Family::Text
            }
            "integer" | "int" | "smallint" => Family::Integer,
            "number" | "decimal" | "numeric" => Family::Decimal,
            "binary_float" | "binary_double" | "float" => Family::Float,
            "raw" | "long raw" | "blob" | "bfile" | "sdo_geometry" | "vector" => Family::Skip,
            _ => Family::Other,
        },
        Dialect::Db2 | Dialect::Ansi | Dialect::AnsiBacktick => vendor_family(dialect, &t),
        Dialect::Hana => match t.as_str() {
            "varchar" | "nvarchar" | "char" | "nchar" | "alphanum" | "shorttext" => Family::Text,
            "tinyint" | "smallint" | "integer" | "bigint" => Family::Integer,
            "decimal" | "smalldecimal" => Family::Decimal,
            "real" | "double" | "float" => Family::Float,
            "binary" | "varbinary" | "blob" | "st_geometry" | "st_point" => Family::Skip,
            _ => Family::Other,
        },
        Dialect::Sqlite if t.contains("int") => Family::Integer,
        Dialect::Sqlite if t.contains("char") || t.contains("clob") || t.contains("text") => {
            Family::Text
        }
        Dialect::Sqlite if t.contains("blob") => Family::Skip,
        Dialect::Sqlite if t.is_empty() => Family::Text,
        Dialect::Sqlite if t.contains("real") || t.contains("floa") || t.contains("doub") => {
            Family::Float
        }
        Dialect::Sqlite if t.contains("dec") || t.contains("num") => Family::Decimal,
        Dialect::Sqlite => Family::Other,
    }
}

pub(crate) fn clickhouse_base(type_name: &str) -> &str {
    let mut t = type_name.trim();
    loop {
        let inner = ["Nullable(", "LowCardinality("]
            .iter()
            .find_map(|w| t.strip_prefix(w).and_then(|r| r.strip_suffix(')')));
        match inner {
            Some(i) => t = i.trim(),
            None => return t,
        }
    }
}

fn clickhouse_family(type_name: &str) -> Family {
    let t = clickhouse_base(type_name);
    let head = t.split('(').next().unwrap_or_default();
    match head {
        "String" | "FixedString" | "Enum8" | "Enum16" | "UUID" => Family::Text,
        "UInt8" | "UInt16" | "UInt32" | "UInt64" | "UInt128" | "UInt256" | "Int8" | "Int16"
        | "Int32" | "Int64" | "Int128" | "Int256" => Family::Integer,
        "Decimal" | "Decimal32" | "Decimal64" | "Decimal128" | "Decimal256" => Family::Decimal,
        "Float32" | "Float64" => Family::Float,
        "AggregateFunction" | "SimpleAggregateFunction" | "Nothing" => Family::Skip,
        _ => Family::Other,
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Number {
    pub text: String,
    pub integer: bool,
}

pub fn number(value: &str) -> Option<Number> {
    let v = value.trim();
    let v = if v.contains('.') {
        v.to_owned()
    } else {
        v.replacen(',', ".", 1)
    };
    let digits = v.strip_prefix(['-', '+']).unwrap_or(&v);
    let (int, frac) = digits.split_once('.').unwrap_or((digits, ""));
    let ok = !int.is_empty()
        && int.bytes().all(|b| b.is_ascii_digit())
        && frac.bytes().all(|b| b.is_ascii_digit())
        && !(digits.contains('.') && frac.is_empty());
    ok.then(|| Number {
        integer: frac.is_empty() && int.len() <= 18,
        text: v.strip_prefix('+').unwrap_or(&v).to_owned(),
    })
}

pub fn searchable(dialect: Dialect, spec: &SearchSpec, type_name: &str) -> bool {
    if spec.value.is_empty() {
        return false;
    }
    let n = number(&spec.value);
    match family(dialect, type_name) {
        Family::Skip => false,
        Family::Text => spec.kinds.text,
        Family::Other => spec.kinds.other,
        Family::Integer => {
            spec.kinds.number && n.is_some_and(|n| n.integer || spec.mode == Match::Contains)
        }
        Family::Decimal | Family::Float => spec.kinds.number && n.is_some(),
    }
}

fn as_text(q: &mut Builder, column: &ColumnType, fam: Family) {
    let lob = matches!(
        (q.dialect, base(&column.type_name).as_str()),
        (Dialect::Mssql, "text" | "ntext") | (Dialect::Oracle, "clob" | "nclob" | "long")
    );
    let plain = fam == Family::Text && !lob;
    if plain || q.dialect == Dialect::Sqlite {
        q.ident(&column.name);
        return;
    }
    match q.dialect {
        Dialect::Postgres => {
            q.ident(&column.name).text("::text");
        }
        Dialect::Duckdb => {
            q.text("CAST(").ident(&column.name).text(" AS VARCHAR)");
        }
        Dialect::Mysql => {
            q.text("CAST(").ident(&column.name).text(" AS CHAR)");
        }
        Dialect::Mssql => {
            q.text("CAST(")
                .ident(&column.name)
                .text(" AS nvarchar(max))");
        }
        Dialect::Clickhouse => {
            q.text("toString(").ident(&column.name).text(")");
        }
        Dialect::Firebird => {
            q.text("CAST(")
                .ident(&column.name)
                .text(" AS VARCHAR(8191))");
        }
        Dialect::Hana => {
            q.text("TO_NVARCHAR(").ident(&column.name).text(")");
        }
        Dialect::Db2 => {
            q.text("VARCHAR(").ident(&column.name).text(")");
        }
        Dialect::Ansi | Dialect::AnsiBacktick => {
            q.text("CAST(")
                .ident(&column.name)
                .text(" AS VARCHAR(4000))");
        }
        Dialect::Oracle if lob => {
            q.text("DBMS_LOB.SUBSTR(")
                .ident(&column.name)
                .text(", 4000, 1)");
        }
        Dialect::Oracle => {
            q.text("TO_CHAR(").ident(&column.name).text(")");
        }
        Dialect::Sqlite
        | Dialect::Cql
        | Dialect::Partiql
        | Dialect::Opensearch
        | Dialect::Elasticsearch => {}
    }
}

fn lowered(q: &mut Builder, inner: impl FnOnce(&mut Builder)) {
    q.text("lower(");
    inner(q);
    q.text(")");
}

fn exact_value(q: &mut Builder, v: &str) {
    match q.dialect {
        Dialect::Mysql => {
            q.text("CONVERT(")
                .value(Some(v))
                .text(" USING utf8mb4) COLLATE utf8mb4_bin");
        }
        _ => {
            q.value(Some(v));
        }
    }
}

fn text_condition(q: &mut Builder, spec: &SearchSpec, column: &ColumnType, fam: Family) {
    let v = spec.value.as_str();
    let col = |q: &mut Builder| as_text(q, column, fam);
    let bin = |q: &mut Builder| {
        col(q);
        if q.dialect == Dialect::Mssql {
            q.text(" COLLATE Latin1_General_100_BIN2");
        }
    };
    match (spec.mode, spec.case_sensitive) {
        (Match::Exact, true) => {
            bin(q);
            q.text(" = ");
            exact_value(q, v);
        }
        (Match::Exact, false) => {
            lowered(q, col);
            q.text(" = ");
            lowered(q, |q| {
                q.value(Some(v));
            });
        }
        (Match::Contains, sensitive) => {
            let (open, needle_first) = match q.dialect {
                Dialect::Postgres => ("strpos(", false),
                Dialect::Duckdb | Dialect::Sqlite => ("instr(", false),
                Dialect::Mysql
                | Dialect::Cql
                | Dialect::Partiql
                | Dialect::Opensearch
                | Dialect::Elasticsearch => ("LOCATE(", true),
                Dialect::Mssql => ("CHARINDEX(", true),
                Dialect::Clickhouse => ("positionUTF8(", false),
                Dialect::Firebird | Dialect::Ansi | Dialect::AnsiBacktick => ("POSITION(", true),
                Dialect::Hana | Dialect::Db2 => ("LOCATE(", false),
                Dialect::Oracle => ("INSTR(", false),
            };
            let hay = |q: &mut Builder| {
                if sensitive {
                    bin(q);
                } else {
                    lowered(q, col);
                }
            };
            let needle = |q: &mut Builder| {
                if sensitive {
                    exact_value(q, v);
                } else {
                    lowered(q, |q| {
                        q.value(Some(v));
                    });
                }
            };
            if sensitive && q.dialect == Dialect::Mysql {
                q.text("LOCATE(CAST(CONVERT(").value(Some(v));
                q.text(" USING utf8mb4) AS BINARY), CAST(CONVERT(");
                col(q);
                q.text(" USING utf8mb4) AS BINARY)) > 0");
                return;
            }
            q.text(open);
            if needle_first {
                needle(q);
                q.text(", ");
                hay(q);
            } else {
                hay(q);
                q.text(", ");
                needle(q);
            }
            q.text(") > 0");
        }
    }
}

fn number_cast(dialect: Dialect, fam: Family) -> (&'static str, &'static str) {
    match (dialect, fam) {
        (Dialect::Postgres, Family::Integer) => ("", "::bigint"),
        (Dialect::Postgres, Family::Float) => ("", "::float8"),
        (Dialect::Postgres, _) => ("", "::numeric"),
        (Dialect::Mysql, Family::Integer) => ("CAST(", " AS SIGNED)"),
        (Dialect::Mysql, _) => ("CAST(", " AS DECIMAL(65,30))"),
        (Dialect::Mssql, Family::Integer) => ("CAST(", " AS bigint)"),
        (Dialect::Mssql, Family::Float) => ("CAST(", " AS float)"),
        (Dialect::Mssql, _) => ("CAST(", " AS decimal(38,10))"),
        (Dialect::Sqlite, Family::Integer) => ("CAST(", " AS INTEGER)"),
        (Dialect::Sqlite, _) => ("CAST(", " AS REAL)"),
        (Dialect::Clickhouse, Family::Integer) => ("toInt64OrNull(", ")"),
        (Dialect::Clickhouse, Family::Float) => ("toFloat64OrNull(", ")"),
        (Dialect::Clickhouse, _) => ("toDecimal128OrNull(", ", 10)"),
        (
            Dialect::Duckdb
            | Dialect::Firebird
            | Dialect::Hana
            | Dialect::Db2
            | Dialect::Ansi
            | Dialect::AnsiBacktick,
            Family::Integer,
        ) => ("CAST(", " AS BIGINT)"),
        (Dialect::Firebird | Dialect::Ansi | Dialect::AnsiBacktick, Family::Float) => {
            ("CAST(", " AS DOUBLE PRECISION)")
        }
        (Dialect::Duckdb | Dialect::Hana | Dialect::Db2, Family::Float) => ("CAST(", " AS DOUBLE)"),
        (Dialect::Duckdb | Dialect::Firebird | Dialect::Hana, _) => {
            ("CAST(", " AS DECIMAL(38,10))")
        }
        (Dialect::Db2 | Dialect::Ansi | Dialect::AnsiBacktick, _) => {
            ("CAST(", " AS DECIMAL(31,10))")
        }
        (Dialect::Oracle, Family::Float) => ("CAST(", " AS BINARY_DOUBLE)"),
        (Dialect::Oracle, _) => ("CAST(", " AS NUMBER)"),
        (Dialect::Cql | Dialect::Partiql | Dialect::Opensearch | Dialect::Elasticsearch, _) => {
            ("", "")
        }
    }
}

fn condition(q: &mut Builder, spec: &SearchSpec, column: &ColumnType) {
    let fam = family(q.dialect, &column.type_name);
    let numeric = matches!(fam, Family::Integer | Family::Decimal | Family::Float);
    match (numeric, spec.mode, number(&spec.value)) {
        (true, Match::Exact, Some(n)) => {
            let (open, close) = number_cast(q.dialect, fam);
            q.ident(&column.name).text(" = ").text(open);
            q.value(Some(&n.text)).text(close);
        }
        (true, _, n) => {
            let text = n.map_or_else(|| spec.value.clone(), |n| n.text);
            let as_number = SearchSpec {
                value: text,
                ..spec.clone()
            };
            text_condition(q, &as_number, column, Family::Other);
        }
        _ => text_condition(q, spec, column, fam),
    }
}

fn any_of(q: &mut Builder, spec: &SearchSpec, columns: &[ColumnType]) {
    q.text("(");
    q.list(columns, " OR ", |q, c| condition(q, spec, c));
    q.text(")");
}

pub fn search_count(
    dialect: Dialect,
    table: &TableRef,
    columns: &[ColumnType],
    spec: &SearchSpec,
) -> Bound {
    let mut q = Builder::new(dialect);
    let cast = if dialect == Dialect::Postgres {
        "::text"
    } else {
        ""
    };
    let count = if dialect == Dialect::Mssql {
        "COUNT_BIG"
    } else {
        "count"
    };
    q.text(&format!("SELECT {count}(*){cast}"));
    for c in columns {
        q.text(&format!(", {count}(CASE WHEN "));
        condition(&mut q, spec, c);
        q.text(&format!(" THEN 1 END){cast}"));
    }
    q.text(" FROM ").table(table).text(" WHERE ");
    any_of(&mut q, spec, columns);
    q.b
}

pub fn search_rows(
    dialect: Dialect,
    table: &TableRef,
    shown: &[String],
    columns: &[ColumnType],
    spec: &SearchSpec,
    limit: u32,
) -> Bound {
    let mut q = Builder::new(dialect);
    q.text("SELECT ");
    if dialect == Dialect::Mssql {
        q.text(&format!("TOP ({limit}) "));
    }
    q.list(shown, ", ", |q, c| {
        q.ident(c);
        if dialect == Dialect::Postgres {
            q.text("::text AS ").ident(c);
        }
    });
    q.text(" FROM ").table(table).text(" WHERE ");
    any_of(&mut q, spec, columns);
    match dialect {
        Dialect::Mssql => {}
        Dialect::Oracle
        | Dialect::Firebird
        | Dialect::Db2
        | Dialect::Ansi
        | Dialect::AnsiBacktick => {
            q.text(&format!(" FETCH FIRST {limit} ROWS ONLY"));
        }
        _ => {
            q.text(&format!(" LIMIT {limit}"));
        }
    }
    q.b
}
