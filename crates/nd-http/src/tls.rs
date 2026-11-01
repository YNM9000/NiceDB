use std::sync::Arc;

use nd_driver::{DriverError, Identity, Result, TlsMode};
use rustls::client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier};
use rustls::pki_types::pem::PemObject;
use rustls::pki_types::{CertificateDer, ServerName, UnixTime};
use rustls::{ClientConfig, DigitallySignedStruct, RootCertStore, SignatureScheme};

fn tls(message: impl Into<String>) -> DriverError {
    DriverError::Tls {
        message: message.into(),
    }
}

pub fn config(
    mode: TlsMode,
    ca_file: Option<&str>,
    identity: Option<Identity>,
) -> Result<Option<Arc<ClientConfig>>> {
    let provider = Arc::new(rustls::crypto::ring::default_provider());
    let builder = ClientConfig::builder_with_provider(provider.clone())
        .with_safe_default_protocol_versions()
        .map_err(|e| tls(e.to_string()))?;
    let builder = match mode {
        TlsMode::Disable => return Ok(None),
        TlsMode::Verify => builder.with_root_certificates(roots(ca_file)?),
        TlsMode::Insecure => builder
            .dangerous()
            .with_custom_certificate_verifier(Arc::new(AcceptAny(provider))),
    };
    let config = match identity {
        None => builder.with_no_client_auth(),
        Some(id) => builder
            .with_client_auth_cert(id.certs, id.key)
            .map_err(|e| tls(e.to_string()))?,
    };
    Ok(Some(Arc::new(config)))
}

fn roots(ca_file: Option<&str>) -> Result<RootCertStore> {
    let mut store = RootCertStore::empty();
    if let Some(path) = ca_file {
        for cert in CertificateDer::pem_file_iter(path).map_err(|e| tls(format!("{path}: {e}")))? {
            let cert = cert.map_err(|e| tls(format!("{path}: {e}")))?;
            store.add(cert).map_err(|e| tls(format!("{path}: {e}")))?;
        }
    } else {
        store.add_parsable_certificates(rustls_native_certs::load_native_certs().certs);
    }
    Ok(store)
}

#[derive(Debug)]
struct AcceptAny(Arc<rustls::crypto::CryptoProvider>);

impl ServerCertVerifier for AcceptAny {
    fn verify_server_cert(
        &self,
        _end_entity: &CertificateDer<'_>,
        _intermediates: &[CertificateDer<'_>],
        _server_name: &ServerName<'_>,
        _ocsp_response: &[u8],
        _now: UnixTime,
    ) -> std::result::Result<ServerCertVerified, rustls::Error> {
        Ok(ServerCertVerified::assertion())
    }

    fn verify_tls12_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> std::result::Result<HandshakeSignatureValid, rustls::Error> {
        rustls::crypto::verify_tls12_signature(
            message,
            cert,
            dss,
            &self.0.signature_verification_algorithms,
        )
    }

    fn verify_tls13_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> std::result::Result<HandshakeSignatureValid, rustls::Error> {
        rustls::crypto::verify_tls13_signature(
            message,
            cert,
            dss,
            &self.0.signature_verification_algorithms,
        )
    }

    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        self.0.signature_verification_algorithms.supported_schemes()
    }
}
