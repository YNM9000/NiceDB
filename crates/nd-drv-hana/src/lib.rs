pub mod catalog;
mod client;
mod value;

use std::net::IpAddr;

use nd_driver::{Connection, HanaTarget, Result};

pub fn capabilities() -> nd_driver::Capabilities {
    use nd_driver::{Capabilities, Group, NamespaceLevel, Tool};
    let mut caps = Capabilities::relational(nd_sql::Dialect::Hana)
        .groups(&[
            Group::Tables,
            Group::Views,
            Group::Functions,
            Group::Sequences,
            Group::Triggers,
        ])
        .namespaces(NamespaceLevel::Always);
    caps.tree.databases = false;
    caps.tools.retain(|t| *t != Tool::Backup);
    caps
}

pub fn url(t: &HanaTarget, via: Option<(IpAddr, u16)>) -> String {
    let (host, port) = via.map_or_else(|| (t.host.clone(), t.port), |(ip, p)| (ip.to_string(), p));
    let scheme = if t.tls == nd_driver::TlsMode::Disable {
        "hdbsql"
    } else {
        "hdbsqls"
    };
    let mut query = Vec::new();
    if !t.database.trim().is_empty() {
        query.push(format!("db={}", t.database.trim()));
    }
    match (t.tls, t.ca_file.as_deref()) {
        (nd_driver::TlsMode::Insecure, _) => {
            query.push("insecure_omit_server_certificate_check".into());
        }
        (nd_driver::TlsMode::Verify, Some(ca)) => {
            let dir = std::path::Path::new(ca)
                .parent()
                .map_or_else(|| ca.to_owned(), |d| d.to_string_lossy().into_owned());
            query.push(format!("tls_certificate_dir={dir}"));
        }
        (nd_driver::TlsMode::Verify, None) => query.push("use_mozillas_root_certificates".into()),
        (nd_driver::TlsMode::Disable, _) => {}
    }
    let mut out = format!("{scheme}://{}@{host}:{port}", t.user);
    if !query.is_empty() {
        out.push('?');
        out.push_str(&query.join("&"));
    }
    out
}

pub async fn connect(
    target: &HanaTarget,
    password: Option<&str>,
    via: Option<(IpAddr, u16)>,
) -> Result<Box<dyn Connection>> {
    let address = url(target, via);
    client::connect(address, password.unwrap_or_default().to_owned()).await
}
