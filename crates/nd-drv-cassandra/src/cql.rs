use std::ops::Range;

use nd_driver::{CommandClass, Risk};

const READS: [&str; 7] = [
    "SELECT",
    "DESCRIBE",
    "DESC",
    "USE",
    "CONSISTENCY",
    "LIST",
    "TRACING",
];

struct Word {
    upper: String,
}

pub(crate) fn commands(text: &str) -> Vec<Range<usize>> {
    let b = text.as_bytes();
    let mut out = Vec::new();
    let mut start: Option<usize> = None;
    let mut words: Vec<Word> = Vec::new();
    let mut i = 0;
    let finish = |out: &mut Vec<Range<usize>>, s: usize, e: usize| {
        let body = text[s..e].trim_end();
        if !body.is_empty() {
            out.push(s..s + body.len());
        }
    };
    while i < b.len() {
        let c = b[i];
        if b[i..].starts_with(b"--") || b[i..].starts_with(b"//") {
            while i < b.len() && b[i] != b'\n' {
                i += 1;
            }
            continue;
        }
        if b[i..].starts_with(b"/*") {
            i = text[i + 2..].find("*/").map_or(b.len(), |e| i + 2 + e + 2);
            continue;
        }
        if c.is_ascii_whitespace() {
            i += 1;
            continue;
        }
        let at = i;
        if start.is_none() && c != b';' {
            start = Some(at);
        }
        match c {
            b'\'' | b'"' => {
                i += 1;
                while i < b.len() {
                    if b[i] == c {
                        if b.get(i + 1) == Some(&c) {
                            i += 2;
                            continue;
                        }
                        break;
                    }
                    i += 1;
                }
                i += 1;
            }
            b'$' if b.get(i + 1) == Some(&b'$') => {
                i = text[i + 2..].find("$$").map_or(b.len(), |e| i + 2 + e + 2);
            }
            b';' => {
                let in_batch = words.first().is_some_and(|w| w.upper == "BEGIN")
                    && !words
                        .windows(2)
                        .any(|w| w[0].upper == "APPLY" && w[1].upper == "BATCH");
                if !in_batch {
                    if let Some(s) = start.take() {
                        finish(&mut out, s, at);
                    }
                    words.clear();
                }
                i += 1;
            }
            c if c.is_ascii_alphanumeric() || c == b'_' => {
                while i < b.len() && (b[i].is_ascii_alphanumeric() || b[i] == b'_') {
                    i += 1;
                }
                if words.len() < 4096 {
                    words.push(Word {
                        upper: text[at..i].to_ascii_uppercase(),
                    });
                }
            }
            _ => i += 1,
        }
    }
    if let Some(s) = start {
        finish(&mut out, s, b.len());
    }
    out
}

fn first_word(command: &str) -> String {
    let ranges = commands(command);
    let body = ranges.first().map_or(command, |r| &command[r.clone()]);
    body.split(|c: char| !c.is_ascii_alphanumeric() && c != '_')
        .find(|w| !w.is_empty())
        .unwrap_or_default()
        .to_ascii_uppercase()
}

pub(crate) fn classify(command: &str) -> CommandClass {
    let word = first_word(command);
    let risk = match word.as_str() {
        "DROP" => Some(Risk::Drop),
        "TRUNCATE" => Some(Risk::Truncate),
        "ALTER" => Some(Risk::Alter),
        _ => None,
    };
    CommandClass {
        write: !READS.contains(&word.as_str()),
        risk,
    }
}

pub(crate) fn consistency_command(command: &str) -> Option<&str> {
    let mut parts = command.trim().trim_end_matches(';').split_whitespace();
    let head = parts.next()?;
    if !head.eq_ignore_ascii_case("CONSISTENCY") {
        return None;
    }
    Some(parts.next().unwrap_or_default())
}

pub(crate) fn quote(name: &str) -> String {
    nd_sql::name(nd_sql::Dialect::Cql, name)
}

pub(crate) fn literal(s: &str) -> String {
    nd_sql::literal(nd_sql::Dialect::Cql, Some(s))
}

pub(crate) fn json_row(doc: serde_json::Map<String, serde_json::Value>) -> String {
    let key = |k: String| {
        if k.to_lowercase() == k {
            k
        } else {
            format!("\"{k}\"")
        }
    };
    serde_json::Value::Object(doc.into_iter().map(|(k, v)| (key(k), v)).collect()).to_string()
}
