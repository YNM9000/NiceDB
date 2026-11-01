use serde::{Deserialize, Serialize};

use crate::Dialect;
use crate::quote::{self, ident, literal, placeholder};
use crate::split::split;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
pub struct TableRef {
    pub namespace: String,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
pub struct Field {
    pub column: String,
    pub value: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Change {
    Update {
        key: Vec<Field>,
        original: Vec<Field>,
        set: Vec<Field>,
    },
    Insert {
        values: Vec<Field>,
    },
    Delete {
        key: Vec<Field>,
        original: Vec<Field>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
pub struct ColumnType {
    pub name: String,
    pub type_name: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
pub struct ChangeSet {
    pub table: TableRef,
    pub columns: Vec<ColumnType>,
    pub changes: Vec<Change>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "snake_case")]
pub enum FilterOp {
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
    Contains,
    IsNull,
    NotNull,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
pub struct QuickFilter {
    pub column: String,
    pub op: FilterOp,
    pub value: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
pub struct Order {
    pub column: String,
    pub descending: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
pub struct Browse {
    pub table: TableRef,
    pub columns: Vec<String>,
    pub key: Vec<String>,
    pub filter: Option<String>,
    pub quick: Vec<QuickFilter>,
    pub order: Vec<Order>,
    pub limit: u32,
    #[specta(type = f64)]
    pub offset: u64,
    #[serde(default)]
    pub after: Option<Vec<Option<String>>>,
    #[serde(default)]
    pub search: Option<String>,
}

pub fn keyset(browse: &Browse) -> Option<(Vec<String>, bool)> {
    if browse.key.is_empty() || !browse.order.iter().all(|o| browse.key.contains(&o.column)) {
        return None;
    }
    let descending = browse.order.first().is_some_and(|o| o.descending);
    let tie: Vec<&String> = browse
        .key
        .iter()
        .filter(|k| !browse.order.iter().any(|o| &o.column == *k))
        .collect();
    let uniform = browse.order.iter().all(|o| o.descending == descending);
    if !uniform || (descending && !tie.is_empty()) {
        return None;
    }
    let columns = browse
        .order
        .iter()
        .map(|o| o.column.clone())
        .chain(tie.into_iter().cloned())
        .collect();
    Some((columns, descending))
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Bound {
    pub sql: String,
    pub params: Vec<Option<String>>,
    pub preview: String,
}

pub(crate) struct Builder {
    pub(crate) dialect: Dialect,
    pub(crate) b: Bound,
}

impl Builder {
    pub(crate) fn new(dialect: Dialect) -> Self {
        Self {
            dialect,
            b: Bound {
                sql: String::new(),
                params: Vec::new(),
                preview: String::new(),
            },
        }
    }

    pub(crate) fn text(&mut self, s: &str) -> &mut Self {
        self.b.sql.push_str(s);
        self.b.preview.push_str(s);
        self
    }

    pub(crate) fn ident(&mut self, name: &str) -> &mut Self {
        let q = ident(self.dialect, name);
        self.text(&q)
    }

    pub(crate) fn value(&mut self, v: Option<&str>) -> &mut Self {
        self.b.params.push(v.map(str::to_owned));
        self.b
            .sql
            .push_str(&placeholder(self.dialect, self.b.params.len()));
        self.b.preview.push_str(&literal(self.dialect, v));
        self
    }

    pub(crate) fn table(&mut self, t: &TableRef) -> &mut Self {
        if !t.namespace.is_empty() && self.dialect != Dialect::Firebird {
            self.ident(&t.namespace).text(".");
        }
        self.ident(&t.name)
    }

    pub(crate) fn list<T>(
        &mut self,
        items: &[T],
        sep: &str,
        mut f: impl FnMut(&mut Self, &T),
    ) -> &mut Self {
        for (i, item) in items.iter().enumerate() {
            if i > 0 {
                self.text(sep);
            }
            f(self, item);
        }
        self
    }

    fn matches(&mut self, f: &Field, kind: Kind, role: Role) {
        let Some(v) = f.value.as_deref() else {
            self.ident(&f.column).text(" IS NULL");
            return;
        };
        match (self.dialect, kind, role) {
            (Dialect::Postgres | Dialect::Duckdb, _, Role::Original) => {
                self.ident(&f.column).text("::text = ").value(Some(v));
            }
            (Dialect::Clickhouse, _, Role::Original) => {
                self.text("toString(").ident(&f.column).text(") = ");
                self.value(Some(v));
            }
            (Dialect::Mssql, Kind::Text, Role::Original) => {
                self.ident(&f.column)
                    .text(" COLLATE Latin1_General_100_BIN2 = ");
                self.value(Some(v));
            }
            (Dialect::Mssql | Dialect::Hana | Dialect::Oracle | Dialect::Db2, Kind::Binary, _) => {
                self.ident(&f.column).text(" = ");
                self.store(Kind::Binary, Some(v));
            }
            (
                Dialect::Postgres
                | Dialect::Duckdb
                | Dialect::Mssql
                | Dialect::Clickhouse
                | Dialect::Firebird
                | Dialect::Hana
                | Dialect::Oracle
                | Dialect::Db2
                | Dialect::Ansi
                | Dialect::AnsiBacktick
                | Dialect::Cql
                | Dialect::Partiql
                | Dialect::Opensearch
                | Dialect::Elasticsearch,
                _,
                _,
            )
            | (Dialect::Sqlite, Kind::Typed, Role::Key) => {
                self.ident(&f.column).text(" = ").value(Some(v));
            }
            (Dialect::Mysql, Kind::Json, _) => {
                self.text("JSON_EXTRACT(")
                    .ident(&f.column)
                    .text(", '$') = JSON_EXTRACT(");
                self.value(Some(v)).text(", '$')");
            }
            (Dialect::Mysql, Kind::Int, Role::Original) => {
                self.text("CAST(")
                    .ident(&f.column)
                    .text(" AS CHAR) = ")
                    .value(Some(v));
            }
            (Dialect::Mysql, Kind::Text, Role::Original) => {
                self.ident(&f.column).text(" = CONVERT(").value(Some(v));
                self.text(" USING utf8mb4) COLLATE utf8mb4_bin");
            }
            (Dialect::Mysql, kind, _) => {
                self.ident(&f.column).text(" = ");
                self.cast(kind, v);
            }
            (Dialect::Sqlite, _, _) => self.dynamic(&f.column, v),
        }
    }

    fn cast(&mut self, kind: Kind, v: &str) {
        let wrap = |q: &mut Self, open: &str, value: &str, close: &str| {
            q.text(open).value(Some(value)).text(close);
        };
        match (kind, hex_digits(v)) {
            (Kind::Decimal, _) => wrap(self, "CAST(", v, " AS DECIMAL(65,30))"),
            (Kind::Float, _) => wrap(self, "CAST(", v, " AS FLOAT)"),
            (Kind::Double, _) => wrap(self, "CAST(", v, " AS DOUBLE)"),
            (Kind::Binary, Some(h)) => wrap(self, "UNHEX(", h, ")"),
            (Kind::Bit, Some(h)) => wrap(self, "CAST(CONV(", h, ", 16, 10) AS UNSIGNED)"),
            _ => {
                self.value(Some(v));
            }
        }
    }

    fn dynamic(&mut self, column: &str, v: &str) {
        let blob = hex_digits(v).map_or_else(|| hex_of(v.as_bytes()), str::to_ascii_uppercase);
        self.text("CASE typeof(")
            .ident(column)
            .text(") WHEN 'integer' THEN ");
        self.ident(column)
            .text(" = CAST(")
            .value(Some(v))
            .text(" AS INTEGER)");
        self.text(" WHEN 'real' THEN ")
            .ident(column)
            .text(" = CAST(");
        self.value(Some(v)).text(" AS REAL) WHEN 'blob' THEN hex(");
        self.ident(column).text(") = ").value(Some(&blob));
        self.text(" ELSE ")
            .ident(column)
            .text(" = ")
            .value(Some(v))
            .text(" END");
    }

    fn store(&mut self, kind: Kind, v: Option<&str>) {
        match (self.dialect, kind, v.and_then(hex_digits)) {
            (Dialect::Postgres, Kind::Binary, Some(h)) => {
                self.value(Some(&format!("\\x{h}")));
            }
            (Dialect::Mysql, Kind::Binary | Kind::Bit, Some(_)) => {
                self.cast(kind, v.unwrap_or_default());
            }
            (Dialect::Sqlite | Dialect::Duckdb, Kind::Binary, Some(h)) => {
                self.text("unhex(").value(Some(h)).text(")");
            }
            (Dialect::Hana, Kind::Binary, Some(h)) => {
                self.text("HEXTOBIN(").value(Some(h)).text(")");
            }
            (Dialect::Oracle | Dialect::Db2, Kind::Binary, Some(h)) => {
                self.text("HEXTORAW(").value(Some(h)).text(")");
            }
            (Dialect::Mssql, Kind::Binary, Some(h)) => {
                self.text("CONVERT(varbinary(max), ")
                    .value(Some(&format!("0x{h}")))
                    .text(", 1)");
            }
            _ => {
                self.value(v);
            }
        }
    }

    fn row_matches(&mut self, types: &[ColumnType], key: &[Field], original: &[Field]) {
        let dialect = self.dialect;
        let kind = |c: &str| kind_of(dialect, types, c);
        let exact_too = |k: Kind| match dialect {
            Dialect::Mysql => matches!(k, Kind::Text | Kind::Int),
            Dialect::Mssql => k == Kind::Text,
            _ => false,
        };
        let mut checks = Vec::new();
        for f in key {
            let k = kind(&f.column);
            checks.push((f, k, Role::Key));
            if exact_too(k) && !original.iter().any(|o| o.column == f.column) {
                checks.push((f, k, Role::Original));
            }
        }
        for f in original {
            let k = kind(&f.column);
            let keyed = key.iter().any(|x| x.column == f.column);
            let opaque = k == Kind::Opaque;
            if (!keyed || exact_too(k)) && !opaque {
                checks.push((f, k, Role::Original));
            }
        }
        for (i, (f, k, role)) in checks.into_iter().enumerate() {
            if i > 0 {
                self.text(" AND ");
            }
            self.matches(f, k, role);
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Role {
    Key,
    Original,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    Text,
    Int,
    Decimal,
    Float,
    Double,
    Json,
    Binary,
    Bit,
    Opaque,
    Typed,
    Other,
}

fn kind(dialect: Dialect, type_name: &str) -> Kind {
    let t = type_name.trim().to_ascii_lowercase();
    let base = t
        .split(|c: char| c == '(' || c.is_whitespace())
        .next()
        .unwrap_or_default();
    match dialect {
        Dialect::Postgres if base == "bytea" => Kind::Binary,
        Dialect::Duckdb if matches!(base, "blob" | "bytea" | "binary" | "varbinary") => {
            Kind::Binary
        }
        Dialect::Postgres
        | Dialect::Duckdb
        | Dialect::Clickhouse
        | Dialect::Cql
        | Dialect::Partiql
        | Dialect::Opensearch
        | Dialect::Elasticsearch => Kind::Other,
        Dialect::Db2 => match base {
            "binary" | "varbinary" => Kind::Binary,
            "blob" | "clob" | "dbclob" | "xml" => Kind::Opaque,
            _ => Kind::Other,
        },
        Dialect::Ansi | Dialect::AnsiBacktick => match base {
            "blob" | "clob" | "binary" | "varbinary" | "longvarbinary" => Kind::Opaque,
            _ => Kind::Other,
        },
        Dialect::Firebird => match base {
            "blob" => Kind::Opaque,
            _ => Kind::Other,
        },
        Dialect::Oracle => match base {
            "raw" => Kind::Binary,
            "blob" | "clob" | "nclob" | "long" | "bfile" | "xmltype" | "json" | "sdo_geometry"
            | "binary_float" | "binary_double" | "vector" | "anydata" | "urowid" => Kind::Opaque,
            _ => Kind::Other,
        },
        Dialect::Hana => match base {
            "binary" | "varbinary" => Kind::Binary,
            "blob" | "clob" | "nclob" | "text" | "bintext" | "st_geometry" | "st_point" => {
                Kind::Opaque
            }
            _ => Kind::Other,
        },
        Dialect::Mssql => match base {
            "binary" | "varbinary" | "image" | "timestamp" | "rowversion" => Kind::Binary,
            "char" | "varchar" | "nchar" | "nvarchar" | "sysname" => Kind::Text,
            "text" | "ntext" | "xml" | "geography" | "geometry" | "hierarchyid" | "sql_variant" => {
                Kind::Opaque
            }
            _ => Kind::Other,
        },
        Dialect::Mysql => match base {
            "tinyint" | "smallint" | "mediumint" | "int" | "integer" | "bigint" => Kind::Int,
            "decimal" | "numeric" | "dec" | "fixed" => Kind::Decimal,
            "float" => Kind::Float,
            "double" | "real" => Kind::Double,
            "json" => Kind::Json,
            "binary" | "varbinary" | "tinyblob" | "blob" | "mediumblob" | "longblob" => {
                Kind::Binary
            }
            "bit" => Kind::Bit,
            "char" | "varchar" | "tinytext" | "text" | "mediumtext" | "longtext" | "enum"
            | "set" => Kind::Text,
            "geometry" | "point" | "linestring" | "polygon" | "multipoint" | "multilinestring"
            | "multipolygon" | "geometrycollection" | "geomcollection" => Kind::Opaque,
            _ => Kind::Other,
        },
        Dialect::Sqlite if t.contains("blob") => Kind::Binary,
        Dialect::Sqlite if t.is_empty() => Kind::Other,
        Dialect::Sqlite => Kind::Typed,
    }
}

fn kind_of(dialect: Dialect, types: &[ColumnType], column: &str) -> Kind {
    types
        .iter()
        .find(|t| t.name == column)
        .map_or(Kind::Other, |t| kind(dialect, &t.type_name))
}

fn hex_digits(v: &str) -> Option<&str> {
    let h = v
        .strip_prefix("\\x")
        .or_else(|| v.strip_prefix("0x"))
        .or_else(|| v.strip_prefix("0X"))?;
    (h.len() % 2 == 0 && h.bytes().all(|b| b.is_ascii_hexdigit())).then_some(h)
}

fn hex_of(bytes: &[u8]) -> String {
    use std::fmt::Write;
    bytes
        .iter()
        .fold(String::with_capacity(bytes.len() * 2), |mut s, b| {
            let _ = write!(s, "{b:02X}");
            s
        })
}

pub fn is_binary(dialect: Dialect, type_name: &str) -> bool {
    matches!(kind(dialect, type_name), Kind::Binary)
}

pub fn invalid_value(dialect: Dialect, set: &ChangeSet) -> Option<String> {
    set.changes
        .iter()
        .flat_map(|c| match c {
            Change::Update { set, .. } => set.as_slice(),
            Change::Insert { values } => values.as_slice(),
            Change::Delete { .. } => &[],
        })
        .find(|f| {
            let binary = matches!(
                kind_of(dialect, &set.columns, &f.column),
                Kind::Binary | Kind::Bit
            );
            binary && f.value.as_deref().is_some_and(|v| hex_digits(v).is_none())
        })
        .map(|f| f.column.clone())
}

pub fn change(dialect: Dialect, table: &TableRef, types: &[ColumnType], change: &Change) -> Bound {
    let mut q = Builder::new(dialect);
    let kind = |c: &str| kind_of(dialect, types, c);
    match change {
        Change::Update { key, original, set } => {
            q.text("UPDATE ").table(table).text(" SET ");
            q.list(set, ", ", |q, f| {
                q.ident(&f.column).text(" = ");
                q.store(kind(&f.column), f.value.as_deref());
            });
            q.text(" WHERE ");
            q.row_matches(types, key, original);
        }
        Change::Insert { values } if values.is_empty() && dialect == Dialect::Oracle => {
            q.text("INSERT INTO ").table(table);
            match types.first() {
                Some(c) => {
                    q.text(" (").ident(&c.name).text(") VALUES (DEFAULT)");
                }
                None => {
                    q.text(" VALUES (DEFAULT)");
                }
            }
        }
        Change::Insert { values } if values.is_empty() => {
            q.text("INSERT INTO ").table(table);
            q.text(if dialect == Dialect::Mysql {
                " () VALUES ()"
            } else {
                " DEFAULT VALUES"
            });
        }
        Change::Insert { values } => {
            q.text("INSERT INTO ").table(table).text(" (");
            q.list(values, ", ", |q, f| {
                q.ident(&f.column);
            });
            q.text(") VALUES (");
            q.list(values, ", ", |q, f| {
                q.store(kind(&f.column), f.value.as_deref());
            });
            q.text(")");
        }
        Change::Delete { key, original } => {
            q.text("DELETE FROM ").table(table).text(" WHERE ");
            q.row_matches(types, key, original);
        }
    }
    q.b
}

pub fn count_change(
    dialect: Dialect,
    table: &TableRef,
    types: &[ColumnType],
    change: &Change,
) -> Option<Bound> {
    let (Change::Update { key, original, .. } | Change::Delete { key, original }) = change else {
        return None;
    };
    let mut q = Builder::new(dialect);
    q.text(count_head(dialect));
    q.table(table).text(" WHERE ");
    q.row_matches(types, key, original);
    Some(q.b)
}

pub fn count_key(dialect: Dialect, table: &TableRef, types: &[ColumnType], key: &[Field]) -> Bound {
    let mut q = Builder::new(dialect);
    q.text(count_head(dialect));
    q.table(table);
    if !key.is_empty() {
        q.text(" WHERE ");
        q.row_matches(types, key, &[]);
    }
    q.b
}

pub fn select_keys(
    dialect: Dialect,
    table: &TableRef,
    types: &[ColumnType],
    keys: &[Vec<Field>],
) -> Bound {
    let mut q = Builder::new(dialect);
    q.text("SELECT * FROM ");
    q.table(table);
    if !keys.is_empty() {
        q.text(" WHERE ");
        q.list(keys, " OR ", |q, key| {
            q.text("(");
            q.row_matches(types, key, &[]);
            q.text(")");
        });
    }
    q.b
}

pub(crate) type Seek<'a> = (&'a [String], bool, &'a [Option<String>]);

pub(crate) fn filters(q: &mut Builder, browse: &Browse, seek: Option<Seek<'_>>) {
    let mut first = true;
    let mut and = |q: &mut Builder| {
        q.text(if first { " WHERE " } else { " AND " });
        first = false;
    };
    if let Some((columns, descending, values)) = seek
        && matches!(
            q.dialect,
            Dialect::Mssql | Dialect::Oracle | Dialect::Db2 | Dialect::Ansi | Dialect::AnsiBacktick
        )
        && columns.len() > 1
    {
        and(q);
        seek_expanded(q, columns, descending, values);
    } else if let Some((columns, descending, values)) = seek {
        and(q);
        let one = columns.len() == 1;
        q.text(if one { "" } else { "(" });
        q.list(columns, ", ", |q, c| {
            q.ident(c);
        });
        q.text(if one { "" } else { ")" });
        q.text(if descending { " < " } else { " > " });
        q.text(if one { "" } else { "(" });
        q.list(values, ", ", |q, v| {
            q.value(v.as_deref());
        });
        q.text(if one { "" } else { ")" });
    }
    if let Some(f) = browse.filter.as_deref().filter(|f| !f.trim().is_empty()) {
        and(q);
        q.text("(\n").text(f).text("\n)");
    }
    for f in &browse.quick {
        and(q);
        let v = f.value.as_deref().unwrap_or_default();
        match f.op {
            FilterOp::IsNull => {
                q.ident(&f.column).text(" IS NULL");
            }
            FilterOp::NotNull => {
                q.ident(&f.column).text(" IS NOT NULL");
            }
            FilterOp::Contains => contains(q, &f.column, v),
            op => {
                let sym = match op {
                    FilterOp::Ne => " <> ",
                    FilterOp::Lt => " < ",
                    FilterOp::Le => " <= ",
                    FilterOp::Gt => " > ",
                    FilterOp::Ge => " >= ",
                    _ => " = ",
                };
                q.ident(&f.column).text(sym).value(Some(v));
            }
        }
    }
    if let Some(text) = browse.search.as_deref().filter(|t| !t.trim().is_empty())
        && !browse.columns.is_empty()
    {
        and(q);
        q.text("(");
        q.list(&browse.columns, " OR ", |q, c| contains(q, c, text.trim()));
        q.text(")");
    }
}

fn contains(q: &mut Builder, column: &str, v: &str) {
    match q.dialect {
        Dialect::Postgres => {
            q.text("strpos(lower(")
                .ident(column)
                .text("::text), lower(");
            q.value(Some(v)).text(")) > 0");
        }
        Dialect::Mysql | Dialect::Opensearch | Dialect::Elasticsearch => {
            q.text("LOCATE(").value(Some(v)).text(", ").ident(column);
            q.text(") > 0");
        }
        Dialect::Sqlite => {
            q.text("instr(lower(").ident(column).text("), lower(");
            q.value(Some(v)).text(")) > 0");
        }
        Dialect::Duckdb => {
            q.text("instr(lower(").ident(column).text("::text), lower(");
            q.value(Some(v)).text(")) > 0");
        }
        Dialect::Clickhouse => {
            q.text("positionCaseInsensitiveUTF8(toString(")
                .ident(column);
            q.text("), ").value(Some(v)).text(") > 0");
        }
        Dialect::Firebird => {
            q.ident(column).text(" CONTAINING ").value(Some(v));
        }
        Dialect::Oracle => {
            q.text("INSTR(LOWER(").ident(column).text("), LOWER(");
            q.value(Some(v)).text(")) > 0");
        }
        Dialect::Cql => {
            q.ident(column)
                .text(" LIKE ")
                .value(Some(&format!("%{v}%")));
        }
        Dialect::Partiql => {
            q.text("contains(").ident(column).text(", ").value(Some(v));
            q.text(")");
        }
        Dialect::Db2 => {
            q.text("LOCATE(LOWER(")
                .value(Some(v))
                .text("), LOWER(VARCHAR(");
            q.ident(column).text("))) > 0");
        }
        Dialect::Ansi | Dialect::AnsiBacktick => {
            q.text("LOWER(CAST(")
                .ident(column)
                .text(" AS VARCHAR(4000))) LIKE ");
            q.value(Some(&format!("%{}%", v.to_lowercase())));
        }
        Dialect::Hana => {
            q.text("LOCATE(LOWER(TO_NVARCHAR(")
                .ident(column)
                .text(")), LOWER(");
            q.value(Some(v)).text(")) > 0");
        }
        Dialect::Mssql => {
            q.text("CHARINDEX(lower(")
                .value(Some(v))
                .text("), lower(CAST(");
            q.ident(column).text(" AS nvarchar(max)))) > 0");
        }
    }
}

fn seek_expanded(q: &mut Builder, columns: &[String], descending: bool, values: &[Option<String>]) {
    let op = if descending { " < " } else { " > " };
    q.text("(");
    for (i, (column, value)) in columns.iter().zip(values).enumerate() {
        if i > 0 {
            q.text(" OR ");
        }
        q.text("(");
        for (c, v) in columns.iter().zip(values).take(i) {
            q.ident(c).text(" = ").value(v.as_deref()).text(" AND ");
        }
        q.ident(column).text(op).value(value.as_deref()).text(")");
    }
    q.text(")");
}

pub fn select(dialect: Dialect, browse: &Browse) -> Bound {
    let mut q = Builder::new(dialect);
    q.text("SELECT ");
    q.list(&browse.columns, ", ", |q, c| {
        q.ident(c);
        if dialect == Dialect::Postgres {
            q.text("::text AS ").ident(c);
        }
    });
    q.text(" FROM ").table(&browse.table);
    let keys = keyset(browse);
    let seek = keys
        .as_ref()
        .zip(browse.after.as_deref())
        .and_then(|((c, d), v)| {
            (c.len() == v.len() && v.iter().all(Option::is_some)).then_some((c.as_slice(), *d, v))
        });
    filters(&mut q, browse, seek);
    let tie = browse
        .key
        .iter()
        .filter(|k| !browse.order.iter().any(|o| &o.column == *k))
        .map(|k| Order {
            column: k.clone(),
            descending: false,
        });
    let order: Vec<Order> = browse.order.iter().cloned().chain(tie).collect();
    if !order.is_empty() {
        q.text(" ORDER BY ");
        q.list(&order, ", ", |q, o| {
            q.ident(&browse.table.name).text(".").ident(&o.column);
            if o.descending {
                q.text(" DESC");
            }
        });
    }
    let offset = if seek.is_some() { 0 } else { browse.offset };
    if matches!(
        dialect,
        Dialect::Mssql
            | Dialect::Firebird
            | Dialect::Oracle
            | Dialect::Db2
            | Dialect::Ansi
            | Dialect::AnsiBacktick
    ) {
        if order.is_empty() && dialect == Dialect::Mssql {
            q.text(" ORDER BY (SELECT NULL)");
        }
        q.text(&format!(
            " OFFSET {offset} ROWS FETCH NEXT {} ROWS ONLY",
            browse.limit
        ));
    } else if dialect == Dialect::Cql {
        q.text(&format!(" LIMIT {}", browse.limit));
    } else if dialect != Dialect::Partiql {
        q.text(&format!(" LIMIT {} OFFSET {offset}", browse.limit));
    }
    q.b
}

fn count_head(dialect: Dialect) -> &'static str {
    match dialect {
        Dialect::Postgres => "SELECT count(*)::text FROM ",
        Dialect::Mssql => "SELECT COUNT_BIG(*) FROM ",
        Dialect::Sqlite | Dialect::Mysql | Dialect::Duckdb => "SELECT count(*) FROM ",
        Dialect::Firebird
        | Dialect::Hana
        | Dialect::Oracle
        | Dialect::Db2
        | Dialect::Ansi
        | Dialect::AnsiBacktick
        | Dialect::Cql
        | Dialect::Partiql
        | Dialect::Opensearch
        | Dialect::Elasticsearch => "SELECT COUNT(*) FROM ",
        Dialect::Clickhouse => "SELECT toString(count()) FROM ",
    }
}

pub fn count(dialect: Dialect, browse: &Browse) -> Bound {
    let mut q = Builder::new(dialect);
    q.text(count_head(dialect));
    q.table(&browse.table);
    filters(&mut q, browse, None);
    q.b
}

pub fn catalog_count(dialect: Dialect, browse: &Browse) -> Option<Bound> {
    let blank = |t: &Option<String>| t.as_deref().is_none_or(|t| t.trim().is_empty());
    if dialect != Dialect::Mssql
        || !blank(&browse.filter)
        || !blank(&browse.search)
        || !browse.quick.is_empty()
    {
        return None;
    }
    let mut name = Builder::new(dialect);
    name.table(&browse.table);
    let mut q = Builder::new(dialect);
    q.text("SELECT CAST(SUM(p.rows) AS bigint) FROM sys.partitions p WHERE p.index_id IN (0, 1) AND p.object_id = OBJECT_ID(")
        .value(Some(&name.b.sql))
        .text(")");
    Some(q.b)
}

pub fn begin(dialect: Dialect) -> &'static str {
    match dialect {
        Dialect::Mysql => "START TRANSACTION",
        Dialect::Mssql
        | Dialect::Clickhouse
        | Dialect::Db2
        | Dialect::Ansi
        | Dialect::AnsiBacktick => "BEGIN TRANSACTION",
        Dialect::Postgres
        | Dialect::Sqlite
        | Dialect::Duckdb
        | Dialect::Partiql
        | Dialect::Opensearch
        | Dialect::Elasticsearch => "BEGIN",
        Dialect::Cql => "BEGIN BATCH",
        Dialect::Firebird => "SET TRANSACTION",
        Dialect::Hana | Dialect::Oracle => "SET TRANSACTION READ WRITE",
    }
}

pub fn read_only(dialect: Dialect, on: bool) -> Option<&'static str> {
    Some(match (dialect, on) {
        (Dialect::Postgres, true) => "SET SESSION default_transaction_read_only = on",
        (Dialect::Postgres, false) => "SET SESSION default_transaction_read_only = off",
        (Dialect::Mysql, true) => "SET SESSION TRANSACTION READ ONLY",
        (Dialect::Mysql, false) => "SET SESSION TRANSACTION READ WRITE",
        (Dialect::Sqlite, true) => "PRAGMA query_only = ON",
        (Dialect::Sqlite, false) => "PRAGMA query_only = OFF",
        (
            Dialect::Mssql
            | Dialect::Duckdb
            | Dialect::Clickhouse
            | Dialect::Firebird
            | Dialect::Hana
            | Dialect::Oracle
            | Dialect::Db2
            | Dialect::Ansi
            | Dialect::AnsiBacktick
            | Dialect::Cql
            | Dialect::Partiql
            | Dialect::Opensearch
            | Dialect::Elasticsearch,
            _,
        ) => return None,
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "snake_case")]
pub enum Generator {
    Select,
    Insert,
    Update,
    Delete,
    Upsert,
    Copy,
    Drop,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
pub struct GenColumn {
    pub name: String,
    pub primary_key: bool,
    #[serde(default)]
    pub type_name: String,
    #[serde(default)]
    pub not_null: bool,
    #[serde(default)]
    pub identity: Option<Identity>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "snake_case")]
pub enum Identity {
    Always,
    ByDefault,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
pub struct GenRequest {
    pub table: TableRef,
    pub view: bool,
    pub columns: Vec<GenColumn>,
    pub generator: Generator,
    pub copy_name: String,
}

pub fn generators(view: bool, has_key: bool) -> Vec<Generator> {
    use Generator::{Copy, Delete, Drop, Insert, Select, Update, Upsert};
    match (view, has_key) {
        (true, _) => vec![Select, Copy, Drop],
        (false, false) => vec![Select, Insert, Copy, Drop],
        (false, true) => vec![Select, Insert, Update, Delete, Upsert, Copy, Drop],
    }
}

const WIDTH: usize = 64;

struct Gen<'a> {
    dialect: Dialect,
    table: String,
    columns: Vec<(String, String, bool)>,
    req: &'a GenRequest,
}

impl Gen<'_> {
    fn identity(&self, i: usize) -> Option<Identity> {
        self.req.columns.get(i).and_then(|c| c.identity)
    }

    fn identity_insert(&self, sql: String) -> String {
        let explicit = (0..self.columns.len()).any(|i| self.identity(i) == Some(Identity::Always));
        if self.dialect == Dialect::Mssql && explicit {
            let t = &self.table;
            format!("SET IDENTITY_INSERT {t} ON;\n{sql}\nSET IDENTITY_INSERT {t} OFF;")
        } else {
            sql
        }
    }

    fn overriding(&self) -> &'static str {
        let always = (0..self.columns.len()).any(|i| self.identity(i) == Some(Identity::Always));
        if self.dialect == Dialect::Postgres && always {
            "OVERRIDING SYSTEM VALUE\n"
        } else {
            ""
        }
    }
}

impl Gen<'_> {
    fn name(&self, n: &str) -> String {
        quote::name(self.dialect, n)
    }

    fn qualified(&self, t: &TableRef) -> String {
        if t.namespace.is_empty() || self.dialect == Dialect::Firebird {
            self.name(&t.name)
        } else {
            format!("{}.{}", self.name(&t.namespace), self.name(&t.name))
        }
    }

    fn keys(&self) -> impl Iterator<Item = &(String, String, bool)> {
        self.columns.iter().filter(|c| c.2)
    }

    fn rest(&self) -> impl Iterator<Item = &(String, String, bool)> {
        self.columns.iter().filter(|c| !c.2)
    }

    fn names(&self) -> Vec<String> {
        self.columns.iter().map(|c| c.0.clone()).collect()
    }

    fn params(&self) -> Vec<String> {
        self.columns.iter().map(|c| format!(":{}", c.1)).collect()
    }

    fn key_match(&self, left: &str, right: &dyn Fn(&(String, String, bool)) -> String) -> String {
        self.keys()
            .map(|c| format!("{left}{} = {}", c.0, right(c)))
            .collect::<Vec<_>>()
            .join("\n  AND ")
    }
}

fn group(items: &[String], open: &str, close: &str) -> String {
    let flat = items.join(", ");
    if flat.len() + open.len() <= WIDTH {
        format!("{open}({flat}){close}")
    } else {
        format!("{open}(\n  {}\n){close}", items.join(",\n  "))
    }
}

fn assignments(items: &[String]) -> String {
    let flat = items.join(", ");
    if flat.len() <= WIDTH && items.len() <= 2 {
        format!(" {flat}")
    } else {
        format!("\n  {}", items.join(",\n  "))
    }
}

fn param_names(columns: &[GenColumn]) -> Vec<String> {
    let mut out: Vec<String> = Vec::with_capacity(columns.len());
    for c in columns {
        let mut p: String = c
            .name
            .chars()
            .map(|ch| {
                if ch.is_ascii_alphanumeric() {
                    ch.to_ascii_lowercase()
                } else {
                    '_'
                }
            })
            .collect();
        if p.is_empty() || p.starts_with(|ch: char| ch.is_ascii_digit()) {
            p.insert(0, 'c');
        }
        let base = p.clone();
        let mut n = 2;
        while out.contains(&p) {
            p = format!("{base}_{n}");
            n += 1;
        }
        out.push(p);
    }
    out
}

pub fn generate(dialect: Dialect, req: &GenRequest) -> Option<String> {
    let known = matches!(
        req.generator,
        Generator::Select | Generator::Copy | Generator::Drop
    ) || !req.columns.is_empty();
    let uncopyable = dialect == Dialect::Firebird && req.generator == Generator::Copy;
    if !known
        || uncopyable
        || !generators(req.view, req.columns.iter().any(|c| c.primary_key)).contains(&req.generator)
    {
        return None;
    }
    let params = param_names(&req.columns);
    let columns = req
        .columns
        .iter()
        .zip(params)
        .map(|(c, p)| (quote::name(dialect, &c.name), p, c.primary_key))
        .collect();
    let mut g = Gen {
        dialect,
        table: String::new(),
        columns,
        req,
    };
    g.table = g.qualified(&req.table);
    let t = g.table.as_str();
    let sql = match req.generator {
        Generator::Select => select_all(&g),
        Generator::Insert => insert(&g),
        Generator::Update => update(&g),
        Generator::Delete => format!(
            "DELETE FROM {t}\nWHERE {};",
            g.key_match("", &|c| format!(":{}", c.1))
        ),
        Generator::Upsert => upsert(&g),
        Generator::Copy => copy(&g),
        Generator::Drop => format!("DROP {} {t};", if req.view { "VIEW" } else { "TABLE" }),
    };
    if dialect == Dialect::Oracle && split(&sql, dialect).len() == 1 {
        return Some(sql.trim_end_matches(';').to_owned());
    }
    Some(sql)
}

fn insert(g: &Gen<'_>) -> String {
    let t = g.table.as_str();
    let given: Vec<usize> = (0..g.columns.len())
        .filter(|i| g.identity(*i).is_none())
        .collect();
    if given.is_empty() {
        return if g.dialect == Dialect::Mysql {
            format!("INSERT INTO {t} () VALUES ();")
        } else {
            format!("INSERT INTO {t} DEFAULT VALUES;")
        };
    }
    let names: Vec<String> = given.iter().map(|i| g.columns[*i].0.clone()).collect();
    let params: Vec<String> = given
        .iter()
        .map(|i| format!(":{}", g.columns[*i].1))
        .collect();
    format!(
        "INSERT INTO {}\n{};",
        group(&names, &format!("{t} "), ""),
        group(&params, "VALUES ", "")
    )
}

fn select_all(g: &Gen<'_>) -> String {
    let names = g.names();
    let list = if names.is_empty() {
        " *".to_owned()
    } else {
        assignments(&names)
    };
    format!("SELECT{list}\nFROM {};", g.table)
}

fn update(g: &Gen<'_>) -> String {
    let rest: Vec<_> = g.rest().collect();
    let (set, suffix) = if rest.is_empty() {
        (g.keys().collect::<Vec<_>>(), "_new")
    } else {
        (rest, "")
    };
    let set: Vec<String> = set
        .iter()
        .map(|c| format!("{} = :{}{suffix}", c.0, c.1))
        .collect();
    let head = if g.dialect == Dialect::Clickhouse {
        format!("ALTER TABLE {}\nUPDATE", g.table)
    } else {
        format!("UPDATE {}\nSET", g.table)
    };
    format!(
        "{head}{}\nWHERE {};",
        assignments(&set),
        g.key_match("", &|c| format!(":{}", c.1))
    )
}

fn merge_values(g: &Gen<'_>, rest: &[&(String, String, bool)]) -> String {
    let t = g.table.as_str();
    let on = g.key_match("t.", &|c| format!("s.{}", c.0));
    let matched = if rest.is_empty() {
        String::new()
    } else {
        let set: Vec<String> = rest.iter().map(|c| format!("t.{0} = s.{0}", c.0)).collect();
        format!("\nWHEN MATCHED THEN UPDATE SET{}", assignments(&set))
    };
    let values: Vec<String> = g.columns.iter().map(|c| format!("s.{}", c.0)).collect();
    format!(
        "MERGE INTO {t} t\nUSING (VALUES {}) AS s {}\nON ({on}){matched}\nWHEN NOT MATCHED THEN INSERT {}\n  {};",
        group(&g.params(), "", ""),
        group(&g.names(), "", ""),
        group(&g.names(), "", ""),
        group(&values, "VALUES ", "")
    )
}

fn upsert(g: &Gen<'_>) -> String {
    g.identity_insert(upsert_plain(g))
}

fn upsert_plain(g: &Gen<'_>) -> String {
    let t = g.table.as_str();
    let insert = format!(
        "INSERT INTO {}\n{}{}",
        group(&g.names(), &format!("{t} "), ""),
        g.overriding(),
        group(&g.params(), "VALUES ", "")
    );
    let keys: Vec<String> = g.keys().map(|c| c.0.clone()).collect();
    let rest: Vec<&(String, String, bool)> = g.rest().collect();
    match g.dialect {
        Dialect::Postgres | Dialect::Sqlite | Dialect::Duckdb => {
            let conflict = format!("ON CONFLICT ({})", keys.join(", "));
            if rest.is_empty() {
                format!("{insert}\n{conflict} DO NOTHING;")
            } else {
                let set: Vec<String> = rest
                    .iter()
                    .map(|c| format!("{0} = EXCLUDED.{0}", c.0))
                    .collect();
                format!("{insert}\n{conflict} DO UPDATE SET{};", assignments(&set))
            }
        }
        Dialect::Mysql => {
            let set: Vec<String> = if rest.is_empty() {
                keys.iter().map(|k| format!("{k} = {k}")).collect()
            } else {
                rest.iter()
                    .map(|c| format!("{0} = VALUES({0})", c.0))
                    .collect()
            };
            format!("{insert}\nON DUPLICATE KEY UPDATE{};", assignments(&set))
        }
        Dialect::Clickhouse
        | Dialect::Cql
        | Dialect::Partiql
        | Dialect::Opensearch
        | Dialect::Elasticsearch => format!("{insert};"),
        Dialect::Firebird => format!("UPDATE OR {insert}\nMATCHING {};", group(&keys, "", "")),
        Dialect::Oracle => {
            let pairs: Vec<String> = g
                .columns
                .iter()
                .map(|c| format!(":{} AS {}", c.1, c.0))
                .collect();
            let on = g.key_match("t.", &|c| format!("s.{}", c.0));
            let matched = if rest.is_empty() {
                String::new()
            } else {
                let set: Vec<String> = rest.iter().map(|c| format!("t.{0} = s.{0}", c.0)).collect();
                format!("\nWHEN MATCHED THEN UPDATE SET{}", assignments(&set))
            };
            let values: Vec<String> = g.columns.iter().map(|c| format!("s.{}", c.0)).collect();
            format!(
                "MERGE INTO {t} t\nUSING (SELECT{} FROM dual) s\nON ({on}){matched}\nWHEN NOT MATCHED THEN INSERT {}\n  {};",
                assignments(&pairs),
                group(&g.names(), "", ""),
                group(&values, "VALUES ", "")
            )
        }
        Dialect::Db2 | Dialect::Ansi | Dialect::AnsiBacktick => merge_values(g, &rest),
        Dialect::Hana => format!(
            "UPSERT {t} {}\n{}\nWITH PRIMARY KEY;",
            group(&g.names(), "", ""),
            group(&g.params(), "VALUES ", "")
        ),
        Dialect::Mssql => {
            let source = format!(
                "{} AS s {}",
                group(&g.params(), "(VALUES ", ")"),
                group(&g.names(), "", "")
            );
            let on = g.key_match("t.", &|c| format!("s.{}", c.0));
            let matched = if rest.is_empty() {
                String::new()
            } else {
                let set: Vec<String> = rest.iter().map(|c| format!("{0} = s.{0}", c.0)).collect();
                format!("\nWHEN MATCHED THEN UPDATE SET{}", assignments(&set))
            };
            let values: Vec<String> = g.columns.iter().map(|c| format!("s.{}", c.0)).collect();
            format!(
                "MERGE INTO {t} AS t\nUSING {source}\nON {on}{matched}\nWHEN NOT MATCHED THEN INSERT {}\n  {};",
                group(&g.names(), "", ""),
                group(&values, "VALUES ", "")
            )
        }
    }
}

fn copy(g: &Gen<'_>) -> String {
    let target = g.qualified(&TableRef {
        namespace: g.req.table.namespace.clone(),
        name: g.req.copy_name.clone(),
    });
    let t = g.table.as_str();
    let keys: Vec<String> = g.keys().map(|c| c.0.clone()).collect();
    let typed = !g.req.view && g.req.columns.iter().all(|c| !c.type_name.is_empty());
    match (g.dialect, g.req.view) {
        (Dialect::Mssql, false) if !keys.is_empty() => {
            let pk = g.name(&format!("pk_{}", g.req.copy_name));
            format!(
                "SELECT *\nINTO {target}\nFROM {t};\nALTER TABLE {target} ADD CONSTRAINT {pk} PRIMARY KEY {};",
                group(&keys, "", "")
            )
        }
        (Dialect::Mssql, _) => format!("SELECT *\nINTO {target}\nFROM {t};"),
        (Dialect::Sqlite | Dialect::Duckdb, false) if typed => {
            let mut lines: Vec<String> = g
                .columns
                .iter()
                .zip(&g.req.columns)
                .map(|(c, col)| {
                    let null = if col.not_null || col.primary_key {
                        " NOT NULL"
                    } else {
                        ""
                    };
                    format!("{} {}{null}", c.0, col.type_name)
                })
                .collect();
            if !keys.is_empty() {
                lines.push(format!("PRIMARY KEY ({})", keys.join(", ")));
            }
            format!(
                "CREATE TABLE {target} (\n  {}\n);\nINSERT INTO {target}\nSELECT * FROM {t};",
                lines.join(",\n  ")
            )
        }
        (Dialect::Postgres, false) => format!(
            "CREATE TABLE {target} (LIKE {t} INCLUDING ALL);\nINSERT INTO {target}\nSELECT * FROM {t};"
        ),
        (Dialect::Mysql, false) => {
            format!("CREATE TABLE {target} LIKE {t};\nINSERT INTO {target}\nSELECT * FROM {t};")
        }
        (Dialect::Clickhouse, false) => {
            format!("CREATE TABLE {target} AS {t};\nINSERT INTO {target}\nSELECT * FROM {t};")
        }
        _ => format!("CREATE TABLE {target} AS\nSELECT * FROM {t};"),
    }
}
