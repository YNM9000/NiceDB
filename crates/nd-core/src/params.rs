use nd_sql::{Dialect, Filled, ParamError, ParamName, ParamValue, Statement};
use nd_store::ParamUse;
use serde::Serialize;

use crate::offsets::byte_to_utf16;
use crate::run::{RunScope, select};
use crate::{Core, Result};

pub(crate) struct Prepared {
    pub(crate) display: Filled,
    pub(crate) native: Option<Filled>,
}

impl Prepared {
    pub(crate) fn exec(&self) -> &Filled {
        self.native.as_ref().unwrap_or(&self.display)
    }
}

#[derive(Debug, Clone, Serialize, specta::Type)]
pub struct ParamSlot {
    pub name: String,
    pub label: String,
    pub spots: Vec<ParamSpot>,
}

#[derive(Debug, Clone, Copy, Serialize, specta::Type)]
pub struct ParamSpot {
    pub from: u32,
    pub to: u32,
}

#[derive(Debug, Clone, Serialize, specta::Type)]
pub struct ParamsFound {
    pub slots: Vec<ParamSlot>,
    pub history: Vec<ParamUse>,
}

fn found(
    text: &str,
    dialect: Dialect,
    statements: &[Statement],
) -> Vec<Vec<(nd_sql::Placeholder, String)>> {
    let mut positional = 0;
    statements
        .iter()
        .map(|st| {
            let list = nd_sql::placeholders(st.text(text), dialect);
            let first = positional;
            let named: Vec<_> = list
                .into_iter()
                .map(|p| {
                    if let ParamName::Positional(n) = p.name {
                        positional = positional.max(first + n);
                    }
                    let key = p.key(first);
                    (p, key)
                })
                .collect();
            named
        })
        .collect()
}

pub(crate) fn query_key(text: &str, statements: &[Statement]) -> String {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for st in statements {
        for word in st.text(text).split_whitespace() {
            for b in word.bytes().chain(*b" ") {
                hash ^= u64::from(b);
                hash = hash.wrapping_mul(0x0100_0000_01b3);
            }
        }
        hash ^= u64::from(b';');
    }
    format!("{hash:016x}")
}

pub(crate) fn prepare(
    text: &str,
    dialect: Dialect,
    statements: &[Statement],
    values: Option<&[ParamValue]>,
) -> std::result::Result<(Vec<Prepared>, Vec<ParamValue>), ParamError> {
    let plain = |st: &Statement| Prepared {
        display: Filled::plain(st.text(text)),
        native: None,
    };
    let Some(values) = values else {
        return Ok((statements.iter().map(plain).collect(), Vec::new()));
    };
    let mut used: Vec<ParamValue> = Vec::new();
    let mut out = Vec::with_capacity(statements.len());
    for (st, list) in statements.iter().zip(found(text, dialect, statements)) {
        if list.is_empty() {
            out.push(plain(st));
            continue;
        }
        let mut slots = Vec::with_capacity(list.len());
        for (p, key) in &list {
            let value = values
                .iter()
                .find(|v| v.name == *key)
                .ok_or_else(|| ParamError::Missing { name: key.clone() })?;
            if !used.iter().any(|u| u.name == value.name) {
                used.push(value.clone());
            }
            slots.push((p.range.clone(), value));
        }
        let sql = st.text(text);
        let display = nd_sql::bind(sql, dialect, &slots, false)?;
        let native =
            Some(nd_sql::bind(sql, dialect, &slots, true)?).filter(|n| !n.params.is_empty());
        out.push(Prepared { display, native });
    }
    Ok((out, used))
}

pub(crate) fn filled_text(
    text: &str,
    dialect: Dialect,
    st: &Statement,
    values: Option<&[ParamValue]>,
) -> std::result::Result<String, ParamError> {
    let (mut prepared, _) = prepare(text, dialect, std::slice::from_ref(st), values)?;
    Ok(prepared
        .pop()
        .map_or_else(|| st.text(text).to_owned(), |p| p.display.sql))
}

impl Core {
    pub fn params_found(
        &self,
        text: &str,
        dialect: Dialect,
        scope: &RunScope,
    ) -> Result<ParamsFound> {
        let statements = select(text, dialect, scope);
        let mut slots: Vec<ParamSlot> = Vec::new();
        for (st, list) in statements.iter().zip(found(text, dialect, &statements)) {
            for (p, key) in list {
                let spot = ParamSpot {
                    from: byte_to_utf16(text, st.range.start + p.range.start),
                    to: byte_to_utf16(text, st.range.start + p.range.end),
                };
                match slots.iter_mut().find(|s| s.name == key) {
                    Some(s) => s.spots.push(spot),
                    None => slots.push(ParamSlot {
                        label: st.text(text)[p.range.clone()].to_owned(),
                        name: key,
                        spots: vec![spot],
                    }),
                }
            }
        }
        if slots.is_empty() {
            return Ok(ParamsFound {
                slots,
                history: Vec::new(),
            });
        }
        let names: Vec<String> = slots.iter().map(|s| s.name.clone()).collect();
        let key = query_key(text, &statements);
        let history = self.store(|s| s.param_history(&key, &names))?;
        Ok(ParamsFound { slots, history })
    }

    pub(crate) fn remember_params(&self, key: &str, used: &[ParamValue]) {
        if used.is_empty() {
            return;
        }
        if let Err(e) = self.store(|s| s.remember_params(key, used)) {
            tracing::warn!(error = %e, "parameter values not remembered");
        }
    }
}
