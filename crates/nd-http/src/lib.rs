mod tls;

use std::fmt::Write as _;
use std::net::IpAddr;
use std::sync::Arc;
use std::time::Duration;

use bytes::Bytes;
use http_body_util::{BodyExt, Full};
use hyper::body::Incoming;
use hyper::{Method, Request};
use hyper_util::rt::TokioIo;
use nd_driver::{DriverError, Result};
use rustls::ClientConfig;
use rustls::pki_types::ServerName;
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::net::TcpStream;

pub use hyper::header::HeaderMap;
pub use tls::config as tls_config;

const DIAL_TIMEOUT: Duration = Duration::from_secs(10);

#[derive(Clone)]
pub struct Http {
    host: String,
    port: u16,
    dial: (String, u16),
    tls: Option<Arc<ClientConfig>>,
}

pub struct Response {
    pub status: u16,
    pub headers: HeaderMap,
    body: Incoming,
}

fn io(message: impl Into<String>) -> DriverError {
    DriverError::Io {
        message: message.into(),
    }
}

impl Http {
    pub fn new(
        host: &str,
        port: u16,
        via: Option<(IpAddr, u16)>,
        tls: Option<Arc<ClientConfig>>,
    ) -> Self {
        let dial = via.map_or_else(|| (host.to_owned(), port), |(ip, p)| (ip.to_string(), p));
        Self {
            host: host.to_owned(),
            port,
            dial,
            tls,
        }
    }

    pub fn host(&self) -> &str {
        &self.host
    }

    pub fn authority(&self) -> String {
        let default = if self.tls.is_some() { 443 } else { 80 };
        if self.port == default {
            self.host.clone()
        } else {
            format!("{}:{}", self.host, self.port)
        }
    }

    pub async fn send(
        &self,
        method: &str,
        path: &str,
        headers: &[(&str, &str)],
        body: Bytes,
    ) -> Result<Response> {
        let tcp = tokio::time::timeout(
            DIAL_TIMEOUT,
            TcpStream::connect((self.dial.0.as_str(), self.dial.1)),
        )
        .await
        .map_err(|_| DriverError::Connect {
            message: format!("{}:{} did not answer", self.host, self.port),
        })?
        .map_err(|e| DriverError::Connect {
            message: format!("{}:{}: {e}", self.host, self.port),
        })?;
        let _ = tcp.set_nodelay(true);
        let method = Method::from_bytes(method.as_bytes()).map_err(|e| io(e.to_string()))?;
        let mut request = Request::builder()
            .method(method)
            .uri(if path.starts_with('/') {
                path.to_owned()
            } else {
                format!("/{path}")
            })
            .header(hyper::header::HOST, self.authority());
        for (k, v) in headers {
            request = request.header(*k, *v);
        }
        let request = request
            .body(Full::new(body))
            .map_err(|e| io(e.to_string()))?;
        match &self.tls {
            None => send(tcp, request).await,
            Some(cfg) => {
                let name =
                    ServerName::try_from(self.host.clone()).map_err(|e| DriverError::Tls {
                        message: format!("{}: {e}", self.host),
                    })?;
                let stream = tokio_rustls::TlsConnector::from(cfg.clone())
                    .connect(name, tcp)
                    .await
                    .map_err(|e| DriverError::Tls {
                        message: e.to_string(),
                    })?;
                send(stream, request).await
            }
        }
    }
}

async fn send<S>(stream: S, request: Request<Full<Bytes>>) -> Result<Response>
where
    S: AsyncRead + AsyncWrite + Unpin + Send + 'static,
{
    let (mut sender, conn) = hyper::client::conn::http1::handshake(TokioIo::new(stream))
        .await
        .map_err(|e| io(e.to_string()))?;
    tokio::spawn(async move {
        let _ = conn.await;
    });
    let response = sender
        .send_request(request)
        .await
        .map_err(|e| io(e.to_string()))?;
    let (parts, body) = response.into_parts();
    Ok(Response {
        status: parts.status.as_u16(),
        headers: parts.headers,
        body,
    })
}

impl Response {
    pub fn ok(&self) -> bool {
        (200..300).contains(&self.status)
    }

    pub async fn bytes(mut self, cap: usize) -> Result<Vec<u8>> {
        let mut out = Vec::new();
        while let Some(frame) = self.body.frame().await {
            let frame = frame.map_err(|e| io(e.to_string()))?;
            if let Some(data) = frame.data_ref() {
                if out.len() + data.len() > cap {
                    return Err(DriverError::query(format!(
                        "the reply is larger than {} MB",
                        cap / (1024 * 1024)
                    )));
                }
                out.extend_from_slice(data);
            }
        }
        Ok(out)
    }

    pub async fn text(self, cap: usize) -> Result<String> {
        let bytes = self.bytes(cap).await?;
        Ok(String::from_utf8_lossy(&bytes).into_owned())
    }
}

pub fn encode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b'~') {
            out.push(char::from(b));
        } else {
            let _ = write!(out, "%{b:02X}");
        }
    }
    out
}

pub fn parse_url(url: &str) -> Option<(bool, String, u16)> {
    let (https, rest) = if let Some(r) = url.trim().strip_prefix("https://") {
        (true, r)
    } else {
        (false, url.trim().strip_prefix("http://")?)
    };
    let authority = rest.split('/').next()?;
    if authority.is_empty() {
        return None;
    }
    let (host, port) = match authority.rsplit_once(':') {
        Some((h, p)) if !h.ends_with(']') || h.starts_with('[') => (h, p.parse().ok()?),
        _ => (authority, if https { 443 } else { 80 }),
    };
    Some((
        https,
        host.trim_start_matches('[')
            .trim_end_matches(']')
            .to_owned(),
        port,
    ))
}
