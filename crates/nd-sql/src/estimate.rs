use std::fmt::Write as _;

use crate::Dialect;
use crate::split::{dollar_tag, find, memchr, skip_block_comment, skip_quoted};

struct Word {
    upper: String,
    depth: usize,
    start: usize,
    end: usize,
}

fn scan(sql: &str, dialect: Dialect) -> Option<Vec<Word>> {
    let b = sql.as_bytes();
    let mut out = Vec::new();
    let mut depth = 0usize;
    let mut i = 0;
    while i < b.len() {
        let c = b[i];
        match c {
            b'-' if b.get(i + 1) == Some(&b'-') => i = memchr(b'\n', b, i).unwrap_or(b.len()),
            b'#' if dialect.hash_comments() => i = memchr(b'\n', b, i).unwrap_or(b.len()),
            b'/' if b.get(i + 1) == Some(&b'*') => {
                if dialect == Dialect::Mysql && matches!(b.get(i + 2), Some(b'!' | b'+')) {
                    return None;
                }
                i = skip_block_comment(b, i + 2, dialect.nested_comments());
            }
            b'\'' | b'"' | b'`' => {
                i = skip_quoted(b, i + 1, c, dialect.backslash_escapes(c));
            }
            b'[' if dialect == Dialect::Sqlite => {
                i = memchr(b']', b, i + 1).map_or(b.len(), |e| e + 1);
            }
            b'[' if dialect == Dialect::Mssql => i = skip_quoted(b, i + 1, b']', false),
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
            b',' if depth == 0 => {
                out.push(Word {
                    upper: ",".into(),
                    depth,
                    start: i,
                    end: i + 1,
                });
                i += 1;
            }
            _ if c.is_ascii_alphabetic() || c == b'_' => {
                let end = b[i..]
                    .iter()
                    .position(|x| !(x.is_ascii_alphanumeric() || *x == b'_' || *x == b'$'))
                    .map_or(b.len(), |p| i + p);
                out.push(Word {
                    upper: sql[i..end].to_ascii_uppercase(),
                    depth,
                    start: i,
                    end,
                });
                i = end;
            }
            _ => i += 1,
        }
    }
    Some(out)
}

struct Single<'a> {
    table: &'a str,
    set: Option<&'a str>,
    cond: Option<&'a str>,
}

fn single(sql: &str, dialect: Dialect) -> Option<Single<'_>> {
    let w = scan(sql, dialect)?;
    let top: Vec<&Word> = w.iter().filter(|x| x.depth == 0).collect();
    let at = |word: &str| top.iter().position(|x| x.upper == word);
    let (target, rest, set) = match top.first()?.upper.as_str() {
        "DELETE" => {
            let from = at("FROM").filter(|&f| f == 1)?;
            let end = top
                .iter()
                .skip(from + 1)
                .position(|x| {
                    ["WHERE", "USING", "RETURNING", "ORDER", "LIMIT", "OUTPUT"]
                        .contains(&x.upper.as_str())
                })
                .map_or(top.len(), |p| p + from + 1);
            (from + 1..end, end, None)
        }
        "UPDATE" => {
            let mut first = 1;
            if top.get(1).is_some_and(|x| x.upper == "OR") {
                first = 3;
            }
            let set = at("SET")?;
            if top[set..]
                .iter()
                .any(|x| matches!(x.upper.as_str(), "FROM" | "OUTPUT"))
            {
                return None;
            }
            let end = top
                .iter()
                .skip(set)
                .position(|x| ["WHERE", "RETURNING", "ORDER", "LIMIT"].contains(&x.upper.as_str()))
                .map_or(top.len(), |p| p + set);
            let assigned = &sql[top[set].end..top.get(end).map_or(sql.len(), |x| x.start)];
            (first..set, end, Some(assigned))
        }
        _ => return None,
    };
    let names = top.get(target.clone())?;
    let joined = names
        .iter()
        .any(|x| matches!(x.upper.as_str(), "," | "JOIN" | "STRAIGHT_JOIN" | "TOP"));
    if names.is_empty() || joined {
        return None;
    }
    let table_from = top[target.start].start;
    let table_to = top.get(target.end).map_or(sql.len(), |x| x.start);
    let table = sql[table_from..table_to].trim();
    let mut cond = None;
    if let Some(kw) = top.get(rest).filter(|x| x.upper == "WHERE") {
        if top.get(rest + 1).is_some_and(|x| x.upper == "CURRENT") {
            return None;
        }
        if top[rest..]
            .iter()
            .any(|x| matches!(x.upper.as_str(), "ORDER" | "LIMIT"))
        {
            return None;
        }
        let cond_to = top[rest..]
            .iter()
            .find(|x| x.upper == "RETURNING")
            .map_or(sql.len(), |x| x.start);
        cond = Some(sql[kw.end..cond_to].trim());
    } else if top.get(rest).is_some_and(|x| x.upper != "RETURNING") {
        return None;
    }
    Some(Single {
        table,
        set: set.map(str::trim),
        cond,
    })
}

fn with_cond(mut out: String, cond: Option<&str>) -> String {
    if let Some(c) = cond {
        out.push_str("\nWHERE ");
        out.push_str(c);
        out.push('\n');
    }
    out
}

pub fn count_query(sql: &str, dialect: Dialect) -> Option<String> {
    let s = single(sql, dialect)?;
    let out = with_cond(format!("SELECT count(*) FROM {}", s.table), s.cond);
    crate::is_read(&out, dialect).then_some(out)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Preview {
    pub sql: String,
    pub assigned: Vec<String>,
    pub relation: Option<(String, String)>,
    pub exact_only: bool,
}

fn assignment(part: &str) -> Option<(String, &str)> {
    let b = part.as_bytes();
    let mut i = 0;
    loop {
        let name = match b.get(i)? {
            q @ (b'"' | b'`' | b'[') => {
                let close = if *q == b'[' { b']' } else { *q };
                let end = memchr(close, b, i + 1)?;
                let name = &part[i + 1..end];
                i = end + 1;
                name
            }
            c if c.is_ascii_alphabetic() || *c == b'_' => {
                let end = b[i..]
                    .iter()
                    .position(|x| !(x.is_ascii_alphanumeric() || *x == b'_' || *x == b'$'))
                    .map_or(b.len(), |p| i + p);
                let name = &part[i..end];
                i = end;
                name
            }
            _ => return None,
        };
        while b.get(i).is_some_and(u8::is_ascii_whitespace) {
            i += 1;
        }
        match b.get(i)? {
            b'.' => {
                i += 1;
                while b.get(i).is_some_and(u8::is_ascii_whitespace) {
                    i += 1;
                }
            }
            b'=' => {
                return Some((name.to_owned(), part[i + 1..].trim()))
                    .filter(|(_, e)| !e.is_empty());
            }
            _ => return None,
        }
    }
}

fn top_commas(text: &str, dialect: Dialect) -> Option<Vec<&str>> {
    let mut parts = Vec::new();
    let mut from = 0;
    for w in scan(text, dialect)? {
        if w.upper == "," && w.depth == 0 {
            parts.push(&text[from..w.start]);
            from = w.end;
        }
    }
    parts.push(&text[from..]);
    Some(parts)
}

pub fn preview_query(sql: &str, dialect: Dialect, limit: u32) -> Option<Preview> {
    let s = single(sql, dialect)?;
    let mut assigned = Vec::new();
    let mut extra = String::new();
    let mut exact_only = false;
    if let Some(set) = s.set {
        for (i, part) in top_commas(set, dialect)?.into_iter().enumerate() {
            let (name, expr) = assignment(part.trim())?;
            let expr = if expr.eq_ignore_ascii_case("DEFAULT") {
                exact_only = true;
                "NULL"
            } else {
                expr
            };
            let _ = write!(extra, ",\n  ({expr}) AS nd_after_{}", i + 1);
            assigned.push(name);
        }
    }
    let top = if dialect == Dialect::Mssql {
        format!("TOP ({limit}) ")
    } else {
        String::new()
    };
    let star = if dialect == Dialect::Oracle {
        let table = s.table.trim();
        let alias = table.rsplit(char::is_whitespace).next().unwrap_or(table);
        format!("{alias}.*")
    } else {
        "*".to_owned()
    };
    let mut out = with_cond(
        format!("SELECT {top}{star}{extra}\nFROM {}", s.table),
        s.cond,
    );
    if dialect != Dialect::Mssql {
        if !out.ends_with('\n') {
            out.push('\n');
        }
        if dialect == Dialect::Oracle {
            let _ = write!(out, "FETCH FIRST {limit} ROWS ONLY");
        } else {
            let _ = write!(out, "LIMIT {limit}");
        }
    }
    let relation = (!assigned.is_empty())
        .then(|| object_name(s.table, dialect))
        .flatten();
    crate::is_read(&out, dialect).then_some(Preview {
        sql: out,
        assigned,
        relation,
        exact_only,
    })
}

fn object_name(text: &str, dialect: Dialect) -> Option<(String, String)> {
    let fold = matches!(dialect, Dialect::Postgres | Dialect::Duckdb);
    let mut parts = Vec::new();
    let mut chars = text.trim().chars().peekable();
    loop {
        let close = match chars.peek()? {
            '"' => Some('"'),
            '`' => Some('`'),
            '[' => Some(']'),
            _ => None,
        };
        let mut part = String::new();
        if let Some(close) = close {
            chars.next();
            loop {
                let c = chars.next()?;
                if c == close {
                    if chars.peek() == Some(&close) {
                        chars.next();
                        part.push(c);
                    } else {
                        break;
                    }
                } else {
                    part.push(c);
                }
            }
        } else {
            while let Some(&c) = chars.peek() {
                if c.is_alphanumeric() || matches!(c, '_' | '$' | '#' | '@') {
                    part.push(if fold { c.to_ascii_lowercase() } else { c });
                    chars.next();
                } else {
                    break;
                }
            }
            if part.is_empty() {
                return None;
            }
        }
        parts.push(part);
        if chars.peek() == Some(&'.') {
            chars.next();
        } else {
            break;
        }
    }
    match chars.next() {
        None => {}
        Some(c) if c.is_whitespace() => {}
        Some(_) => return None,
    }
    let name = parts.pop()?;
    let schema = parts.pop().unwrap_or_default();
    Some((schema, name))
}
