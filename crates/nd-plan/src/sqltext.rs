use crate::model::Plan;

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Tok {
    Word(String),
    Quoted(String),
    Num(String),
    Str(String),
    Punct(String),
}

impl Tok {
    pub(crate) fn word(&self) -> Option<&str> {
        match self {
            Self::Word(w) | Self::Quoted(w) => Some(w),
            _ => None,
        }
    }

    pub(crate) fn is_kw(&self, kw: &str) -> bool {
        matches!(self, Self::Word(w) if w.eq_ignore_ascii_case(kw))
    }

    pub(crate) fn is(&self, p: &str) -> bool {
        matches!(self, Self::Punct(x) if x == p)
    }
}

const OPERATORS: &[&str] = &[
    "::", "<>", "!=", "<=", ">=", "->>", "->", "~~", "||", "#>>", "#>", "@>", "<@",
];

pub(crate) fn tokens(s: &str) -> Vec<Tok> {
    let b = s.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    let until = |from: usize, end: u8| -> usize {
        let mut j = from;
        while j < b.len() {
            if b[j] == end {
                if b.get(j + 1) == Some(&end) {
                    j += 2;
                    continue;
                }
                return j;
            }
            j += 1;
        }
        b.len()
    };
    while i < b.len() {
        let c = b[i];
        if c.is_ascii_whitespace() {
            i += 1;
        } else if c == b'-' && b.get(i + 1) == Some(&b'-')
            || c == b'#' && b.get(i + 1) == Some(&b' ')
        {
            while i < b.len() && b[i] != b'\n' {
                i += 1;
            }
        } else if c == b'/' && b.get(i + 1) == Some(&b'*') {
            i = s[i + 2..].find("*/").map_or(b.len(), |p| i + 2 + p + 2);
        } else if c == b'\'' {
            let end = until(i + 1, b'\'');
            out.push(Tok::Str(s.get(i..=end).unwrap_or(&s[i..]).to_owned()));
            i = end + 1;
        } else if c == b'"' || c == b'`' {
            let end = until(i + 1, c);
            let inner = s.get(i + 1..end).unwrap_or_default();
            out.push(Tok::Quoted(inner.to_owned()));
            i = end + 1;
        } else if c.is_ascii_digit() || c == b'.' && b.get(i + 1).is_some_and(u8::is_ascii_digit) {
            let start = i;
            while i < b.len()
                && (b[i].is_ascii_alphanumeric()
                    || b[i] == b'.'
                    || (matches!(b[i], b'+' | b'-') && matches!(b[i - 1], b'e' | b'E')))
            {
                i += 1;
            }
            out.push(Tok::Num(s[start..i].to_owned()));
        } else if c.is_ascii_alphabetic() || c == b'_' || c >= 0x80 {
            let start = i;
            while i < b.len()
                && (b[i].is_ascii_alphanumeric() || b[i] == b'_' || b[i] == b'$' || b[i] >= 0x80)
            {
                i += 1;
            }
            out.push(Tok::Word(s[start..i].to_owned()));
        } else {
            let op = OPERATORS.iter().find(|op| s[i..].starts_with(**op));
            let len = op.map_or(1, |op| op.len());
            let end = (i + len).min(b.len());
            let mut end = end;
            while !s.is_char_boundary(end) {
                end += 1;
            }
            out.push(Tok::Punct(s[i..end].to_owned()));
            i = end;
        }
    }
    out
}

const EXPLAIN_WORDS: &[&str] = &[
    "ANALYZE",
    "ANALYSE",
    "VERBOSE",
    "QUERY",
    "PLAN",
    "EXTENDED",
    "PARTITIONS",
    "FORMAT",
    "TREE",
    "JSON",
    "TRADITIONAL",
    "XML",
    "TEXT",
    "YAML",
];

pub fn strip_explain(sql: &str) -> &str {
    let mut rest = sql.trim_start();
    let first = rest
        .split(|c: char| !c.is_ascii_alphabetic())
        .next()
        .unwrap_or("");
    if !["EXPLAIN", "ANALYZE", "DESCRIBE", "DESC"]
        .iter()
        .any(|k| first.eq_ignore_ascii_case(k))
    {
        return sql;
    }
    rest = rest[first.len()..].trim_start();
    if first.eq_ignore_ascii_case("ANALYZE") {
        let next = rest
            .split(|c: char| !c.is_ascii_alphabetic())
            .next()
            .unwrap_or("");
        let statement = [
            "SELECT", "WITH", "UPDATE", "DELETE", "INSERT", "REPLACE", "FORMAT", "TABLE",
        ];
        if !statement.iter().any(|k| next.eq_ignore_ascii_case(k)) {
            return sql;
        }
    }
    loop {
        if rest.starts_with('(') {
            let mut depth = 0usize;
            let mut end = rest.len();
            for (i, c) in rest.char_indices() {
                match c {
                    '(' => depth += 1,
                    ')' => {
                        depth = depth.saturating_sub(1);
                        if depth == 0 {
                            end = i + 1;
                            break;
                        }
                    }
                    _ => {}
                }
            }
            rest = rest[end..].trim_start();
            continue;
        }
        if let Some(r) = rest.strip_prefix('=') {
            rest = r.trim_start();
            continue;
        }
        let word = rest
            .split(|c: char| !c.is_ascii_alphabetic())
            .next()
            .unwrap_or("");
        if !word.is_empty() && EXPLAIN_WORDS.iter().any(|k| word.eq_ignore_ascii_case(k)) {
            rest = rest[word.len()..].trim_start();
            continue;
        }
        return rest;
    }
}

const NOT_ALIAS: &[&str] = &[
    "ON",
    "USING",
    "WHERE",
    "JOIN",
    "INNER",
    "LEFT",
    "RIGHT",
    "FULL",
    "CROSS",
    "NATURAL",
    "GROUP",
    "ORDER",
    "LIMIT",
    "HAVING",
    "WINDOW",
    "UNION",
    "EXCEPT",
    "INTERSECT",
    "LATERAL",
    "SET",
    "OUTER",
    "FOR",
    "OFFSET",
    "FETCH",
    "STRAIGHT_JOIN",
    "USE",
    "FORCE",
    "IGNORE",
    "VALUES",
    "RETURNING",
    "AND",
    "OR",
    "SELECT",
    "WITH",
];

fn not_alias(w: &str) -> bool {
    NOT_ALIAS.iter().any(|k| w.eq_ignore_ascii_case(k))
}

pub(crate) fn aliases(sql: &str) -> Vec<(String, String)> {
    let toks = tokens(sql);
    let mut out = Vec::new();
    let mut in_from = false;
    let mut i = 0;
    while i < toks.len() {
        let starts = toks[i].is_kw("FROM")
            || toks[i].is_kw("JOIN")
            || toks[i].is_kw("UPDATE")
            || toks[i].is_kw("INTO")
            || (in_from && toks[i].is(","));
        if toks[i].is_kw("WHERE")
            || toks[i].is_kw("GROUP")
            || toks[i].is_kw("ORDER")
            || toks[i].is(")")
        {
            in_from = false;
        }
        if toks[i].is_kw("FROM") {
            in_from = true;
        }
        if !starts {
            i += 1;
            continue;
        }
        let mut j = i + 1;
        while toks
            .get(j)
            .is_some_and(|x| x.is_kw("LATERAL") || x.is_kw("ONLY"))
        {
            j += 1;
        }
        let Some(mut table) = toks.get(j).and_then(Tok::word).map(str::to_owned) else {
            i += 1;
            continue;
        };
        if not_alias(&table) {
            i += 1;
            continue;
        }
        while toks.get(j + 1).is_some_and(|x| x.is(".")) {
            if let Some(w) = toks.get(j + 2).and_then(Tok::word) {
                w.clone_into(&mut table);
            }
            j += 2;
        }
        let mut k = j + 1;
        if toks.get(k).is_some_and(|x| x.is_kw("AS")) {
            k += 1;
        }
        let alias = match toks.get(k).and_then(Tok::word) {
            Some(a) if !not_alias(a) && !toks.get(k + 1).is_some_and(|x| x.is("(")) => a.to_owned(),
            _ => table.clone(),
        };
        out.push((alias, table));
        i = j + 1;
    }
    out
}

pub(crate) fn resolve<'a>(aliases: &'a [(String, String)], name: &'a str) -> &'a str {
    aliases
        .iter()
        .find(|(a, _)| a.eq_ignore_ascii_case(name))
        .map_or(name, |(_, t)| t.as_str())
}

fn top_words(sql: &str) -> Vec<Tok> {
    let mut depth = 0i32;
    let mut out = Vec::new();
    for tok in tokens(sql) {
        if tok.is("(") {
            depth += 1;
        } else if tok.is(")") {
            depth -= 1;
        } else if depth == 0 {
            out.push(tok);
        }
    }
    out
}

pub(crate) fn has_limit(sql: &str) -> bool {
    let w = top_words(sql);
    w.iter()
        .any(|t| t.is_kw("LIMIT") || t.is_kw("FETCH") || t.is_kw("TOP"))
}

pub(crate) fn is_query(sql: &str) -> bool {
    tokens(sql).first().is_some_and(|t| {
        t.is_kw("SELECT") || t.is_kw("WITH") || t.is_kw("TABLE") || t.is_kw("VALUES")
    })
}

pub(crate) fn selects_star(sql: &str) -> bool {
    let w = top_words(sql);
    let Some(start) = w.iter().position(|t| t.is_kw("SELECT")) else {
        return false;
    };
    w[start + 1..]
        .iter()
        .take_while(|t| !t.is_kw("FROM"))
        .any(|t| t.is("*"))
}

pub(crate) fn join_pairs(sql: &str) -> Vec<((String, String), (String, String))> {
    let t = tokens(sql);
    let col = |i: usize| -> Option<(String, String)> {
        let a = t.get(i)?.word()?;
        if !t.get(i + 1)?.is(".") {
            return None;
        }
        let c = t.get(i + 2)?.word()?;
        Some((a.to_owned(), c.to_owned()))
    };
    let mut out = Vec::new();
    for i in 0..t.len() {
        if !t[i].is("=") || i < 3 {
            continue;
        }
        if t.get(i.wrapping_sub(4)).is_some_and(|x| x.is(".")) {
            continue;
        }
        if let (Some(l), Some(r)) = (col(i - 3), col(i + 1))
            && !l.0.eq_ignore_ascii_case(&r.0)
        {
            out.push((l, r));
        }
    }
    out
}

pub(crate) fn literal_filters(sql: &str) -> Vec<(String, String)> {
    let t = tokens(sql);
    let Some(start) = t.iter().position(|x| x.is_kw("WHERE")) else {
        return Vec::new();
    };
    let constant = |x: Option<&Tok>| {
        matches!(x, Some(Tok::Num(_) | Tok::Str(_))) || x.is_some_and(|x| x.is("?") || x.is("$"))
    };
    let mut out = Vec::new();
    for i in start + 1..t.len() {
        if !(t[i].is("=") || t[i].is_kw("LIKE") || t[i].is_kw("IN")) {
            continue;
        }
        let list = t[i].is_kw("IN") && t.get(i + 1).is_some_and(|x| x.is("("));
        if !constant(t.get(i + 1)) && !list {
            continue;
        }
        let Some(column) = t.get(i - 1).and_then(Tok::word) else {
            continue;
        };
        let alias = if t.get(i.wrapping_sub(2)).is_some_and(|x| x.is(".")) {
            t.get(i.wrapping_sub(3))
                .and_then(Tok::word)
                .unwrap_or_default()
        } else {
            ""
        };
        out.push((alias.to_owned(), column.to_owned()));
    }
    out
}

pub fn annotate(plan: &mut Plan, sql: &str) {
    let map = aliases(sql);
    for node in &mut plan.nodes {
        let Some(rel) = node.relation.clone() else {
            continue;
        };
        let table = resolve(&map, &rel);
        if !table.eq_ignore_ascii_case(&rel) {
            node.alias = Some(rel.clone());
            node.relation = Some(table.to_owned());
        }
    }
}
