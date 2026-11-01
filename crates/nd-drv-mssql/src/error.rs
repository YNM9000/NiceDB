use nd_driver::DriverError;
use tiberius::error::Error;

const LOGIN_FAILED: u32 = 18456;

pub fn map_error(e: &Error, sql: &str) -> DriverError {
    match e {
        Error::Server(t) => DriverError::Query {
            message: t.message().to_owned(),
            code: Some(t.code().to_string()),
            detail: Some(format!(
                "Msg {}, Level {}, State {}, Line {}",
                t.code(),
                t.class(),
                t.state(),
                t.line()
            )),
            hint: None,
            offset: offset(t.message(), t.line(), sql),
        },
        Error::Io { .. } => DriverError::Io {
            message: e.to_string(),
        },
        _ => DriverError::query(e.to_string()),
    }
}

pub(crate) fn map_connect(e: &Error) -> DriverError {
    match e {
        Error::Server(t) if t.code() == LOGIN_FAILED => DriverError::Auth {
            message: t.message().to_owned(),
        },
        Error::Server(t) => DriverError::Connect {
            message: t.message().to_owned(),
        },
        Error::Tls(m) => DriverError::Tls { message: m.clone() },
        _ => {
            let message = e.to_string();
            if ["TLS", "tls", "certificate", "Certificate"]
                .iter()
                .any(|w| message.contains(w))
            {
                DriverError::Tls { message }
            } else {
                DriverError::Connect { message }
            }
        }
    }
}

fn offset(message: &str, line: u32, sql: &str) -> Option<u32> {
    let quoted = |q: char| {
        let end = message.rfind(q)?;
        let start = message[..end].rfind(q)?;
        Some(&message[start + 1..end])
    };
    let token = quoted('\'').or_else(|| quoted('"'))?;
    if token.is_empty() {
        return None;
    }
    let line = usize::try_from(line).ok()?.max(1);
    let line_start: usize = sql.split_inclusive('\n').take(line - 1).map(str::len).sum();
    let text = sql.get(line_start..)?;
    let text = &text[..text.find('\n').unwrap_or(text.len())];
    let at = find_word(text, token)?;
    u32::try_from(line_start + at).ok()
}

fn find_word(text: &str, token: &str) -> Option<usize> {
    let lower = text.to_lowercase();
    let needle = token.to_lowercase();
    if lower.len() != text.len() {
        return text.find(token);
    }
    let word = |c: Option<char>| c.is_some_and(|c| c.is_alphanumeric() || c == '_');
    let mut first = None;
    for (i, _) in lower.match_indices(&needle) {
        first.get_or_insert(i);
        let before = lower[..i].chars().next_back();
        let after = lower[i + needle.len()..].chars().next();
        if !word(before) && !word(after) {
            return Some(i);
        }
    }
    first
}
