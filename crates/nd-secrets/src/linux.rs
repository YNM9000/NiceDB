use std::collections::HashMap;

use dbus_secret_service::{Collection, EncryptionType, Error as SsError, SecretService};

use crate::{Error, Result, other};

pub(crate) fn missing_default(e: &keyring::Error) -> bool {
    match e {
        keyring::Error::NoStorageAccess(inner) => {
            matches!(inner.downcast_ref::<SsError>(), Some(SsError::NoResult))
        }
        _ => false,
    }
}

fn attributes<'a>(service: &'a str, key: &'a str) -> HashMap<&'a str, &'a str> {
    HashMap::from([
        ("service", service),
        ("username", key),
        ("target", "default"),
        ("application", "rust-keyring"),
    ])
}

fn login(ss: &SecretService) -> Option<Collection<'_>> {
    ss.get_collection_by_alias("login").ok().or_else(|| {
        ss.get_all_collections()
            .ok()?
            .into_iter()
            .find(|c| c.get_label().is_ok_and(|l| l.eq_ignore_ascii_case("login")))
    })
}

pub(crate) fn set_in_login(service: &str, key: &str, value: &str) -> Result<()> {
    let ss = SecretService::connect(EncryptionType::Dh).map_err(other)?;
    let collection = login(&ss).ok_or(Error::NoKeyring)?;
    collection.ensure_unlocked().map_err(other)?;
    collection
        .create_item(
            &format!("{key}@{service}:default"),
            attributes(service, key),
            value.as_bytes(),
            true,
            "text/plain",
        )
        .map_err(other)?;
    Ok(())
}
