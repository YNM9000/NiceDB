#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Begin,
    Commit,
    Rollback,
    Plain,
}

pub fn kind(sql: &str) -> Kind {
    let words: Vec<String> = sql
        .trim()
        .trim_end_matches(';')
        .split_whitespace()
        .map(str::to_ascii_uppercase)
        .collect();
    let w: Vec<&str> = words.iter().map(String::as_str).collect();
    match w.as_slice() {
        ["BEGIN"] | ["BEGIN", "WORK" | "TRANSACTION"] | ["START", "TRANSACTION"] => Kind::Begin,
        ["COMMIT"] | ["COMMIT", "WORK"] => Kind::Commit,
        ["ROLLBACK"] | ["ROLLBACK", "WORK"] => Kind::Rollback,
        _ => Kind::Plain,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Window {
    pub skip: u64,
    pub take: u64,
}

fn number(s: &str) -> Option<u64> {
    s.parse().ok()
}

pub fn take_window(sql: &str) -> (&str, Option<Window>) {
    let trimmed = sql.trim_end().trim_end_matches(';').trim_end();
    let words: Vec<&str> = trimmed.rsplitn(9, ' ').collect();
    let at = |i: usize| words.get(i).copied().unwrap_or("");
    let upper = |i: usize| at(i).to_ascii_uppercase();
    let tail_is = |list: &[&str]| list.iter().enumerate().all(|(i, w)| upper(i) == *w);
    if tail_is(&["ONLY", "ROWS"])
        && upper(3) == "NEXT"
        && upper(4) == "FETCH"
        && upper(5) == "ROWS"
        && upper(7) == "OFFSET"
        && let (Some(take), Some(skip)) = (number(at(2)), number(at(6)))
    {
        let cut = trimmed.len() - words[..8].iter().map(|w| w.len() + 1).sum::<usize>();
        return (trimmed[..cut].trim_end(), Some(Window { skip, take }));
    }
    if tail_is(&["ONLY", "ROWS"])
        && upper(3) == "FIRST"
        && upper(4) == "FETCH"
        && let Some(take) = number(at(2))
    {
        let cut = trimmed.len() - words[..5].iter().map(|w| w.len() + 1).sum::<usize>();
        return (trimmed[..cut].trim_end(), Some(Window { skip: 0, take }));
    }
    (sql, None)
}

const VERSION_QUERIES: &[(&str, &str)] = &[
    ("postgres", "SHOW server_version"),
    ("mysql", "SELECT VERSION()"),
    ("mariadb", "SELECT VERSION()"),
    (
        "sql server",
        "SELECT CAST(SERVERPROPERTY('ProductVersion') AS VARCHAR(64))",
    ),
    ("sqlite", "SELECT sqlite_version()"),
    (
        "oracle",
        "SELECT version FROM product_component_version WHERE ROWNUM = 1",
    ),
    ("db2", crate::db2::VERSION),
    (
        "firebird",
        "SELECT rdb$get_context('SYSTEM', 'ENGINE_VERSION') FROM rdb$database",
    ),
    (
        "informix",
        "SELECT DBINFO('version', 'full') FROM systables WHERE tabid = 1",
    ),
    (
        "teradata",
        "SELECT InfoData FROM DBC.DBCInfoV WHERE InfoKey = 'VERSION'",
    ),
    ("snowflake", "SELECT CURRENT_VERSION()"),
    ("hdb", "SELECT VERSION FROM SYS.M_DATABASE"),
    ("adaptive server", "SELECT @@version"),
    ("sql anywhere", "SELECT @@version"),
    ("sybase", "SELECT @@version"),
    (
        "exasol",
        "SELECT PARAM_VALUE FROM EXA_METADATA WHERE PARAM_NAME = 'databaseProductVersion'",
    ),
    ("ingres", "SELECT dbmsinfo('_version')"),
    (
        "monetdb",
        "SELECT value FROM sys.environment WHERE name = 'monet_version'",
    ),
    ("vertica", "SELECT version()"),
    ("redshift", "SELECT version()"),
    ("clickhouse", "SELECT version()"),
    ("duckdb", "SELECT version()"),
    ("trino", "SELECT version()"),
    ("presto", "SELECT version()"),
    ("spark", "SELECT version()"),
];

pub fn version_sql(product: &str) -> Option<&'static str> {
    let p = product.to_ascii_lowercase();
    VERSION_QUERIES
        .iter()
        .find(|(word, _)| p.contains(word))
        .map(|(_, sql)| *sql)
}

pub fn version_line(raw: &str) -> String {
    raw.lines().next().unwrap_or_default().trim().to_owned()
}

pub fn tidy_message(message: &str) -> String {
    let mut rest = message.trim();
    while rest.starts_with('[')
        && let Some(end) = rest.find(']')
    {
        rest = rest[end + 1..].trim_start();
    }
    rest.trim_end().to_owned()
}

pub fn rejected(e: &nd_driver::DriverError) -> bool {
    matches!(e, nd_driver::DriverError::Query { code: Some(s), .. } if s.starts_with("42") || s.starts_with("37"))
}

pub fn sized_type(name: &str, size: Option<&str>, digits: Option<&str>) -> String {
    const TEXT: &[&str] = &[
        "char",
        "varchar",
        "character",
        "character varying",
        "nchar",
        "nvarchar",
        "varchar2",
        "nvarchar2",
        "bpchar",
        "binary",
        "varbinary",
    ];
    const EXACT: &[&str] = &["numeric", "decimal", "dec", "number"];
    let size = size.filter(|s| s.parse::<u64>().is_ok_and(|n| n > 0));
    let lower = name.to_ascii_lowercase();
    match size {
        Some(n) if TEXT.contains(&lower.as_str()) => format!("{name}({n})"),
        Some(p) if EXACT.contains(&lower.as_str()) => {
            format!("{name}({p},{})", digits.unwrap_or("0"))
        }
        _ => name.to_owned(),
    }
}

pub fn value(v: &str) -> String {
    if v.contains([';', '{', '}']) || v.starts_with(' ') || v.ends_with(' ') {
        format!("{{{}}}", v.replace('}', "}}"))
    } else {
        v.to_owned()
    }
}

pub fn push(out: &mut String, key: &str, v: &str) {
    if v.is_empty() {
        return;
    }
    if !out.is_empty() && !out.ends_with(';') {
        out.push(';');
    }
    out.push_str(key);
    out.push('=');
    out.push_str(&value(v));
    out.push(';');
}

pub fn odbc_string(t: &nd_driver::OdbcTarget, password: Option<&str>) -> String {
    let mut out = String::new();
    if !t.dsn.trim().is_empty() {
        push(&mut out, "DSN", t.dsn.trim());
    } else if !t.driver_path.trim().is_empty() {
        push(&mut out, "DRIVER", t.driver_path.trim());
    }
    let extra = t.connect_string.trim().trim_matches(';');
    if !extra.is_empty() {
        if !out.is_empty() && !out.ends_with(';') {
            out.push(';');
        }
        out.push_str(extra);
        out.push(';');
    }
    push(&mut out, "UID", &t.user);
    push(&mut out, "PWD", password.unwrap_or_default());
    out
}

pub fn db2_string(
    driver: &str,
    t: &nd_driver::Db2Target,
    password: Option<&str>,
    via: Option<(std::net::IpAddr, u16)>,
) -> String {
    let (host, port) = via.map_or_else(|| (t.host.clone(), t.port), |(ip, p)| (ip.to_string(), p));
    let mut out = String::new();
    push(&mut out, "DRIVER", driver);
    push(&mut out, "DATABASE", &t.database);
    push(&mut out, "HOSTNAME", &host);
    push(&mut out, "PORT", &port.to_string());
    push(&mut out, "PROTOCOL", "TCPIP");
    push(&mut out, "UID", &t.user);
    push(&mut out, "PWD", password.unwrap_or_default());
    if t.tls != nd_driver::TlsMode::Disable {
        push(&mut out, "SECURITY", "SSL");
        if let Some(ca) = t.ca_file.as_deref() {
            push(&mut out, "SSLServerCertificate", ca);
        }
    }
    out
}

pub fn db2_library(home: &std::path::Path) -> std::path::PathBuf {
    let file = if cfg!(target_os = "macos") {
        "libdb2o.dylib"
    } else {
        "libdb2o.so"
    };
    let direct = home.join(file);
    if direct.is_file() {
        direct
    } else {
        home.join("lib").join(file)
    }
}

pub fn db2_driver(home: &std::path::Path) -> String {
    if cfg!(windows) {
        "IBM DB2 ODBC DRIVER".to_owned()
    } else {
        db2_library(home).to_string_lossy().into_owned()
    }
}
