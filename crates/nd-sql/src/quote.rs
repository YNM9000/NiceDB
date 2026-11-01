use crate::Dialect;

pub fn ident(dialect: Dialect, name: &str) -> String {
    if dialect == Dialect::Oracle && name == "ROWID" {
        return name.to_owned();
    }
    let (open, close) = match dialect {
        Dialect::Mysql | Dialect::Clickhouse | Dialect::Opensearch | Dialect::AnsiBacktick => {
            ('`', '`')
        }
        Dialect::Mssql => ('[', ']'),
        Dialect::Postgres
        | Dialect::Sqlite
        | Dialect::Duckdb
        | Dialect::Firebird
        | Dialect::Hana
        | Dialect::Db2
        | Dialect::Ansi
        | Dialect::Oracle
        | Dialect::Cql
        | Dialect::Partiql
        | Dialect::Elasticsearch => ('"', '"'),
    };
    let mut s = String::with_capacity(name.len() + 2);
    s.push(open);
    for c in name.chars() {
        if c == close || (c == '\\' && dialect == Dialect::Clickhouse) {
            s.push(c);
        }
        s.push(c);
    }
    s.push(close);
    s
}

const RESERVED: &[&str] = &[
    "add",
    "all",
    "alter",
    "analyze",
    "and",
    "any",
    "array",
    "as",
    "asc",
    "authorization",
    "backup",
    "between",
    "both",
    "by",
    "cascade",
    "case",
    "cast",
    "check",
    "collate",
    "column",
    "commit",
    "constraint",
    "create",
    "cross",
    "current",
    "current_date",
    "current_time",
    "current_timestamp",
    "current_user",
    "database",
    "default",
    "delete",
    "desc",
    "distinct",
    "do",
    "drop",
    "else",
    "end",
    "except",
    "exec",
    "exists",
    "false",
    "fetch",
    "file",
    "for",
    "foreign",
    "from",
    "full",
    "function",
    "grant",
    "group",
    "having",
    "if",
    "in",
    "index",
    "inner",
    "insert",
    "intersect",
    "interval",
    "into",
    "is",
    "join",
    "key",
    "keys",
    "leading",
    "left",
    "like",
    "limit",
    "lock",
    "natural",
    "not",
    "null",
    "offset",
    "on",
    "only",
    "or",
    "order",
    "outer",
    "over",
    "percent",
    "plan",
    "primary",
    "procedure",
    "range",
    "read",
    "references",
    "rename",
    "replace",
    "right",
    "row",
    "rows",
    "schema",
    "select",
    "session_user",
    "set",
    "some",
    "table",
    "then",
    "to",
    "top",
    "trailing",
    "transaction",
    "trigger",
    "true",
    "union",
    "unique",
    "update",
    "user",
    "using",
    "values",
    "view",
    "when",
    "where",
    "window",
    "with",
];

const ORACLE_RESERVED: &[&str] = &[
    "access",
    "audit",
    "char",
    "cluster",
    "comment",
    "compress",
    "connect",
    "date",
    "decimal",
    "exclusive",
    "float",
    "identified",
    "immediate",
    "increment",
    "initial",
    "integer",
    "level",
    "long",
    "maxextents",
    "minus",
    "mlslabel",
    "mode",
    "modify",
    "noaudit",
    "nocompress",
    "nowait",
    "number",
    "of",
    "offline",
    "online",
    "option",
    "pctfree",
    "prior",
    "privileges",
    "public",
    "raw",
    "resource",
    "rowid",
    "rownum",
    "session",
    "share",
    "size",
    "smallint",
    "start",
    "successful",
    "synonym",
    "sysdate",
    "uid",
    "validate",
    "varchar",
    "varchar2",
    "whenever",
];

const CQL_RESERVED: &[&str] = &[
    "allow",
    "apply",
    "authorize",
    "batch",
    "begin",
    "columnfamily",
    "describe",
    "entries",
    "execute",
    "infinity",
    "keyspace",
    "materialized",
    "mbean",
    "mbeans",
    "modify",
    "nan",
    "norecursive",
    "of",
    "revoke",
    "token",
    "truncate",
    "unlogged",
    "use",
];

pub fn name(dialect: Dialect, name: &str) -> String {
    if dialect == Dialect::Partiql {
        return ident(dialect, name);
    }
    let mut chars = name.chars();
    let lower_only = matches!(dialect, Dialect::Postgres | Dialect::Duckdb | Dialect::Cql);
    let upper_only = matches!(
        dialect,
        Dialect::Firebird | Dialect::Hana | Dialect::Oracle | Dialect::Db2
    );
    let oracle = dialect == Dialect::Oracle;
    let fits = |c: char| {
        c == '_'
            || (!upper_only && c.is_ascii_lowercase())
            || (!lower_only && c.is_ascii_uppercase())
    };
    let lower = name.to_ascii_lowercase();
    let plain = chars
        .next()
        .is_some_and(|c| fits(c) && !(oracle && c == '_'))
        && chars.all(|c| fits(c) || c.is_ascii_digit() || (oracle && matches!(c, '$' | '#')))
        && !RESERVED.contains(&lower.as_str())
        && !(oracle && ORACLE_RESERVED.contains(&lower.as_str()))
        && !(dialect == Dialect::Cql && CQL_RESERVED.contains(&lower.as_str()))
        && (!dialect.is_generic() || name == lower || name == name.to_ascii_uppercase());
    if plain {
        name.to_owned()
    } else {
        ident(dialect, name)
    }
}

pub fn literal(dialect: Dialect, value: Option<&str>) -> String {
    let Some(v) = value else {
        return "NULL".into();
    };
    let escaped = match dialect {
        Dialect::Mysql | Dialect::Clickhouse => true,
        Dialect::Postgres => v.contains('\\'),
        _ => false,
    };
    let mut s = String::with_capacity(v.len() + 3);
    if escaped && dialect == Dialect::Postgres {
        s.push('E');
    }
    s.push('\'');
    for c in v.chars() {
        match c {
            '\'' => s.push_str("''"),
            '\\' if escaped => s.push_str("\\\\"),
            c => s.push(c),
        }
    }
    s.push('\'');
    s
}

pub fn string(dialect: Dialect, value: &str) -> String {
    match dialect {
        Dialect::Mssql => format!("N{}", literal(dialect, Some(value))),
        _ => literal(dialect, Some(value)),
    }
}

pub(crate) fn placeholder(dialect: Dialect, n: usize) -> String {
    match dialect {
        Dialect::Postgres | Dialect::Duckdb => format!("${n}"),
        Dialect::Mssql => format!("@P{n}"),
        Dialect::Oracle => format!(":{n}"),
        Dialect::Sqlite
        | Dialect::Mysql
        | Dialect::Clickhouse
        | Dialect::Firebird
        | Dialect::Hana
        | Dialect::Db2
        | Dialect::Ansi
        | Dialect::AnsiBacktick
        | Dialect::Cql
        | Dialect::Partiql
        | Dialect::Opensearch
        | Dialect::Elasticsearch => "?".into(),
    }
}
