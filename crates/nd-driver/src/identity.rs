use pkcs8::EncryptedPrivateKeyInfoRef;
use pkcs8::der::pem;
use rustls_pki_types::pem::PemObject;
use rustls_pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer};

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use zeroize::Zeroizing;

use crate::{ClientCert, DriverError, Result};

pub struct Identity {
    pub certs: Vec<CertificateDer<'static>>,
    pub key: PrivateKeyDer<'static>,
}

impl Identity {
    pub fn certs_pem(&self) -> Vec<u8> {
        let mut out = Vec::new();
        for c in &self.certs {
            if let Ok(text) = pem::encode_string("CERTIFICATE", pem::LineEnding::LF, c.as_ref()) {
                out.extend_from_slice(text.as_bytes());
            }
        }
        out
    }

    pub fn key_pem(&self) -> Vec<u8> {
        let label = match &self.key {
            PrivateKeyDer::Pkcs1(_) => "RSA PRIVATE KEY",
            PrivateKeyDer::Sec1(_) => "EC PRIVATE KEY",
            _ => "PRIVATE KEY",
        };
        pem::encode_string(label, pem::LineEnding::LF, self.key.secret_der())
            .map(String::into_bytes)
            .unwrap_or_default()
    }
}

fn tls(message: String) -> DriverError {
    DriverError::Tls { message }
}

pub fn load_identity(cert: &ClientCert, passphrase: Option<&str>) -> Result<Identity> {
    let cert_bytes = std::fs::read(&cert.cert).map_err(|e| tls(format!("{}: {e}", cert.cert)))?;
    let certs = CertificateDer::pem_slice_iter(&cert_bytes)
        .collect::<std::result::Result<Vec<_>, _>>()
        .map_err(|e| tls(format!("{}: {e}", cert.cert)))?;
    if certs.is_empty() {
        return Err(tls(format!("{}: no certificate", cert.cert)));
    }
    let key_bytes = std::fs::read(&cert.key).map_err(|e| tls(format!("{}: {e}", cert.key)))?;
    let key = if String::from_utf8_lossy(&key_bytes).contains("-----BEGIN ENCRYPTED PRIVATE KEY") {
        let pass = passphrase
            .filter(|p| !p.is_empty())
            .ok_or_else(|| tls(format!("{}: the key needs its passphrase", cert.key)))?;
        let (_, der) =
            pem::decode_vec(&key_bytes).map_err(|e| tls(format!("{}: {e}", cert.key)))?;
        let info = EncryptedPrivateKeyInfoRef::try_from(der.as_slice())
            .map_err(|e| tls(format!("{}: {e}", cert.key)))?;
        let plain = info
            .decrypt(pass)
            .map_err(|_| tls(format!("{}: wrong passphrase", cert.key)))?;
        PrivateKeyDer::Pkcs8(PrivatePkcs8KeyDer::from(plain.as_bytes().to_vec()))
    } else if String::from_utf8_lossy(&key_bytes).contains("Proc-Type: 4,ENCRYPTED") {
        return Err(tls(format!(
            "{}: legacy encrypted PEM; convert it with `openssl pkcs8 -topk8`",
            cert.key
        )));
    } else {
        PrivateKeyDer::from_pem_slice(&key_bytes).map_err(|e| tls(format!("{}: {e}", cert.key)))?
    };
    Ok(Identity { certs, key })
}

pub fn key_encrypted(cert: &ClientCert) -> Result<bool> {
    let key = std::fs::read(&cert.key).map_err(|e| tls(format!("{}: {e}", cert.key)))?;
    let text = String::from_utf8_lossy(&key);
    Ok(
        text.contains("-----BEGIN ENCRYPTED PRIVATE KEY")
            || text.contains("Proc-Type: 4,ENCRYPTED"),
    )
}

pub struct KeyFile {
    path: PathBuf,
}

impl KeyFile {
    pub fn write(pem: &[u8]) -> Result<Self> {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let dir = std::env::var_os("XDG_RUNTIME_DIR")
            .map(PathBuf::from)
            .filter(|d| d.is_dir())
            .unwrap_or_else(std::env::temp_dir);
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.subsec_nanos());
        let name = format!(
            "nicedb-key-{}-{}-{stamp:08x}.pem",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        );
        let path = dir.join(name);
        let mut options = std::fs::OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        std::os::unix::fs::OpenOptionsExt::mode(&mut options, 0o600);
        let fail = |e: std::io::Error| tls(format!("{}: {e}", path.display()));
        let mut handle = options.open(&path).map_err(fail)?;
        std::io::Write::write_all(&mut handle, pem).map_err(|e| {
            let _ = std::fs::remove_file(&path);
            fail(e)
        })?;
        Ok(KeyFile { path })
    }

    pub fn of(identity: &Identity) -> Result<Self> {
        Self::write(&Zeroizing::new(identity.key_pem()))
    }

    pub fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for KeyFile {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}

pub fn key_file(cert: &ClientCert, passphrase: Option<&str>) -> Result<Option<KeyFile>> {
    if !key_encrypted(cert)? {
        return Ok(None);
    }
    KeyFile::of(&load_identity(cert, passphrase)?).map(Some)
}
