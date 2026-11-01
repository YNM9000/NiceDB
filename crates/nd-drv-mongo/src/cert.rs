use std::path::{Path, PathBuf};

use nd_driver::{ClientCert, DriverError, Result};

pub(crate) struct CertFile {
    path: PathBuf,
    _dir: Option<nd_driver::PrivateDir>,
    password: Option<Vec<u8>>,
}

fn tls(message: String) -> DriverError {
    DriverError::Tls { message }
}

fn encrypted(key: &[u8]) -> bool {
    String::from_utf8_lossy(key).contains("-----BEGIN ENCRYPTED PRIVATE KEY")
}

impl CertFile {
    pub(crate) fn new(cert: &ClientCert, passphrase: Option<&str>) -> Result<Self> {
        nd_driver::load_identity(cert, passphrase)?;
        let key = std::fs::read(&cert.key).map_err(|e| tls(format!("{}: {e}", cert.key)))?;
        let password = encrypted(&key)
            .then(|| passphrase.map(|p| p.as_bytes().to_vec()))
            .flatten();
        if Path::new(&cert.cert) == Path::new(&cert.key) {
            return Ok(Self {
                path: cert.cert.clone().into(),
                _dir: None,
                password,
            });
        }
        let chain = std::fs::read(&cert.cert).map_err(|e| tls(format!("{}: {e}", cert.cert)))?;
        let dir = nd_driver::PrivateDir::new().map_err(|e| tls(e.to_string()))?;
        let path = dir
            .write("client.pem", &[&chain, b"\n", &key])
            .map_err(|e| tls(format!("{}: {e}", dir.path().display())))?;
        Ok(Self {
            path,
            _dir: Some(dir),
            password,
        })
    }

    pub(crate) fn path(&self) -> &Path {
        &self.path
    }

    pub(crate) fn password(&self) -> Option<Vec<u8>> {
        self.password.clone()
    }
}
