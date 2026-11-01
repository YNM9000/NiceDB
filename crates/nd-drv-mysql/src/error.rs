use mysql_async::{Error, IoError};
use nd_driver::DriverError;

const QUERY_INTERRUPTED: u16 = 1317;
const AUTH_ERRORS: &[u16] = &[1044, 1045, 1698];

pub fn map_error(e: &Error, sql: &str) -> DriverError {
    match e {
        Error::Server(s) if s.code == QUERY_INTERRUPTED => DriverError::Cancelled,
        Error::Server(s) => DriverError::Query {
            message: s.message.clone(),
            code: Some(s.state.clone()),
            detail: Some(format!("error {}", s.code)),
            hint: None,
            offset: near_offset(&s.message, sql),
        },
        Error::Io(_) | Error::Driver(mysql_async::DriverError::ConnectionClosed) => {
            DriverError::Io {
                message: e.to_string(),
            }
        }
        _ => DriverError::query(e.to_string()),
    }
}

pub(crate) fn map_connect(e: &Error) -> DriverError {
    match e {
        Error::Server(s) if AUTH_ERRORS.contains(&s.code) => DriverError::Auth {
            message: s.message.clone(),
        },
        Error::Server(s) => DriverError::Connect {
            message: s.message.clone(),
        },
        Error::Io(IoError::Tls(t)) => DriverError::Tls {
            message: t.to_string(),
        },
        _ => {
            let message = e.to_string();
            if ["TLS", "SSL", "tls", "certificate"]
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

fn near_offset(message: &str, sql: &str) -> Option<u32> {
    let rest = &message[message.find(" near '")? + 7..];
    let end = rest.rfind("' at line ")?;
    let near = &rest[..end];
    let line: usize = rest[end + 10..].trim().parse().ok()?;
    let line_start: usize = sql
        .split_inclusive('\n')
        .take(line.saturating_sub(1))
        .map(str::len)
        .sum();
    let at = if near.is_empty() {
        sql.trim_end().len()
    } else {
        line_start + sql.get(line_start..)?.find(near)?
    };
    u32::try_from(at).ok()
}
