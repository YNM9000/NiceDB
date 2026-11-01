use std::ops::Range;

use nd_driver::{CommandClass, Risk};

pub(crate) const READ_CALLS: [&str; 10] = [
    "Scan",
    "Query",
    "GetItem",
    "BatchGetItem",
    "DescribeTable",
    "ListTables",
    "DescribeTimeToLive",
    "DescribeContinuousBackups",
    "ListTagsOfResource",
    "TransactGetItems",
];

pub(crate) fn commands(text: &str) -> Vec<Range<usize>> {
    let b = text.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < b.len() {
        if b[i].is_ascii_whitespace() || b[i] == b';' {
            i += 1;
            continue;
        }
        if b[i..].starts_with(b"--") || b[i..].starts_with(b"//") {
            while i < b.len() && b[i] != b'\n' {
                i += 1;
            }
            continue;
        }
        let start = i;
        let json = b[i] == b'{';
        let mut depth = 0i32;
        let mut quote: Option<u8> = None;
        let mut end = b.len();
        while i < b.len() {
            let c = b[i];
            if let Some(q) = quote {
                if c == b'\\' && q == b'"' && json {
                    i += 2;
                    continue;
                }
                if c == q {
                    if !json && b.get(i + 1) == Some(&q) {
                        i += 2;
                        continue;
                    }
                    quote = None;
                }
                i += 1;
                continue;
            }
            match c {
                b'\'' | b'"' => quote = Some(c),
                b'{' | b'[' => depth += 1,
                b'}' | b']' => {
                    depth -= 1;
                    if json && depth == 0 {
                        end = i + 1;
                        i += 1;
                        break;
                    }
                }
                b';' if !json && depth <= 0 => {
                    end = i;
                    i += 1;
                    break;
                }
                b'-' if !json && b.get(i + 1) == Some(&b'-') => {
                    while i < b.len() && b[i] != b'\n' {
                        i += 1;
                    }
                    continue;
                }
                _ => {}
            }
            i += 1;
        }
        let body = text[start..end.min(b.len())].trim_end();
        if !body.is_empty() {
            out.push(start..start + body.len());
        }
    }
    out
}

pub(crate) fn call(command: &str) -> Option<(String, serde_json::Map<String, serde_json::Value>)> {
    let v: serde_json::Value = serde_json::from_str(command.trim()).ok()?;
    let o = v.as_object()?;
    if o.len() != 1 {
        return None;
    }
    let (op, req) = o.iter().next()?;
    Some((op.clone(), req.as_object().cloned().unwrap_or_default()))
}

pub(crate) fn classify(command: &str) -> CommandClass {
    let text = command.trim_start();
    if text.starts_with('{') {
        let Some((op, _)) = call(text) else {
            return CommandClass {
                write: true,
                risk: None,
            };
        };
        let risk = match op.as_str() {
            "DeleteTable" => Some(Risk::Drop),
            "UpdateTable" => Some(Risk::Alter),
            _ => None,
        };
        return CommandClass {
            write: !READ_CALLS.contains(&op.as_str()),
            risk,
        };
    }
    let word = text.split_whitespace().next().unwrap_or_default();
    CommandClass {
        write: !word.eq_ignore_ascii_case("SELECT"),
        risk: None,
    }
}
