use std::path::{Path, PathBuf};

use nd_store::{
    ExportSelection, ImportChoice, ImportOutcome, ImportPreview, PACKAGE_VERSION, PLAIN_FORMAT,
    Package, PackedSecret,
};
use serde::{Deserialize, Serialize};
use zeroize::{Zeroize, Zeroizing};

use crate::session::{secret_key, secret_names};
use crate::{Core, CoreError, Result};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "snake_case")]
pub enum PackageFault {
    NeedsPassphrase,
    WrongPassphrase,
    Malformed,
    Unsupported,
    WrongExtension,
}

impl From<nd_pack::Error> for CoreError {
    fn from(e: nd_pack::Error) -> Self {
        Self::Package(match e {
            nd_pack::Error::Decrypt => PackageFault::WrongPassphrase,
            nd_pack::Error::EmptyPassphrase => PackageFault::NeedsPassphrase,
            nd_pack::Error::Unsupported | nd_pack::Error::Cost => PackageFault::Unsupported,
            nd_pack::Error::Malformed | nd_pack::Error::Random => PackageFault::Malformed,
        })
    }
}

#[derive(Deserialize, specta::Type)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ExportFormat {
    Sealed { passphrase: String },
    Plain,
}

impl Drop for ExportFormat {
    fn drop(&mut self) {
        if let Self::Sealed { passphrase } = self {
            passphrase.zeroize();
        }
    }
}

fn extension_is(path: &Path, ext: &str) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case(ext))
}

fn io(e: impl std::fmt::Display) -> CoreError {
    CoreError::Io(e.to_string())
}

async fn blocking<T: Send + 'static>(f: impl FnOnce() -> Result<T> + Send + 'static) -> Result<T> {
    tokio::task::spawn_blocking(f).await.map_err(io)?
}

fn decode(bytes: &[u8], passphrase: Option<&str>) -> Result<(Package, bool)> {
    let (package, sealed) = if nd_pack::is_sealed(bytes) {
        let passphrase = passphrase
            .filter(|p| !p.is_empty())
            .ok_or(CoreError::Package(PackageFault::NeedsPassphrase))?;
        let plain = nd_pack::open(bytes, passphrase)?;
        let package: Package = serde_json::from_slice(&plain)
            .map_err(|_| CoreError::Package(PackageFault::Malformed))?;
        (package, true)
    } else {
        let package: Package = serde_json::from_slice(bytes)
            .map_err(|_| CoreError::Package(PackageFault::Malformed))?;
        if package.format.as_deref() != Some(PLAIN_FORMAT) {
            return Err(CoreError::Package(PackageFault::Malformed));
        }
        (package, false)
    };
    if package.version > PACKAGE_VERSION {
        return Err(CoreError::Package(PackageFault::Unsupported));
    }
    Ok((package, sealed))
}

impl Core {
    async fn read_package(
        &self,
        path: PathBuf,
        passphrase: Option<String>,
    ) -> Result<(Package, bool)> {
        let passphrase = passphrase.map(Zeroizing::new);
        blocking(move || {
            let bytes = std::fs::read(&path).map_err(io)?;
            decode(&bytes, passphrase.as_deref().map(String::as_str))
        })
        .await
    }

    pub async fn export_package(
        &self,
        path: PathBuf,
        selection: ExportSelection,
        format: ExportFormat,
    ) -> Result<()> {
        let sealed = matches!(format, ExportFormat::Sealed { .. });
        if !extension_is(&path, if sealed { "nicedb" } else { "json" }) {
            return Err(CoreError::Package(PackageFault::WrongExtension));
        }
        let selection = if sealed {
            selection
        } else {
            ExportSelection {
                folders: selection.folders,
                connections: selection.connections,
                ..ExportSelection::default()
            }
        };
        let mut package = self.store(|s| s.snapshot(&selection))?;
        if sealed && selection.secrets {
            let store = self.secrets.clone();
            let ids: Vec<String> = package.connections.iter().map(|c| c.id.clone()).collect();
            package.secrets = blocking(move || {
                let mut out = Vec::new();
                for id in ids {
                    if let Some(password) = store.get(&secret_key(&id, "password"))? {
                        out.push(PackedSecret {
                            connection_id: id,
                            password: Some(password),
                            ssh: None,
                            extra: std::collections::BTreeMap::new(),
                        });
                    }
                }
                Ok(out)
            })
            .await?;
        }
        let passphrase = match &format {
            ExportFormat::Sealed { passphrase } => Some(Zeroizing::new(passphrase.clone())),
            ExportFormat::Plain => None,
        };
        blocking(move || {
            let bytes = if let Some(p) = passphrase {
                let json = Zeroizing::new(serde_json::to_vec(&package).map_err(io)?);
                nd_pack::seal(&json, &p)?
            } else {
                package.format = Some(PLAIN_FORMAT.into());
                serde_json::to_vec_pretty(&package).map_err(io)?
            };
            std::fs::write(&path, bytes).map_err(io)
        })
        .await
    }

    pub async fn import_preview(
        &self,
        path: PathBuf,
        passphrase: Option<String>,
    ) -> Result<ImportPreview> {
        let (package, sealed) = self.read_package(path, passphrase).await?;
        let mut preview = self.store(|s| s.preview_import(&package))?;
        preview.sealed = sealed;
        Ok(preview)
    }

    pub async fn import_apply(
        &self,
        path: PathBuf,
        passphrase: Option<String>,
        choices: Vec<ImportChoice>,
    ) -> Result<ImportOutcome> {
        let (package, _) = self.read_package(path, passphrase).await?;
        let applied = self.store(|s| s.apply_import(&package, &choices))?;
        let store = self.secrets.clone();
        let writes: Vec<(String, PackedSecret)> = package
            .secrets
            .iter()
            .filter_map(|s| {
                applied
                    .connections
                    .get(&s.connection_id)
                    .map(|id| (id.clone(), s.clone()))
            })
            .collect();
        blocking(move || {
            for (id, s) in &writes {
                if let Some(p) = &s.password {
                    store.set(&secret_key(id, "password"), p)?;
                }
                if let Some(p) = &s.ssh {
                    store.set(&secret_key(id, "ssh"), p)?;
                }
                for (what, v) in &s.extra {
                    if secret_names().contains(what) {
                        store.set(&secret_key(id, what), v)?;
                    }
                }
            }
            Ok(())
        })
        .await?;
        Ok(applied.outcome)
    }
}
