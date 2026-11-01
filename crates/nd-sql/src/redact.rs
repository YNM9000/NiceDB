use std::borrow::Cow;
use std::ops::Range;

use crate::Dialect;

const MASK: &str = "***";

const NOT_A_PASSWORD: &[&str] = &[
    "NULL", "EXPIRE", "EXPIRED", "FOR", "VALID", "REQUIRE", "HISTORY", "REUSE", "LOCK", "UNLOCK",
    "DEFAULT", "RETAIN", "DISCARD", "LIFETIME", "NEVER", "NO", "DISABLE", "ENABLE", "CHANGE",
    "FROM", "WHERE", "AND", "OR", "AS", "IS", "IN", "NOT", "SET", "WITH", "BY", "ON", "TO",
    "ORDER", "GROUP", "LIMIT", "JOIN", "INTO", "VALUES", "CURRENT", "INITIAL", "LOGIN", "NOLOGIN",
    "FORCE", "UNTIL",
];

#[must_use]
pub fn redact_secrets(text: &str, dialect: Option<Dialect>) -> Cow<'_, str> {
    let mut spans = if let Some(d) = dialect {
        sql_spans(text, d)
    } else {
        let mut s = redis_spans(text);
        s.extend(mongo_spans(text));
        s
    };
    if spans.is_empty() {
        return Cow::Borrowed(text);
    }
    spans.sort_by_key(|r| r.start);
    let mut out = String::with_capacity(text.len());
    let mut at = 0;
    for r in spans {
        if r.start < at {
            continue;
        }
        out.push_str(&text[at..r.start]);
        out.push_str(MASK);
        at = r.end;
    }
    out.push_str(&text[at..]);
    Cow::Owned(out)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    Word,
    Str,
    Punct(char),
}

#[derive(Debug, Clone)]
struct Tok {
    kind: Kind,
    span: Range<usize>,
}

impl Tok {
    fn is_word(&self, text: &str, w: &str) -> bool {
        self.kind == Kind::Word && text[self.span.clone()].eq_ignore_ascii_case(w)
    }
}

fn word_char(c: char) -> bool {
    c.is_alphanumeric() || matches!(c, '_' | '$' | '#' | '@')
}

fn quoted_end(bytes: &[u8], start: usize, quote: u8, backslash: bool) -> usize {
    let mut at = start;
    while let Some(&byte) = bytes.get(at) {
        let escaped =
            (byte == b'\\' && backslash) || (byte == quote && bytes.get(at + 1) == Some(&quote));
        if escaped {
            at += 2;
        } else if byte == quote {
            break;
        } else {
            at += 1;
        }
    }
    at.min(bytes.len())
}

fn tokens(text: &str, dialect: Option<Dialect>) -> Vec<Tok> {
    let backslash = matches!(dialect, None | Some(Dialect::Mysql | Dialect::Clickhouse));
    let dollar = matches!(dialect, Some(Dialect::Postgres | Dialect::Duckdb));
    let bytes = text.as_bytes();
    let mut out = Vec::new();
    let mut at = 0;
    while at < bytes.len() {
        let rest = &text[at..];
        let Some(ch) = rest.chars().next() else { break };
        if ch.is_whitespace() {
            at += ch.len_utf8();
        } else if rest.starts_with("--")
            || (ch == '#' && dialect == Some(Dialect::Mysql))
            || (rest.starts_with("//") && dialect.is_none())
        {
            at += rest.find('\n').unwrap_or(rest.len());
        } else if let Some(body) = rest.strip_prefix("/*") {
            at += body.find("*/").map_or(rest.len(), |end| end + 4);
        } else if matches!(ch, '\'' | '"' | '`') {
            let start = at + 1;
            let end = quoted_end(bytes, start, bytes[at], backslash);
            out.push(Tok {
                kind: Kind::Str,
                span: start..end,
            });
            at = (end + 1).min(bytes.len());
        } else if ch == '$'
            && dollar
            && let Some(tag_end) = rest[1..].find('$')
        {
            let tag = &rest[..tag_end + 2];
            if tag[1..tag.len() - 1].chars().all(word_char) {
                let start = at + tag.len();
                let end = text[start..].find(tag).map_or(text.len(), |e| start + e);
                out.push(Tok {
                    kind: Kind::Str,
                    span: start..end,
                });
                at = (end + tag.len()).min(bytes.len());
            } else {
                out.push(Tok {
                    kind: Kind::Punct('$'),
                    span: at..at + 1,
                });
                at += 1;
            }
        } else if matches!(ch, 'N' | 'n' | 'E' | 'e' | 'X' | 'x') && rest[1..].starts_with('\'') {
            at += 1;
        } else if word_char(ch) {
            let len = rest.find(|x: char| !word_char(x)).unwrap_or(rest.len());
            out.push(Tok {
                kind: Kind::Word,
                span: at..at + len,
            });
            at += len;
        } else {
            out.push(Tok {
                kind: Kind::Punct(ch),
                span: at..at + ch.len_utf8(),
            });
            at += ch.len_utf8();
        }
    }
    out
}

fn sql_spans(text: &str, dialect: Dialect) -> Vec<Range<usize>> {
    let toks = tokens(text, Some(dialect));
    let mut out = Vec::new();
    let mut stmt_start = 0;
    for (i, t) in toks.iter().enumerate() {
        if t.kind == Kind::Punct(';') {
            stmt_start = i + 1;
            continue;
        }
        let stmt = &toks[stmt_start..i];
        let about_accounts = stmt
            .iter()
            .take(4)
            .any(|x| ["USER", "ROLE", "LOGIN"].iter().any(|w| x.is_word(text, w)));
        let word = |w: &str| t.is_word(text, w);
        if word("PASSWORD") || word("OLD_PASSWORD") {
            if toks.get(i + 1).is_some_and(|n| n.is_word(text, "FOR")) {
                if let Some(eq) = toks[i..]
                    .iter()
                    .position(|x| matches!(x.kind, Kind::Punct('=' | ';')))
                    .map(|p| i + p)
                    .filter(|&p| toks[p].kind == Kind::Punct('='))
                {
                    push_value(text, &toks, eq + 1, false, &mut out);
                }
            } else {
                push_value(text, &toks, i + 1, about_accounts, &mut out);
            }
        } else if word("IDENTIFIED") {
            let mut j = i + 1;
            if toks.get(j).is_some_and(|n| n.is_word(text, "WITH")) {
                j += 2;
            }
            if toks
                .get(j)
                .is_some_and(|n| n.is_word(text, "BY") || n.is_word(text, "AS"))
            {
                let after = push_value(text, &toks, j + 1, true, &mut out);
                if let Some(k) = after
                    && toks.get(k).is_some_and(|n| n.is_word(text, "REPLACE"))
                {
                    push_value(text, &toks, k + 1, true, &mut out);
                }
            }
        } else if word("USING") && i >= 2 && toks[i - 2].is_word(text, "USER") {
            push_value(text, &toks, i + 1, true, &mut out);
        }
    }
    out
}

fn push_value(
    text: &str,
    toks: &[Tok],
    mut at: usize,
    bare: bool,
    out: &mut Vec<Range<usize>>,
) -> Option<usize> {
    if toks.get(at).is_some_and(|t| t.kind == Kind::Punct('=')) {
        at += 1;
    }
    if toks.get(at).is_some_and(|t| t.kind == Kind::Punct('(')) {
        at += 1;
    }
    let t = toks.get(at)?;
    match t.kind {
        Kind::Str if !t.span.is_empty() => out.push(t.span.clone()),
        Kind::Word
            if bare
                && !NOT_A_PASSWORD
                    .iter()
                    .any(|w| text[t.span.clone()].eq_ignore_ascii_case(w)) =>
        {
            out.push(t.span.clone());
        }
        _ => return None,
    }
    Some(at + 1)
}

fn redis_args(line: &str, base: usize) -> Vec<Range<usize>> {
    let b = line.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < b.len() {
        if b[i].is_ascii_whitespace() {
            i += 1;
            continue;
        }
        if b[i] == b'"' || b[i] == b'\'' {
            let q = b[i];
            let start = i + 1;
            let mut j = start;
            while j < b.len() && b[j] != q {
                j += if b[j] == b'\\' { 2 } else { 1 };
            }
            let end = j.min(b.len());
            out.push(base + start..base + end);
            i = end + 1;
        } else {
            let start = i;
            while i < b.len() && !b[i].is_ascii_whitespace() {
                i += 1;
            }
            out.push(base + start..base + i);
        }
    }
    out
}

fn redis_spans(text: &str) -> Vec<Range<usize>> {
    let mut out = Vec::new();
    let mut base = 0;
    for line in text.split_inclusive('\n') {
        let args = redis_args(line, base);
        base += line.len();
        let word = |k: usize, w: &str| {
            args.get(k)
                .is_some_and(|r| text[r.clone()].eq_ignore_ascii_case(w))
        };
        let mut mask = |k: usize| {
            if let Some(r) = args.get(k).filter(|r| !r.is_empty()) {
                out.push(r.clone());
            }
        };
        if word(0, "AUTH") && args.len() > 1 {
            mask(args.len() - 1);
        } else if word(0, "HELLO") || word(0, "MIGRATE") {
            for k in 1..args.len() {
                if word(k, "AUTH") {
                    mask(k + if word(0, "HELLO") { 2 } else { 1 });
                } else if word(k, "AUTH2") {
                    mask(k + 2);
                }
            }
        } else if word(0, "CONFIG") && word(1, "SET") {
            for k in (2..args.len()).step_by(2) {
                if word(k, "requirepass") || word(k, "masterauth") {
                    mask(k + 1);
                }
            }
        } else if word(0, "ACL") && word(1, "SETUSER") {
            for r in args.iter().skip(3) {
                if text[r.clone()].starts_with(['>', '<']) && r.len() > 1 {
                    out.push(r.start + 1..r.end);
                }
            }
        }
    }
    out
}

fn mongo_spans(text: &str) -> Vec<Range<usize>> {
    let toks = tokens(text, None);
    let mut out = Vec::new();
    for (i, t) in toks.iter().enumerate() {
        let key = match t.kind {
            Kind::Word | Kind::Str => &text[t.span.clone()],
            Kind::Punct(_) => continue,
        };
        if key == "pwd"
            && toks.get(i + 1).is_some_and(|n| n.kind == Kind::Punct(':'))
            && let Some(v) = toks
                .get(i + 2)
                .filter(|v| v.kind == Kind::Str && !v.span.is_empty())
        {
            out.push(v.span.clone());
        }
    }
    out
}
