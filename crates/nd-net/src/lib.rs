mod proxy;

use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use nd_driver::{SshAuth, SshConfig};
use russh::client::{self, AuthResult, Handle};
use russh::keys::{self, HashAlg, PrivateKeyWithHashAlg, PublicKey, PublicKeyOrCertificate};
use serde::Serialize;
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::net::TcpListener;
use tokio::task::JoinHandle;

pub use proxy::Proxy;

#[derive(Debug, Clone, thiserror::Error, Serialize, specta::Type)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum TunnelError {
    #[error("unknown host key for {host}: {fingerprint}")]
    HostKeyUnknown {
        host: String,
        port: u16,
        fingerprint: String,
        key: String,
    },
    #[error("host key for {host} changed")]
    HostKeyChanged { host: String },
    #[error("{message}")]
    Auth { message: String },
    #[error("{message}")]
    Connect { message: String },
    #[error("{message}")]
    Proxy { message: String },
    #[error("{message}")]
    ProxyAuth { message: String },
}

pub struct KnownHosts {
    pub files: Vec<PathBuf>,
}

impl KnownHosts {
    pub fn trust(&self, host: &str, port: u16, key_openssh: &str) -> Result<(), TunnelError> {
        let key = PublicKey::from_openssh(key_openssh).map_err(|e| TunnelError::Connect {
            message: e.to_string(),
        })?;
        let file = self.files.last().ok_or_else(|| TunnelError::Connect {
            message: "no known_hosts file".into(),
        })?;
        keys::known_hosts::learn_known_hosts_path(host, port, &key, file).map_err(|e| {
            TunnelError::Connect {
                message: e.to_string(),
            }
        })
    }
}

#[derive(Default)]
enum Verdict {
    #[default]
    Pending,
    Unknown(PublicKey),
    Changed,
}

struct Checker {
    host: String,
    port: u16,
    files: Vec<PathBuf>,
    verdict: Arc<Mutex<Verdict>>,
}

impl client::Handler for Checker {
    type Error = russh::Error;

    #[allow(clippy::unused_async_trait_impl)]
    async fn check_server_key(
        &mut self,
        server: &PublicKeyOrCertificate,
    ) -> Result<bool, Self::Error> {
        let PublicKeyOrCertificate::PublicKey { key, .. } = server else {
            return Ok(false);
        };
        for file in self.files.iter().filter(|f| f.exists()) {
            match keys::check_known_hosts_path(&self.host, self.port, key, file) {
                Ok(true) => return Ok(true),
                Ok(false) => {}
                Err(_) => {
                    *self.verdict.lock().map_err(|_| russh::Error::Disconnect)? = Verdict::Changed;
                    return Ok(false);
                }
            }
        }
        *self.verdict.lock().map_err(|_| russh::Error::Disconnect)? = Verdict::Unknown(key.clone());
        Ok(false)
    }
}

pub struct Tunnel {
    pub local: SocketAddr,
    task: JoinHandle<()>,
}

impl Drop for Tunnel {
    fn drop(&mut self) {
        self.task.abort();
    }
}

#[derive(Clone, Copy)]
pub struct Hop<'a> {
    pub host: &'a str,
    pub port: u16,
    pub user: &'a str,
    pub auth: &'a SshAuth,
    pub secret: Option<&'a str>,
}

#[derive(Default)]
pub struct Route<'a> {
    pub proxy: Proxy<'a>,
    pub hops: Vec<Hop<'a>>,
    pub agent: Option<PathBuf>,
}

impl<'a> Route<'a> {
    pub fn ssh(
        config: &'a SshConfig,
        secret: Option<&'a str>,
        jumps: &'a [Option<String>],
    ) -> Self {
        let mut hops: Vec<Hop<'a>> = config
            .jumps
            .iter()
            .enumerate()
            .map(|(i, j)| Hop {
                host: &j.host,
                port: j.port,
                user: &j.user,
                auth: &j.auth,
                secret: jumps.get(i).and_then(Option::as_deref),
            })
            .collect();
        hops.push(Hop {
            host: &config.host,
            port: config.port,
            user: &config.user,
            auth: &config.auth,
            secret,
        });
        Self {
            proxy: None,
            hops,
            agent: None,
        }
    }
}

pub async fn open(
    config: &SshConfig,
    secret: Option<&str>,
    known: &KnownHosts,
    target: (String, u16),
) -> Result<Tunnel, TunnelError> {
    open_route(&Route::ssh(config, secret, &[]), known, target).await
}

fn connect_error(e: &dyn std::fmt::Display) -> TunnelError {
    TunnelError::Connect {
        message: e.to_string(),
    }
}

pub async fn open_route(
    route: &Route<'_>,
    known: &KnownHosts,
    target: (String, u16),
) -> Result<Tunnel, TunnelError> {
    let listener = TcpListener::bind(("127.0.0.1", 0))
        .await
        .map_err(|e| connect_error(&e))?;
    let local = listener.local_addr().map_err(|e| connect_error(&e))?;

    let Some((first, rest)) = route.hops.split_first() else {
        let Some((p, password)) = route.proxy else {
            return Err(connect_error(&"nothing to tunnel through"));
        };
        let (p, password) = (p.clone(), password.map(str::to_owned));
        let mut ready =
            Some(proxy::dial(Some((&p, password.as_deref())), &target.0, target.1).await?);
        let task = tokio::spawn(async move {
            while let Ok((mut socket, _)) = listener.accept().await {
                let (p, password, (host, port)) = (p.clone(), password.clone(), target.clone());
                let early = ready.take();
                tokio::spawn(async move {
                    let dialed = match early {
                        Some(s) => Ok(s),
                        None => proxy::dial(Some((&p, password.as_deref())), &host, port).await,
                    };
                    match dialed {
                        Ok(mut stream) => {
                            let _ = tokio::io::copy_bidirectional(&mut socket, &mut stream).await;
                        }
                        Err(e) => tracing::warn!(error = %e, "proxy refused"),
                    }
                });
            }
        });
        return Ok(Tunnel { local, task });
    };

    let stream = proxy::dial(route.proxy, first.host, first.port).await?;
    let mut session = handshake(stream, first, known, route.agent.as_deref()).await?;
    let mut held = Vec::new();
    for hop in rest {
        let channel = session
            .channel_open_direct_tcpip(hop.host, u32::from(hop.port), "127.0.0.1", 0)
            .await
            .map_err(|e| TunnelError::Connect {
                message: format!("{}:{}: {e}", hop.host, hop.port),
            })?;
        let next = handshake(channel.into_stream(), hop, known, route.agent.as_deref()).await?;
        held.push(session);
        session = next;
    }

    let session = Arc::new(session);
    let task = tokio::spawn(async move {
        let _held = held;
        while let Ok((mut socket, peer)) = listener.accept().await {
            let session = session.clone();
            let (host, port) = target.clone();
            tokio::spawn(async move {
                let channel = match session
                    .channel_open_direct_tcpip(
                        host,
                        u32::from(port),
                        peer.ip().to_string(),
                        u32::from(peer.port()),
                    )
                    .await
                {
                    Ok(c) => c,
                    Err(e) => {
                        tracing::warn!(error = %e, "ssh direct-tcpip refused");
                        return;
                    }
                };
                let mut stream = channel.into_stream();
                let _ = tokio::io::copy_bidirectional(&mut socket, &mut stream).await;
            });
        }
    });
    Ok(Tunnel { local, task })
}

async fn handshake<S>(
    stream: S,
    hop: &Hop<'_>,
    known: &KnownHosts,
    agent: Option<&std::path::Path>,
) -> Result<Handle<Checker>, TunnelError>
where
    S: AsyncRead + AsyncWrite + Unpin + Send + 'static,
{
    let verdict = Arc::new(Mutex::new(Verdict::Pending));
    let checker = Checker {
        host: hop.host.to_owned(),
        port: hop.port,
        files: known.files.clone(),
        verdict: verdict.clone(),
    };
    let ssh_config = Arc::new(client::Config {
        inactivity_timeout: None,
        keepalive_interval: Some(Duration::from_secs(30)),
        ..Default::default()
    });
    let connecting = client::connect_stream(ssh_config, stream, checker);
    let mut session = match tokio::time::timeout(Duration::from_secs(15), connecting).await {
        Err(_) => {
            return Err(TunnelError::Connect {
                message: format!("{}:{}: timed out", hop.host, hop.port),
            });
        }
        Ok(Ok(s)) => s,
        Ok(Err(e)) => {
            let v = std::mem::take(&mut *verdict.lock().map_err(|_| TunnelError::Connect {
                message: "poisoned".into(),
            })?);
            return Err(match v {
                Verdict::Unknown(key) => TunnelError::HostKeyUnknown {
                    host: hop.host.to_owned(),
                    port: hop.port,
                    fingerprint: key.fingerprint(HashAlg::Sha256).to_string(),
                    key: key.to_openssh().unwrap_or_default(),
                },
                Verdict::Changed => TunnelError::HostKeyChanged {
                    host: hop.host.to_owned(),
                },
                Verdict::Pending => TunnelError::Connect {
                    message: format!("{}:{}: {e}", hop.host, hop.port),
                },
            });
        }
    };
    authenticate(&mut session, hop, agent).await?;
    Ok(session)
}

async fn authenticate(
    session: &mut Handle<Checker>,
    hop: &Hop<'_>,
    agent_path: Option<&std::path::Path>,
) -> Result<(), TunnelError> {
    let auth = |e: &dyn std::fmt::Display| TunnelError::Auth {
        message: format!("{}@{}: {e}", hop.user, hop.host),
    };
    let result = match hop.auth {
        SshAuth::Password => session
            .authenticate_password(hop.user, hop.secret.unwrap_or_default())
            .await
            .map_err(|e| auth(&e))?,
        SshAuth::Key { path } => {
            let key = keys::load_secret_key(expand_home(path), hop.secret).map_err(|e| auth(&e))?;
            let hash = session
                .best_supported_rsa_hash()
                .await
                .map_err(|e| auth(&e))?
                .flatten();
            session
                .authenticate_publickey(hop.user, PrivateKeyWithHashAlg::new(Arc::new(key), hash))
                .await
                .map_err(|e| auth(&e))?
        }
        SshAuth::Agent => {
            #[cfg(unix)]
            let mut agent = match agent_path {
                Some(p) => keys::agent::client::AgentClient::connect_uds(p).await,
                None => keys::agent::client::AgentClient::connect_env().await,
            }
            .map_err(|e| auth(&e))?;
            #[cfg(windows)]
            let mut agent = {
                let _ = agent_path;
                keys::agent::client::AgentClient::connect_named_pipe(r"\\.\pipe\openssh-ssh-agent")
                    .await
                    .map_err(|e| auth(&e))?
            };
            let identities = agent.request_identities().await.map_err(|e| auth(&e))?;
            let hash = session
                .best_supported_rsa_hash()
                .await
                .map_err(|e| auth(&e))?
                .flatten();
            let mut result = AuthResult::Failure {
                remaining_methods: russh::MethodSet::empty(),
                partial_success: false,
            };
            for id in identities {
                result = session
                    .authenticate_publickey_with(
                        hop.user,
                        id.public_key().into_owned(),
                        hash,
                        &mut agent,
                    )
                    .await
                    .map_err(|e| auth(&e))?;
                if result.success() {
                    break;
                }
            }
            result
        }
    };
    if result.success() {
        Ok(())
    } else {
        Err(TunnelError::Auth {
            message: format!("{}@{}: authentication refused", hop.user, hop.host),
        })
    }
}

fn expand_home(path: &str) -> PathBuf {
    match (path.strip_prefix("~/"), std::env::var_os("HOME")) {
        (Some(rest), Some(home)) => PathBuf::from(home).join(rest),
        _ => PathBuf::from(path),
    }
}
