use mysql_async::{ClientIdentity, SslOpts};
use nd_driver::{ClientCert, DriverError, Result, TlsMode};

pub(crate) async fn ssl_opts(
    mode: TlsMode,
    ca_file: Option<&str>,
    client: Option<(&ClientCert, Option<&str>)>,
) -> Result<Option<SslOpts>> {
    let _ = rustls::crypto::ring::default_provider().install_default();
    let mut opts = SslOpts::default();
    if let Some((cert, passphrase)) = client
        && mode != TlsMode::Disable
    {
        let id = nd_driver::load_identity(cert, passphrase)?;
        opts = opts.with_client_identity(Some(ClientIdentity::new(
            id.certs_pem().into(),
            id.key_pem().into(),
        )));
    }
    Ok(match mode {
        TlsMode::Disable => None,
        TlsMode::Insecure => Some(opts.with_danger_accept_invalid_certs(true)),
        TlsMode::Verify => {
            let roots = match ca_file {
                Some(path) => vec![
                    tokio::fs::read(path)
                        .await
                        .map_err(|e| DriverError::Tls {
                            message: format!("{path}: {e}"),
                        })?
                        .into(),
                ],
                None => rustls_native_certs::load_native_certs()
                    .certs
                    .into_iter()
                    .map(|c| c.to_vec().into())
                    .collect::<Vec<_>>(),
            };
            Some(
                opts.with_disable_built_in_roots(true)
                    .with_root_certs(roots),
            )
        }
    })
}
