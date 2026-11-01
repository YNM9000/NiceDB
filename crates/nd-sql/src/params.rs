use std::collections::HashSet;
use std::fmt;
use std::ops::Range;

use serde::{Deserialize, Serialize};

use crate::Dialect;
use crate::split::{
    dollar_tag, find, line_comment, memchr, skip_block_comment, skip_bracket, skip_quoted,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Placeholder {
    pub range: Range<usize>,
    pub name: ParamName,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ParamName {
    Named(String),
    Positional(usize),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "snake_case")]
pub enum ParamKind {
    Text,
    Number,
    Date,
    Boolean,
    Null,
    Raw,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
pub struct ParamValue {
    pub name: String,
    pub kind: ParamKind,
    pub value: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ParamError {
    Missing { name: String },
    Invalid { name: String, as_kind: ParamKind },
}

impl fmt::Display for ParamError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Missing { name } => write!(f, "no value for parameter {name}"),
            Self::Invalid { name, as_kind } => {
                write!(f, "the value of {name} is not a valid {as_kind:?}")
            }
        }
    }
}

impl std::error::Error for ParamError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Filled {
    pub sql: String,
    pub params: Vec<Option<String>>,
    edits: Vec<(Range<usize>, Range<usize>)>,
}

impl Filled {
    pub fn plain(sql: &str) -> Self {
        Self {
            sql: sql.to_owned(),
            params: Vec::new(),
            edits: Vec::new(),
        }
    }

    pub fn changed(&self) -> bool {
        !self.edits.is_empty()
    }

    pub fn source_offset(&self, at: usize) -> usize {
        let mut shift = 0isize;
        for (out, src) in &self.edits {
            if at < out.start {
                break;
            }
            if at < out.end {
                return src.start;
            }
            shift = src.end.cast_signed() - out.end.cast_signed();
        }
        at.saturating_add_signed(shift)
    }
}

const VALUE_AFTER: [&str; 24] = [
    "select", "where", "and", "or", "not", "in", "like", "ilike", "then", "else", "when", "by",
    "limit", "offset", "values", "set", "is", "between", "on", "having", "case", "return",
    "distinct", "fetch",
];

#[allow(clippy::too_many_lines)]
pub fn placeholders(sql: &str, dialect: Dialect) -> Vec<Placeholder> {
    let b = sql.as_bytes();
    let mut out = Vec::new();
    let mut positional = 0;
    let mut prev: Option<u8> = None;
    let mut word = 0..0;
    let tsql = (dialect == Dialect::Mssql).then(|| tsql_variables(sql));
    let mut i = 0;
    while i < b.len() {
        let c = b[i];
        if let Some((found, next)) = engine_form(sql, i, dialect, tsql.as_ref()) {
            out.extend(found);
            prev = Some(b'x');
            i = next;
            continue;
        }
        match c {
            b'-' | b'#' if line_comment(b, i, dialect) => {
                i = memchr(b'\n', b, i).unwrap_or(b.len());
                continue;
            }
            b'/' if b.get(i + 1) == Some(&b'*') => {
                i = skip_block_comment(b, i + 2, dialect.nested_comments());
                continue;
            }
            b'\'' | b'"' | b'`' => {
                i = skip_quoted(b, i + 1, c, dialect.backslash_escapes(c));
                prev = Some(c);
                continue;
            }
            b'[' if matches!(dialect, Dialect::Sqlite | Dialect::Mssql) => {
                i = skip_bracket(b, i, dialect);
                prev = Some(b']');
                continue;
            }
            b'$' if b.get(i + 1) == Some(&b'{') => {
                if let Some(end) = braced(b, i + 2) {
                    out.push(Placeholder {
                        range: i..end + 1,
                        name: ParamName::Named(sql[i + 2..end].to_owned()),
                    });
                    i = end + 1;
                    prev = Some(b'}');
                    continue;
                }
            }
            b'$' if dialect.dollar_quotes() => {
                if let Some(tag_end) = dollar_tag(b, i) {
                    let tag = &b[i..=tag_end];
                    i = find(b, tag, tag_end + 1).map_or(b.len(), |e| e + tag.len());
                    prev = Some(b'$');
                    continue;
                }
            }
            b':' => {
                let next = b.get(i + 1).copied();
                if matches!(next, Some(b':' | b'=')) {
                    i += 2;
                    prev = Some(b':');
                    continue;
                }
                let joined = i > 0 && (b[i - 1].is_ascii_alphanumeric() || b[i - 1] == b'_');
                let end = ident_end(b, i + 1);
                let row_ref = dialect == Dialect::Oracle
                    && b.get(end) == Some(&b'.')
                    && ["new", "old", "parent"]
                        .iter()
                        .any(|w| sql[i + 1..end].eq_ignore_ascii_case(w));
                if end > i + 1 && !joined && !row_ref && !matches!(prev, Some(b'\'' | b'"')) {
                    out.push(Placeholder {
                        range: i..end,
                        name: ParamName::Named(sql[i + 1..end].to_owned()),
                    });
                    i = end;
                    prev = Some(b'x');
                    word = end..end;
                    continue;
                }
            }
            b'?' => {
                let (found, next) =
                    question(sql, i, dialect, prev, &sql[word.clone()], &mut positional);
                out.extend(found);
                prev = Some(b'?');
                i = next;
                continue;
            }
            _ if c.is_ascii_alphabetic() || c == b'_' => {
                let end = ident_end(b, i);
                if dialect == Dialect::Oracle
                    && let Some(next) = crate::split::q_quote(b, i, end)
                {
                    prev = Some(b'\'');
                    i = next;
                    continue;
                }
                word = i..end;
                prev = Some(b[end - 1]);
                i = end;
                continue;
            }
            _ if !c.is_ascii_whitespace() => prev = Some(c),
            _ => {}
        }
        i += 1;
    }
    out
}

fn engine_form(
    sql: &str,
    i: usize,
    dialect: Dialect,
    tsql: Option<&(HashSet<String>, bool)>,
) -> Option<(Option<Placeholder>, usize)> {
    let b = sql.as_bytes();
    let c = b[i];
    if i > 0 && (b[i - 1].is_ascii_alphanumeric() || matches!(b[i - 1], b'_' | b'$' | b'@')) {
        return None;
    }
    let named = |end: usize| Placeholder {
        range: i..end,
        name: ParamName::Named(sql[i + 1..end].to_owned()),
    };
    if c == b'$'
        && matches!(dialect, Dialect::Postgres | Dialect::Duckdb)
        && b.get(i + 1).is_some_and(u8::is_ascii_digit)
    {
        let end = b[i + 1..]
            .iter()
            .position(|x| !x.is_ascii_digit())
            .map_or(b.len(), |p| i + 1 + p);
        return Some((Some(named(end)), end));
    }
    let wanted = dialect == Dialect::Sqlite || (c == b'@' && dialect == Dialect::Mssql);
    if !matches!(c, b'@' | b'$') || !wanted || b.get(i + 1) == Some(&b'{') {
        return None;
    }
    if c == b'@' && b.get(i + 1) == Some(&b'@') {
        return Some((None, ident_end(b, i + 2).max(i + 2)));
    }
    let end = ident_end(b, i + 1);
    if end == i + 1 {
        return None;
    }
    let line = &sql[sql[..i].rfind(['\n', ';']).map_or(0, |p| p + 1)..i];
    let exec = line
        .split(|c: char| !c.is_ascii_alphanumeric())
        .any(|w| w.eq_ignore_ascii_case("exec") || w.eq_ignore_ascii_case("execute"));
    let argument = exec && sql[end..].trim_start().starts_with('=');
    let variable = tsql.is_some_and(|(declared, routine)| {
        *routine || declared.contains(&sql[i + 1..end].to_ascii_lowercase())
    });
    Some(((!argument && !variable).then(|| named(end)), end))
}

fn tsql_variables(sql: &str) -> (HashSet<String>, bool) {
    let b = sql.as_bytes();
    let mut declared = HashSet::new();
    let mut routine = false;
    let mut declaring = false;
    let mut item = false;
    let mut depth = 0i32;
    let mut last_word = String::new();
    let mut tail: Option<u8> = None;
    let mut i = 0;
    while i < b.len() {
        let c = b[i];
        match c {
            b'-' if b.get(i + 1) == Some(&b'-') => {
                i = memchr(b'\n', b, i).unwrap_or(b.len());
                continue;
            }
            b'/' if b.get(i + 1) == Some(&b'*') => {
                i = skip_block_comment(b, i + 2, true);
                continue;
            }
            b'\'' | b'"' => {
                i = skip_quoted(b, i + 1, c, false);
                tail = Some(c);
                continue;
            }
            b'[' => {
                i = skip_bracket(b, i, Dialect::Mssql);
                tail = Some(b']');
                continue;
            }
            b'@' => {
                let end = ident_end(b, i + 1);
                if declaring && item && end > i + 1 {
                    declared.insert(sql[i + 1..end].to_ascii_lowercase());
                }
                item = false;
                tail = Some(b'x');
                i = end.max(i + 1);
                continue;
            }
            b'(' => depth += 1,
            b')' => depth -= 1,
            b',' if depth == 0 && declaring => item = true,
            b';' => declaring = false,
            b'\n' if tail != Some(b',') => declaring = false,
            _ if c.is_ascii_alphabetic() || c == b'_' => {
                let end = ident_end(b, i);
                let w = sql[i..end].to_ascii_lowercase();
                if w == "declare" {
                    declaring = true;
                    item = true;
                    depth = 0;
                } else {
                    item = false;
                }
                if matches!(w.as_str(), "proc" | "procedure" | "function" | "trigger")
                    && matches!(last_word.as_str(), "create" | "alter" | "or")
                {
                    routine = true;
                }
                last_word = w;
                tail = Some(b'x');
                i = end;
                continue;
            }
            _ => {}
        }
        if !c.is_ascii_whitespace() {
            tail = Some(c);
        }
        i += 1;
    }
    (declared, routine)
}

fn question(
    sql: &str,
    i: usize,
    dialect: Dialect,
    prev: Option<u8>,
    word: &str,
    positional: &mut usize,
) -> (Option<Placeholder>, usize) {
    let b = sql.as_bytes();
    let digits = b[i + 1..].iter().take_while(|x| x.is_ascii_digit()).count();
    if digits > 0 && dialect == Dialect::Sqlite {
        let end = i + 1 + digits;
        let name = ParamName::Named(sql[i + 1..end].to_owned());
        return (
            Some(Placeholder {
                range: i..end,
                name,
            }),
            end,
        );
    }
    if json_operator(b, i, dialect, prev, word) {
        return (None, i + 1);
    }
    *positional += 1;
    let name = ParamName::Positional(*positional);
    (
        Some(Placeholder {
            range: i..i + 1,
            name,
        }),
        i + 1,
    )
}

fn ident_end(b: &[u8], from: usize) -> usize {
    if !b
        .get(from)
        .is_some_and(|c| c.is_ascii_alphabetic() || *c == b'_')
    {
        return from;
    }
    b[from..]
        .iter()
        .position(|x| !(x.is_ascii_alphanumeric() || *x == b'_'))
        .map_or(b.len(), |p| from + p)
}

fn braced(b: &[u8], from: usize) -> Option<usize> {
    let end = ident_end(b, from);
    (end > from && b.get(end) == Some(&b'}')).then_some(end)
}

fn json_operator(b: &[u8], i: usize, dialect: Dialect, prev: Option<u8>, word: &str) -> bool {
    if !dialect.dollar_quotes() {
        return false;
    }
    if matches!(b.get(i + 1), Some(b'|' | b'&')) {
        return true;
    }
    match prev {
        Some(p) if p.is_ascii_alphanumeric() || p == b'_' => {
            !VALUE_AFTER.iter().any(|w| word.eq_ignore_ascii_case(w))
        }
        Some(b')' | b']' | b'"' | b'\'') => true,
        _ => false,
    }
}

impl Placeholder {
    pub fn key(&self, first_positional: usize) -> String {
        match &self.name {
            ParamName::Named(n) => n.clone(),
            ParamName::Positional(n) => (n + first_positional).to_string(),
        }
    }
}

pub fn bind(
    sql: &str,
    dialect: Dialect,
    slots: &[(Range<usize>, &ParamValue)],
    native: bool,
) -> Result<Filled, ParamError> {
    let mut out = String::with_capacity(sql.len() + 16);
    let mut params: Vec<Option<String>> = Vec::new();
    let mut numbered: Vec<&str> = Vec::new();
    let mut edits = Vec::with_capacity(slots.len());
    let mut last = 0;
    for (range, value) in slots {
        out.push_str(&sql[last..range.start]);
        let start = out.len();
        let piece = if native && matches!(value.kind, ParamKind::Text | ParamKind::Date) {
            checked(value, dialect)?;
            native_marker(dialect, value, &mut params, &mut numbered)
        } else {
            literal(value, dialect)?
        };
        out.push_str(&piece);
        edits.push((start..out.len(), range.clone()));
        last = range.end;
    }
    out.push_str(&sql[last..]);
    Ok(Filled {
        sql: out,
        params,
        edits,
    })
}

fn native_marker<'a>(
    dialect: Dialect,
    value: &'a ParamValue,
    params: &mut Vec<Option<String>>,
    numbered: &mut Vec<&'a str>,
) -> String {
    let shared = !matches!(dialect, Dialect::Sqlite | Dialect::Mysql | Dialect::Oracle);
    let n = match numbered.iter().position(|n| *n == value.name) {
        Some(i) if shared => i + 1,
        _ => {
            numbered.push(&value.name);
            params.push(Some(value.value.clone()));
            params.len()
        }
    };
    crate::quote::placeholder(dialect, n)
}

fn invalid(value: &ParamValue) -> ParamError {
    ParamError::Invalid {
        name: value.name.clone(),
        as_kind: value.kind,
    }
}

fn checked(value: &ParamValue, _dialect: Dialect) -> Result<(), ParamError> {
    let v = value.value.trim();
    let ok = match value.kind {
        ParamKind::Text | ParamKind::Null => true,
        ParamKind::Number => is_number(v),
        ParamKind::Date => is_date(v),
        ParamKind::Boolean => boolean(v).is_some(),
        ParamKind::Raw => is_name(v),
    };
    if ok { Ok(()) } else { Err(invalid(value)) }
}

pub fn literal(value: &ParamValue, dialect: Dialect) -> Result<String, ParamError> {
    checked(value, dialect)?;
    let v = value.value.trim();
    Ok(match value.kind {
        ParamKind::Text => crate::quote::string(dialect, &value.value),
        ParamKind::Date => crate::quote::string(dialect, v),
        ParamKind::Null => "NULL".into(),
        ParamKind::Number if v.starts_with('-') => format!("({v})"),
        ParamKind::Number | ParamKind::Raw => v.to_owned(),
        ParamKind::Boolean => {
            let on = boolean(v).unwrap_or(false);
            match (dialect, on) {
                (Dialect::Sqlite | Dialect::Mssql, true) => "1".into(),
                (Dialect::Sqlite | Dialect::Mssql, false) => "0".into(),
                (_, true) => "TRUE".into(),
                (_, false) => "FALSE".into(),
            }
        }
    })
}

fn is_number(v: &str) -> bool {
    let body = v.strip_prefix(['-', '+']).unwrap_or(v);
    let (mantissa, exponent) = match body.find(['e', 'E']) {
        Some(p) => (&body[..p], Some(&body[p + 1..])),
        None => (body, None),
    };
    let mut parts = mantissa.splitn(2, '.');
    let whole = parts.next().unwrap_or("");
    let frac = parts.next();
    let digits = |s: &str| s.bytes().all(|c| c.is_ascii_digit());
    let mantissa_ok = digits(whole)
        && frac.is_none_or(digits)
        && !(whole.is_empty() && frac.is_none_or(str::is_empty));
    let exponent_ok = exponent.is_none_or(|e| {
        let e = e.strip_prefix(['-', '+']).unwrap_or(e);
        !e.is_empty() && digits(e)
    });
    mantissa_ok && exponent_ok
}

fn is_date(v: &str) -> bool {
    let b = v.as_bytes();
    b.len() >= 10
        && b[..4].iter().all(u8::is_ascii_digit)
        && b[4] == b'-'
        && b[5..7].iter().all(u8::is_ascii_digit)
        && b[7] == b'-'
        && b[8..10].iter().all(u8::is_ascii_digit)
        && b[10..]
            .iter()
            .all(|c| c.is_ascii_digit() || b" T:.+-Z".contains(c))
}

fn boolean(v: &str) -> Option<bool> {
    match v.to_ascii_lowercase().as_str() {
        "true" | "1" | "t" | "yes" | "on" | "sì" | "si" | "vero" => Some(true),
        "false" | "0" | "f" | "no" | "off" | "falso" => Some(false),
        _ => None,
    }
}

fn is_name(v: &str) -> bool {
    !v.is_empty()
        && v.split('.').all(|part| {
            let b = part.as_bytes();
            b.first()
                .is_some_and(|c| c.is_ascii_alphabetic() || *c == b'_')
                && b.iter()
                    .all(|c| c.is_ascii_alphanumeric() || *c == b'_' || *c == b'$')
        })
}
