use nd_driver::{DriverError, Row};
use serde_json::Value;

pub(crate) fn values(line: &str) -> Option<Vec<Value>> {
    match serde_json::from_str(line) {
        Ok(Value::Array(items)) => Some(items),
        _ => None,
    }
}

pub(crate) fn texts(line: &str) -> Option<Vec<String>> {
    values(line)?
        .into_iter()
        .map(|v| match v {
            Value::String(s) => Some(s),
            _ => None,
        })
        .collect()
}

pub(crate) fn cell(v: Value) -> Option<String> {
    match v {
        Value::Null => None,
        Value::String(s) => Some(s),
        Value::Bool(b) => Some(b.to_string()),
        Value::Number(n) => Some(n.to_string()),
        other => Some(other.to_string()),
    }
}

pub(crate) fn row(line: &str) -> Option<Row> {
    Some(values(line)?.into_iter().map(cell).collect())
}

pub(crate) fn server_error(text: &str) -> Option<DriverError> {
    let start = text.find("Code: ")?;
    let text = text[start..].trim();
    let code: u32 = text["Code: ".len()..]
        .split(|c: char| !c.is_ascii_digit())
        .next()?
        .parse()
        .ok()?;
    let body = text.split_once("DB::Exception: ").map_or(text, |(_, b)| b);
    let body = body.split(" (version ").next().unwrap_or(body).trim_end();
    let (message, name) = match body.rsplit_once(" (") {
        Some((m, n))
            if n.ends_with(')')
                && n[..n.len() - 1]
                    .chars()
                    .all(|c| c.is_ascii_uppercase() || c == '_') =>
        {
            (
                m.trim_end_matches('.').trim(),
                Some(n[..n.len() - 1].to_owned()),
            )
        }
        _ => (body.trim_end_matches('.').trim(), None),
    };
    let message = message.to_owned();
    Some(match code {
        192..=194 | 516 => DriverError::Auth { message },
        394 => DriverError::Cancelled,
        _ => DriverError::Query {
            offset: position(&message),
            message,
            code: name.or_else(|| Some(code.to_string())),
            detail: None,
            hint: None,
        },
    })
}

fn position(message: &str) -> Option<u32> {
    let rest = message.split_once("failed at position ")?.1;
    let n: u32 = rest
        .split(|c: char| !c.is_ascii_digit())
        .next()?
        .parse()
        .ok()?;
    n.checked_sub(1)
}

pub(crate) fn summary(header: &str, key: &str) -> Option<u64> {
    let v: Value = serde_json::from_str(header).ok()?;
    match v.get(key)? {
        Value::String(s) => s.parse().ok(),
        Value::Number(n) => n.as_u64(),
        _ => None,
    }
}
