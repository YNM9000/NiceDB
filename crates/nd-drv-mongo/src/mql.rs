use std::ops::Range;

use mongodb::bson::{Bson, Document};
use nd_driver::{CommandClass, DriverError, Risk};

const READS: &[&str] = &[
    "find",
    "aggregate",
    "count",
    "distinct",
    "explain",
    "listCollections",
    "listIndexes",
    "listDatabases",
    "dbStats",
    "collStats",
    "buildInfo",
    "serverStatus",
    "hostInfo",
    "ping",
    "hello",
    "isMaster",
    "connectionStatus",
    "getParameter",
    "currentOp",
    "validate",
    "usersInfo",
    "rolesInfo",
];

pub(crate) const CURSORS: &[&str] = &["find", "aggregate", "listCollections", "listIndexes"];

pub fn split(text: &str) -> Vec<Range<usize>> {
    let b = text.as_bytes();
    let mut out = Vec::new();
    let (mut i, mut depth, mut start) = (0, 0usize, None);
    while i < b.len() {
        match b[i] {
            b'"' | b'\'' => i = string_end(b, i),
            b'/' if b.get(i + 1) == Some(&b'/') => {
                i = b[i..]
                    .iter()
                    .position(|c| *c == b'\n')
                    .map_or(b.len(), |p| i + p);
                continue;
            }
            b'{' | b'[' => {
                if depth == 0 {
                    start = Some(i);
                }
                depth += 1;
            }
            b'}' | b']' if depth > 0 => {
                depth -= 1;
                if depth == 0
                    && let Some(s) = start.take()
                {
                    out.push(s..i + 1);
                }
            }
            c if depth == 0 && !c.is_ascii_whitespace() && !matches!(c, b',' | b';') => {
                let end = b[i..]
                    .iter()
                    .position(|c| matches!(c, b'{' | b'['))
                    .map_or(b.len(), |p| i + p);
                out.push(i..trim_end(text, i, end));
                i = end;
                continue;
            }
            _ => {}
        }
        i += 1;
    }
    if let Some(s) = start {
        out.push(s..trim_end(text, s, b.len()));
    }
    out
}

fn trim_end(text: &str, from: usize, to: usize) -> usize {
    from + text[from..to].trim_end().len()
}

fn string_end(bytes: &[u8], open: usize) -> usize {
    let quote = bytes[open];
    let mut at = open + 1;
    while at < bytes.len() {
        match bytes[at] {
            b'\\' => at += 1,
            ch if ch == quote => return at,
            _ => {}
        }
        at += 1;
    }
    bytes.len()
}

fn uncommented(text: &str) -> String {
    let b = text.as_bytes();
    let mut out = b.to_vec();
    let mut i = 0;
    while i < b.len() {
        match b[i] {
            b'"' => i = string_end(b, i),
            b'/' if b.get(i + 1) == Some(&b'/') => {
                while i < b.len() && b[i] != b'\n' {
                    out[i] = b' ';
                    i += 1;
                }
                continue;
            }
            _ => {}
        }
        i += 1;
    }
    String::from_utf8(out).unwrap_or_else(|_| text.to_owned())
}

pub fn parse(text: &str) -> Result<Document, DriverError> {
    let clean = uncommented(text);
    let value: serde_json::Value = serde_json::from_str(&clean).map_err(|e| {
        let offset = offset_of(&clean, e.line(), e.column());
        DriverError::Query {
            message: format!("not valid JSON: {e}"),
            code: None,
            detail: None,
            hint: Some(
                "write a command document, e.g. { \"find\": \"customers\", \"filter\": {} }".into(),
            ),
            offset: u32::try_from(offset).ok(),
        }
    })?;
    match Bson::try_from(value) {
        Ok(Bson::Document(d)) if !d.is_empty() => Ok(d),
        Ok(Bson::Document(_)) => Err(DriverError::query("the command document is empty")),
        Ok(_) => Err(DriverError::query(
            "a command is a JSON object whose first key names it",
        )),
        Err(e) => Err(DriverError::query(format!("not valid extended JSON: {e}"))),
    }
}

fn offset_of(text: &str, line: usize, column: usize) -> usize {
    let start: usize = text
        .split_inclusive('\n')
        .take(line.saturating_sub(1))
        .map(str::len)
        .sum();
    (start + column.saturating_sub(1)).min(text.len())
}

pub fn name(cmd: &Document) -> &str {
    cmd.keys().next().map_or("", String::as_str)
}

fn empty_filter(v: Option<&Bson>) -> bool {
    match v {
        None => true,
        Some(Bson::Document(d)) => d.is_empty(),
        Some(_) => false,
    }
}

fn writes_out(cmd: &Document) -> bool {
    cmd.get_array("pipeline").is_ok_and(|stages| {
        stages.iter().any(|s| {
            s.as_document()
                .is_some_and(|d| d.contains_key("$out") || d.contains_key("$merge"))
        })
    })
}

fn statements<'a>(cmd: &'a Document, key: &str) -> impl Iterator<Item = &'a Document> {
    cmd.get_array(key)
        .map(|a| a.iter().filter_map(Bson::as_document).collect::<Vec<_>>())
        .unwrap_or_default()
        .into_iter()
}

pub fn classify(text: &str) -> CommandClass {
    let Ok(cmd) = parse(text) else {
        return CommandClass {
            write: true,
            risk: None,
        };
    };
    let name = name(&cmd);
    let write = !READS.contains(&name) || (name == "aggregate" && writes_out(&cmd));
    let risk = match name {
        "delete" => statements(&cmd, "deletes")
            .any(|d| empty_filter(d.get("q")) && d.get_i32("limit").unwrap_or(0) == 0)
            .then_some(Risk::DeleteWithoutWhere),
        "update" => statements(&cmd, "updates")
            .any(|u| empty_filter(u.get("q")) && u.get_bool("multi").unwrap_or(false))
            .then_some(Risk::UpdateWithoutWhere),
        "drop" | "dropDatabase" | "dropIndexes" => Some(Risk::Drop),
        "collMod" | "renameCollection" => Some(Risk::Alter),
        _ => None,
    };
    CommandClass { write, risk }
}
