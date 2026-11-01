use serde::{Deserialize, Serialize};

use crate::risk::{verb, words};
use crate::{Dialect, is_read, split};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
pub struct SortKey {
    pub column: u32,
    pub descending: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "snake_case")]
pub enum Unsortable {
    NotSelect,
    Writes,
    Multiple,
}

pub(crate) fn single(sql: &str, dialect: Dialect) -> Result<&str, Unsortable> {
    let statements = split(sql, dialect);
    let [st] = statements.as_slice() else {
        return Err(if statements.is_empty() {
            Unsortable::NotSelect
        } else {
            Unsortable::Multiple
        });
    };
    let text = st.text(sql);
    if verb(&words(text, dialect, false)) != Some("SELECT") {
        return Err(Unsortable::NotSelect);
    }
    if !is_read(text, dialect) {
        return Err(Unsortable::Writes);
    }
    Ok(text)
}

pub fn sortable(sql: &str, dialect: Dialect) -> Result<(), Unsortable> {
    single(sql, dialect).map(|_| ())
}

pub fn sorted(sql: &str, dialect: Dialect, keys: &[SortKey]) -> Result<String, Unsortable> {
    let text = single(sql, dialect)?;
    if keys.is_empty() {
        return Ok(text.to_owned());
    }
    let order: Vec<String> = keys
        .iter()
        .map(|k| {
            let dir = if k.descending { "DESC" } else { "ASC" };
            format!("{} {dir}", u64::from(k.column) + 1)
        })
        .collect();
    Ok(format!(
        "SELECT * FROM (\n{text}\n) AS q ORDER BY {}",
        order.join(", ")
    ))
}
