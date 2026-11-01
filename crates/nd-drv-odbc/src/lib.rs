mod cancel;
#[cfg(unix)]
mod cli;
mod client;
pub mod db2;
pub mod statement;

use std::net::IpAddr;
use std::path::Path;

use nd_driver::{Connection, Db2Target, DriverError, OdbcTarget, Result};

pub fn check_db2_home(home: Option<&str>) -> Result<std::path::PathBuf> {
    let home = home.map(str::trim).filter(|h| !h.is_empty()).ok_or_else(|| {
        DriverError::Connect {
            message: "the IBM Db2 CLI driver folder is not set: download the clidriver from IBM and choose its folder in Settings › Drivers".into(),
        }
    })?;
    let path = Path::new(home);
    if !cfg!(windows) && db2_driver_file(path).is_none() {
        return Err(DriverError::Connect {
            message: format!(
                "IBM Db2 CLI driver not found in {home}: neither lib/libdb2.so nor libdb2o.so is there"
            ),
        });
    }
    Ok(path.to_path_buf())
}

#[derive(Debug, PartialEq, Eq)]
pub enum Db2Driver {
    Odbc(std::path::PathBuf),
    Cli(std::path::PathBuf),
}

pub fn db2_driver_file(home: &Path) -> Option<Db2Driver> {
    let odbc = statement::db2_library(home);
    if odbc.is_file() {
        return Some(Db2Driver::Odbc(odbc));
    }
    let name = if cfg!(target_os = "macos") {
        "libdb2.dylib"
    } else {
        "libdb2.so"
    };
    [home.join("lib").join(name), home.join(name)]
        .into_iter()
        .find(|p| p.is_file())
        .map(Db2Driver::Cli)
}

pub fn capabilities(dialect: nd_sql::Dialect) -> nd_driver::Capabilities {
    use nd_driver::{Capabilities, Group, NamespaceLevel, Tool};
    if dialect == nd_sql::Dialect::Db2 {
        let mut caps = Capabilities::relational(dialect)
            .groups(&[
                Group::Tables,
                Group::Views,
                Group::MaterializedViews,
                Group::Functions,
                Group::Sequences,
                Group::Triggers,
            ])
            .namespaces(NamespaceLevel::Always);
        caps.tree.databases = false;
        caps.tools.retain(|t| *t != Tool::Backup);
        return caps;
    }
    let mut caps = Capabilities::relational(dialect)
        .groups(&[Group::Tables, Group::Views])
        .namespaces(NamespaceLevel::Multiple);
    caps.tree.databases = false;
    caps.tree.server = false;
    caps.explain = false;
    caps.tools.retain(|t| {
        matches!(
            t,
            Tool::Compare | Tool::Rows | Tool::Transfer | Tool::Search | Tool::Generate
        )
    });
    caps
}

pub async fn connect(target: &OdbcTarget, password: Option<&str>) -> Result<Box<dyn Connection>> {
    let text = statement::odbc_string(target, password);
    client::connect(text, nd_sql::Dialect::Ansi).await
}

pub async fn connect_db2(
    target: &Db2Target,
    password: Option<&str>,
    via: Option<(IpAddr, u16)>,
    home: Option<&str>,
) -> Result<Box<dyn Connection>> {
    let home = check_db2_home(home)?;
    match db2_driver_file(&home) {
        #[cfg(unix)]
        Some(Db2Driver::Cli(lib)) => {
            let text = statement::db2_string("", target, password, via);
            let opened = tokio::task::spawn_blocking(move || cli::CliBackend::open(&lib, &text))
                .await
                .map_err(|e| DriverError::Io {
                    message: e.to_string(),
                })?;
            let (backend, stop) = opened?;
            let backend = std::sync::Mutex::new(Some(backend));
            client::spawn(nd_sql::Dialect::Db2, std::sync::Arc::new(stop), move || {
                backend
                    .lock()
                    .ok()
                    .and_then(|mut b| b.take())
                    .map(|b| Box::new(b) as Box<dyn client::Backend>)
                    .ok_or_else(|| DriverError::query("the Db2 connection was lost"))
            })
            .await
        }
        _ => {
            let text = statement::db2_string(&statement::db2_driver(&home), target, password, via);
            client::connect(text, nd_sql::Dialect::Db2).await
        }
    }
}
