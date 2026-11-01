use serde::Serialize;

use crate::Dialect;
use crate::split::{dollar_tag, find, memchr, q_quote, skip_block_comment, skip_quoted};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "snake_case")]
pub enum Risk {
    UpdateWithoutWhere,
    DeleteWithoutWhere,
    Drop,
    Truncate,
    Alter,
    Flush,
    Server,
    FullScan,
}

fn quoted(sql: &str, open: usize, end: usize, close: u8) -> &str {
    let to = if end > open + 1 && sql.as_bytes().get(end - 1) == Some(&close) {
        end - 1
    } else {
        end
    };
    &sql[open + 1..to.max(open + 1)]
}

pub(crate) fn words(sql: &str, dialect: Dialect, idents: bool) -> Vec<(String, usize)> {
    let b = sql.as_bytes();
    let mut out = Vec::new();
    let mut depth = 0usize;
    let mut i = 0;
    while i < b.len() {
        let c = b[i];
        match c {
            b'-' if b.get(i + 1) == Some(&b'-') => {
                i = memchr(b'\n', b, i).unwrap_or(b.len());
            }
            b'#' if dialect.hash_comments() => {
                i = memchr(b'\n', b, i).unwrap_or(b.len());
            }
            b'/' if dialect == Dialect::Mysql
                && b.get(i + 1) == Some(&b'*')
                && b.get(i + 2) == Some(&b'!') =>
            {
                i += 3;
                while b.get(i).is_some_and(u8::is_ascii_digit) {
                    i += 1;
                }
            }
            b'/' if b.get(i + 1) == Some(&b'*') => {
                i = skip_block_comment(b, i + 2, dialect.nested_comments());
            }
            b'\'' | b'"' | b'`' => {
                let end = skip_quoted(b, i + 1, c, dialect.backslash_escapes(c));
                let ident = c == b'`' || (c == b'"' && dialect.double_quotes_name());
                if idents && ident {
                    out.push((quoted(sql, i, end, c).to_ascii_uppercase(), depth));
                }
                i = end;
            }
            b'[' if matches!(dialect, Dialect::Sqlite | Dialect::Mssql) => {
                let end = skip_quoted(b, i + 1, b']', false);
                if idents {
                    out.push((quoted(sql, i, end, b']').to_ascii_uppercase(), depth));
                }
                i = end;
            }
            b'$' if dialect.dollar_quotes() => {
                if let Some(end) = dollar_tag(b, i) {
                    let tag = &b[i..=end];
                    i = find(b, tag, end + 1).map_or(b.len(), |e| e + tag.len());
                } else {
                    i += 1;
                }
            }
            b'(' => {
                depth += 1;
                i += 1;
            }
            b')' => {
                depth = depth.saturating_sub(1);
                i += 1;
            }
            _ if c.is_ascii_alphabetic() || c == b'_' => {
                let end = b[i..]
                    .iter()
                    .position(|x| !(x.is_ascii_alphanumeric() || *x == b'_'))
                    .map_or(b.len(), |p| i + p);
                if dialect == Dialect::Oracle
                    && let Some(next) = q_quote(b, i, end)
                {
                    i = next;
                    continue;
                }
                out.push((sql[i..end].to_ascii_uppercase(), depth));
                i = end;
            }
            _ => i += 1,
        }
    }
    out
}

const VERBS: &[&str] = &["SELECT", "INSERT", "UPDATE", "DELETE", "MERGE", "VALUES"];

pub(crate) fn verb(words: &[(String, usize)]) -> Option<&str> {
    let first = words.first()?.0.as_str();
    if first != "WITH" {
        return Some(first);
    }
    words
        .iter()
        .skip(1)
        .find(|(w, d)| *d == 0 && VERBS.contains(&w.as_str()))
        .map(|(w, _)| w.as_str())
}

fn top_level(words: &[(String, usize)], word: &str) -> bool {
    words.iter().any(|(w, d)| *d == 0 && w == word)
}

fn unbounded_write(w: &[(String, usize)]) -> Option<Risk> {
    for (i, (word, depth)) in w.iter().enumerate() {
        let risk = match word.as_str() {
            "UPDATE" => Risk::UpdateWithoutWhere,
            "DELETE" => Risk::DeleteWithoutWhere,
            _ => continue,
        };
        let prev = i
            .checked_sub(1)
            .and_then(|p| w.get(p))
            .map(|(x, _)| x.as_str());
        if matches!(prev, Some("FOR" | "ON" | "DO" | "KEY" | "GRANT" | "REVOKE")) {
            continue;
        }
        let bounded = w[i + 1..]
            .iter()
            .take_while(|(_, d)| d >= depth)
            .any(|(x, d)| d == depth && x == "WHERE");
        if !bounded {
            return Some(risk);
        }
    }
    None
}

pub fn risk(sql: &str, dialect: Dialect) -> Option<Risk> {
    let w = words(sql, dialect, false);
    match verb(&w)? {
        "UPDATE" | "DELETE" | "WITH" | "SELECT" | "INSERT" => unbounded_write(&w),
        "EXPLAIN" if w.iter().any(|(x, _)| x == "ANALYZE") => unbounded_write(&w),
        "DROP" => Some(Risk::Drop),
        "TRUNCATE" => Some(Risk::Truncate),
        "ALTER" => Some(Risk::Alter),
        _ => None,
    }
}

pub fn has_limit(sql: &str, dialect: Dialect) -> bool {
    let w = words(sql, dialect, false);
    ["LIMIT", "FETCH", "TOP"].iter().any(|k| top_level(&w, k))
        || (dialect == Dialect::Oracle && top_level(&w, "ROWNUM"))
}

fn oracle_commits(first: &str, second: &str) -> bool {
    match first {
        "CREATE" | "DROP" | "TRUNCATE" | "RENAME" | "GRANT" | "REVOKE" | "COMMENT" | "ANALYZE"
        | "AUDIT" | "NOAUDIT" | "PURGE" | "FLASHBACK" => true,
        "ALTER" => !matches!(second, "SESSION" | "SYSTEM"),
        _ => false,
    }
}

const SIDE_EFFECTS: &[&str] = &[
    "INSERT",
    "UPDATE",
    "DELETE",
    "MERGE",
    "SET_CONFIG",
    "PG_TERMINATE_BACKEND",
    "PG_CANCEL_BACKEND",
    "PG_RELOAD_CONF",
    "DBLINK",
    "DBLINK_EXEC",
    "LO_IMPORT",
    "LO_EXPORT",
    "LO_UNLINK",
    "NEXTVAL",
    "SETVAL",
    "OUTFILE",
    "DUMPFILE",
    "OPENROWSET",
    "OPENQUERY",
    "OPENDATASOURCE",
    "XP_CMDSHELL",
];

pub fn is_read(sql: &str, dialect: Dialect) -> bool {
    let w = words(sql, dialect, true);
    let writes = w.iter().any(|(x, _)| SIDE_EFFECTS.contains(&x.as_str()));
    match verb(&w) {
        Some("SELECT" | "VALUES" | "TABLE" | "FROM") => !writes && !top_level(&w, "INTO"),
        Some("EXPLAIN" | "SHOW" | "DESCRIBE" | "DESC" | "USE" | "SUMMARIZE" | "EXISTS") => !writes,
        Some("PRAGMA") => read_pragma(sql, &w),
        _ => false,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TxFit {
    Held,
    Commits,
    Outside,
}

const MYSQL_COMMITTING: &[&str] = &[
    "CREATE",
    "ALTER",
    "DROP",
    "TRUNCATE",
    "RENAME",
    "GRANT",
    "REVOKE",
    "LOCK",
    "UNLOCK",
    "ANALYZE",
    "OPTIMIZE",
    "REPAIR",
    "INSTALL",
    "UNINSTALL",
    "FLUSH",
    "RESET",
    "CACHE",
    "LOAD",
];

pub fn transaction_fit(sql: &str, dialect: Dialect) -> TxFit {
    let w = words(sql, dialect, false);
    let at = |i: usize| w.get(i).map_or("", |(x, _)| x.as_str());
    let top = |word: &str| top_level(&w, word);
    let (first, second) = (at(0), at(1));
    let outside = match dialect {
        Dialect::Postgres => {
            first == "VACUUM"
                || (matches!(first, "CREATE" | "DROP")
                    && matches!(second, "DATABASE" | "TABLESPACE" | "SUBSCRIPTION"))
                || (first == "ALTER" && second == "SYSTEM")
                || (first == "REINDEX" && matches!(second, "DATABASE" | "SYSTEM"))
                || (matches!(first, "CREATE" | "DROP" | "REINDEX") && top("CONCURRENTLY"))
        }
        Dialect::Mssql => {
            matches!(first, "BACKUP" | "RESTORE" | "RECONFIGURE")
                || (matches!(first, "CREATE" | "ALTER" | "DROP") && second == "DATABASE")
                || (first == "CREATE" && second == "FULLTEXT")
        }
        Dialect::Sqlite => matches!(first, "VACUUM" | "ATTACH" | "DETACH"),
        Dialect::Duckdb => matches!(first, "VACUUM" | "CHECKPOINT" | "FORCE"),
        Dialect::Firebird => {
            (matches!(first, "CREATE" | "DROP" | "ALTER") && second == "DATABASE")
                || (first == "SET" && second == "TERM")
        }
        Dialect::Hana => {
            matches!(first, "BACKUP" | "RECOVER" | "IMPORT" | "EXPORT")
                || (first == "ALTER" && second == "SYSTEM")
        }
        Dialect::Db2 => matches!(
            first,
            "BACKUP" | "RESTORE" | "REORG" | "RUNSTATS" | "LOAD" | "IMPORT" | "EXPORT"
        ),
        Dialect::Mysql
        | Dialect::Clickhouse
        | Dialect::Oracle
        | Dialect::Ansi
        | Dialect::AnsiBacktick
        | Dialect::Cql
        | Dialect::Partiql
        | Dialect::Opensearch
        | Dialect::Elasticsearch => false,
    };
    if outside {
        return TxFit::Outside;
    }
    let temporary = matches!(first, "CREATE" | "DROP") && second == "TEMPORARY";
    if dialect == Dialect::Hana && matches!(first, "CREATE" | "ALTER" | "DROP" | "RENAME") {
        return TxFit::Commits;
    }
    if dialect == Dialect::Oracle && oracle_commits(first, second) {
        return TxFit::Commits;
    }
    if dialect == Dialect::Mysql
        && !temporary
        && (MYSQL_COMMITTING.contains(&first) || (first == "SET" && second == "PASSWORD"))
    {
        return TxFit::Commits;
    }
    TxFit::Held
}

const READ_PRAGMAS: &[&str] = &[
    "TABLE_INFO",
    "TABLE_XINFO",
    "TABLE_LIST",
    "INDEX_LIST",
    "INDEX_INFO",
    "INDEX_XINFO",
    "FOREIGN_KEY_LIST",
    "FOREIGN_KEY_CHECK",
    "INTEGRITY_CHECK",
    "QUICK_CHECK",
];

fn read_pragma(sql: &str, w: &[(String, usize)]) -> bool {
    if sql.contains('=') {
        return false;
    }
    let name = w
        .iter()
        .skip(1)
        .find(|(_, d)| *d == 0)
        .map(|(x, _)| x.as_str());
    let name = match (name, w.get(2)) {
        (Some(_), Some((n, 0))) if sql.contains('.') => n.as_str(),
        (Some(n), _) => n,
        (None, _) => return false,
    };
    !sql.contains('(') || READ_PRAGMAS.contains(&name)
}
