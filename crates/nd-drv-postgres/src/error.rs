use nd_driver::DriverError;
use tokio_postgres::error::{ErrorPosition, SqlState};

pub fn map_error(e: &tokio_postgres::Error, sql: &str) -> DriverError {
    let Some(db) = e.as_db_error() else {
        return if e.is_closed() {
            DriverError::Io {
                message: e.to_string(),
            }
        } else {
            DriverError::query(e.to_string())
        };
    };
    if *db.code() == SqlState::QUERY_CANCELED {
        return DriverError::Cancelled;
    }
    let offset = match db.position() {
        Some(ErrorPosition::Original(chars)) => {
            let idx = usize::try_from(*chars).unwrap_or(1).saturating_sub(1);
            sql.char_indices()
                .nth(idx)
                .map(|(b, _)| u32::try_from(b).unwrap_or(u32::MAX))
        }
        _ => None,
    };
    DriverError::Query {
        message: db.message().to_owned(),
        code: Some(db.code().code().to_owned()),
        detail: db.detail().map(str::to_owned),
        hint: db.hint().map(str::to_owned),
        offset,
    }
}

pub(crate) fn map_connect(e: tokio_postgres::Error) -> DriverError {
    if let Some(db) = e.as_db_error() {
        let code = db.code();
        let message = db.message().to_owned();
        return if *code == SqlState::INVALID_PASSWORD
            || *code == SqlState::INVALID_AUTHORIZATION_SPECIFICATION
        {
            DriverError::Auth { message }
        } else {
            DriverError::Connect { message }
        };
    }
    let text = e.to_string();
    let source = std::error::Error::source(&e)
        .map(ToString::to_string)
        .unwrap_or_default();
    let message = if source.is_empty() {
        text
    } else {
        format!("{text}: {source}")
    };
    if message.contains("tls") || message.contains("certificate") || message.contains("TLS") {
        DriverError::Tls { message }
    } else {
        DriverError::Connect { message }
    }
}
