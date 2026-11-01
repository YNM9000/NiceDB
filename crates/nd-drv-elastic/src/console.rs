use std::ops::Range;

use nd_driver::{CommandClass, Risk};

const METHODS: [&str; 6] = ["GET", "POST", "PUT", "DELETE", "HEAD", "PATCH"];
const SQL_WORDS: [&str; 5] = ["SELECT", "SHOW", "DESCRIBE", "DESC", "EXPLAIN"];

const READS: [&str; 20] = [
    "_search",
    "_msearch",
    "_count",
    "_explain",
    "_validate",
    "_field_caps",
    "_mget",
    "_analyze",
    "_sql",
    "_ppl",
    "_render",
    "_termvectors",
    "_mtermvectors",
    "_search_shards",
    "_rank_eval",
    "_knn_search",
    "_async_search",
    "_pit",
    "_terms_enum",
    "scroll",
];

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Command<'a> {
    Http(Request<'a>),
    Sql(&'a str),
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) struct Request<'a> {
    pub method: String,
    pub path: String,
    pub body: Option<&'a str>,
}

impl Request<'_> {
    pub(crate) fn segments(&self) -> Vec<&str> {
        let path = self.path.split('?').next().unwrap_or_default();
        path.split('/').filter(|s| !s.is_empty()).collect()
    }

    pub(crate) fn is_search(&self) -> bool {
        self.segments().last() == Some(&"_search")
    }
}

fn method_of(line: &str) -> Option<&'static str> {
    let word = line.split_whitespace().next()?;
    METHODS
        .into_iter()
        .find(|m| m.eq_ignore_ascii_case(word))
        .filter(|_| line.split_whitespace().nth(1).is_some() || word.len() == line.trim().len())
}

fn sql_start(line: &str) -> bool {
    line.split_whitespace()
        .next()
        .is_some_and(|w| SQL_WORDS.iter().any(|s| s.eq_ignore_ascii_case(w)))
}

fn comment(line: &str) -> bool {
    let t = line.trim_start();
    t.starts_with('#') || t.starts_with("//")
}

fn depth_change(line: &str, in_string: &mut bool) -> i64 {
    let mut d = 0;
    let mut escaped = false;
    for c in line.chars() {
        if *in_string {
            match c {
                '\\' if !escaped => {
                    escaped = true;
                    continue;
                }
                '"' if !escaped => *in_string = false,
                _ => {}
            }
            escaped = false;
            continue;
        }
        match c {
            '"' => *in_string = true,
            '{' | '[' => d += 1,
            '}' | ']' => d -= 1,
            _ => {}
        }
    }
    d
}

pub(crate) fn commands(text: &str) -> Vec<Range<usize>> {
    let mut out: Vec<Range<usize>> = Vec::new();
    let mut open: Option<(usize, bool)> = None;
    let mut end = 0;
    let mut depth = 0i64;
    let mut in_string = false;
    let mut at = 0;
    let close = |out: &mut Vec<Range<usize>>, start: usize, end: usize| {
        let body = text[start..end].trim_end();
        let body = body.strip_suffix(';').map_or(body, str::trim_end);
        if !body.is_empty() {
            out.push(start..start + body.len());
        }
    };
    for line in text.split_inclusive('\n') {
        let start = at;
        at += line.len();
        let trimmed = line.trim();
        let lead = start + (line.len() - line.trim_start().len());
        if depth <= 0 && !in_string {
            if method_of(trimmed).is_some() || sql_start(trimmed) {
                if let Some((s, _)) = open.take() {
                    close(&mut out, s, end);
                }
                open = Some((lead, sql_start(trimmed)));
                depth = 0;
            } else if open.is_none() && (trimmed.is_empty() || comment(trimmed)) {
                continue;
            }
        }
        let Some((s, sql)) = open else {
            continue;
        };
        if !trimmed.is_empty() && (depth > 0 || !comment(trimmed)) {
            end = start + line.trim_end().len();
            if !sql {
                depth += depth_change(line, &mut in_string);
            }
        }
        if sql && trimmed.ends_with(';') {
            close(&mut out, s, end);
            open = None;
        }
    }
    if let Some((s, _)) = open {
        close(&mut out, s, end.max(s));
    }
    out
}

pub(crate) fn parse(command: &str) -> Option<Command<'_>> {
    let command = command.trim();
    let (head, rest) = command.split_once('\n').unwrap_or((command, ""));
    if let Some(method) = method_of(head.trim()) {
        let path = head.trim()[method.len()..].trim();
        let path = if path.is_empty() { "/" } else { path };
        let body = rest.trim();
        return Some(Command::Http(Request {
            method: method.to_owned(),
            path: path.to_owned(),
            body: Some(body).filter(|b| !b.is_empty()),
        }));
    }
    sql_start(head.trim()).then(|| Command::Sql(command.trim_end_matches(';').trim()))
}

fn class(write: bool, risk: Option<Risk>) -> CommandClass {
    CommandClass { write, risk }
}

fn matches_all(body: Option<&str>) -> bool {
    let Some(body) = body else {
        return true;
    };
    let Ok(v) = serde_json::from_str::<serde_json::Value>(body) else {
        return false;
    };
    match v.get("query") {
        None => true,
        Some(q) => q.get("match_all").is_some(),
    }
}

pub(crate) fn classify(command: &str) -> CommandClass {
    match parse(command) {
        Some(Command::Sql(sql)) => {
            let word = sql.split_whitespace().next().unwrap_or_default();
            class(
                !SQL_WORDS.iter().any(|w| w.eq_ignore_ascii_case(word)),
                None,
            )
        }
        Some(Command::Http(r)) => {
            let segs = r.segments();
            let endpoint = segs
                .iter()
                .rev()
                .find(|s| s.starts_with('_') || **s == "scroll");
            let last = segs.last().copied().unwrap_or_default();
            match r.method.as_str() {
                "GET" | "HEAD" => class(false, None),
                "DELETE"
                    if !segs.contains(&"_doc")
                        && !segs.iter().any(|s| s.starts_with("_search")) =>
                {
                    class(true, Some(Risk::Drop))
                }
                "POST" if last == "_delete_by_query" => class(
                    true,
                    matches_all(r.body).then_some(Risk::DeleteWithoutWhere),
                ),
                "POST" if last == "_update_by_query" => class(
                    true,
                    matches_all(r.body).then_some(Risk::UpdateWithoutWhere),
                ),
                "POST" if endpoint.is_some_and(|e| READS.contains(e)) => class(false, None),
                "PUT" if segs.first() == Some(&"_cluster") => class(true, Some(Risk::Server)),
                "POST" if matches!(last, "_close" | "_shrink" | "_split" | "_rollover") => {
                    class(true, Some(Risk::Alter))
                }
                _ => class(true, None),
            }
        }
        None => class(true, None),
    }
}
