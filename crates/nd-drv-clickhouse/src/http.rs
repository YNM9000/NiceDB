use std::fmt::Write as _;
use std::net::IpAddr;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use bytes::Bytes;
use http_body_util::{BodyExt, Full};
use hyper::body::Incoming;
use hyper::header::HeaderMap;
use hyper::{Request, StatusCode};
use hyper_util::rt::TokioIo;
use nd_driver::{DriverError, Result};
use rustls::ClientConfig;
use rustls::pki_types::ServerName;
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::net::TcpStream;
use zeroize::Zeroizing;

use crate::format::server_error;

const DIAL_TIMEOUT: Duration = Duration::from_secs(10);
const ERROR_CAP: usize = 64 * 1024;

pub(crate) struct Http {
    host: String,
    port: u16,
    dial: (String, u16),
    tls: Option<Arc<ClientConfig>>,
    user: String,
    password: Zeroizing<String>,
    cert_auth: bool,
    read_only: AtomicBool,
}

pub(crate) struct Reply {
    pub headers: HeaderMap,
    pub body: Incoming,
}

fn io(message: impl Into<String>) -> DriverError {
    DriverError::Io {
        message: message.into(),
    }
}

impl Http {
    pub(crate) fn new(
        host: &str,
        port: u16,
        via: Option<(IpAddr, u16)>,
        tls: Option<Arc<ClientConfig>>,
        user: &str,
        password: Option<&str>,
    ) -> Self {
        let dial = via.map_or_else(|| (host.to_owned(), port), |(ip, p)| (ip.to_string(), p));
        Self {
            host: host.to_owned(),
            port,
            dial,
            tls,
            user: if user.is_empty() { "default" } else { user }.to_owned(),
            password: Zeroizing::new(password.unwrap_or_default().to_owned()),
            cert_auth: false,
            read_only: AtomicBool::new(false),
        }
    }

    pub(crate) fn set_read_only(&self, on: bool) {
        self.read_only.store(on, Ordering::Release);
    }

    pub(crate) fn certificate_auth(mut self, on: bool) -> Self {
        self.cert_auth = on;
        self
    }

    pub(crate) async fn post(&self, params: &[(&str, &str)], body: &str) -> Result<Reply> {
        self.dispatch(params, body, self.read_only.load(Ordering::Acquire))
            .await
    }

    pub(crate) async fn post_control(&self, body: &str) -> Result<Reply> {
        self.dispatch(&[], body, false).await
    }

    async fn dispatch(
        &self,
        params: &[(&str, &str)],
        body: &str,
        read_only: bool,
    ) -> Result<Reply> {
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
        let request = self.request(params, body, read_only)?;
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

    fn request(
        &self,
        params: &[(&str, &str)],
        body: &str,
        read_only: bool,
    ) -> Result<Request<Full<Bytes>>> {
        let mut uri = String::from("/?");
        let read_only = read_only.then_some(("readonly", "2"));
        for (i, (k, v)) in params.iter().chain(read_only.as_ref()).enumerate() {
            if i > 0 {
                uri.push('&');
            }
            encode(&mut uri, k);
            uri.push('=');
            encode(&mut uri, v);
        }
        let request = Request::post(uri)
            .header(hyper::header::HOST, format!("{}:{}", self.host, self.port))
            .header("X-ClickHouse-User", &self.user);
        let request = if self.cert_auth {
            request.header("X-ClickHouse-SSL-Certificate-Auth", "on")
        } else {
            request.header("X-ClickHouse-Key", self.password.as_str())
        };
        request
            .header(hyper::header::CONTENT_TYPE, "text/plain; charset=utf-8")
            .body(Full::new(Bytes::copy_from_slice(body.as_bytes())))
            .map_err(|e| io(e.to_string()))
    }
}

async fn send<S>(stream: S, request: Request<Full<Bytes>>) -> Result<Reply>
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
    if parts.status == StatusCode::OK {
        return Ok(Reply {
            headers: parts.headers,
            body,
        });
    }
    let text = read_capped(body).await;
    Err(server_error(&text).unwrap_or_else(|| match parts.status {
        StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN => DriverError::Auth {
            message: text.trim().to_owned(),
        },
        status => DriverError::query(format!("HTTP {status}: {}", text.trim())),
    }))
}

async fn read_capped(mut body: Incoming) -> String {
    let mut out = Vec::new();
    while out.len() < ERROR_CAP {
        match body.frame().await {
            Some(Ok(frame)) => {
                if let Some(data) = frame.data_ref() {
                    out.extend_from_slice(data);
                }
            }
            _ => break,
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn encode(out: &mut String, s: &str) {
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b'~') {
            out.push(char::from(b));
        } else {
            let _ = write!(out, "%{b:02X}");
        }
    }
}

pub(crate) struct Lines {
    body: Incoming,
    buf: Vec<u8>,
    done: bool,
}

impl Lines {
    pub(crate) fn new(body: Incoming) -> Self {
        Self {
            body,
            buf: Vec::new(),
            done: false,
        }
    }

    pub(crate) async fn next(&mut self) -> Result<Option<String>> {
        loop {
            if let Some(at) = self.buf.iter().position(|b| *b == b'\n') {
                let mut line: Vec<u8> = self.buf.drain(..=at).collect();
                line.pop();
                return Ok(Some(String::from_utf8_lossy(&line).into_owned()));
            }
            if self.done {
                if self.buf.is_empty() {
                    return Ok(None);
                }
                let line = std::mem::take(&mut self.buf);
                return Ok(Some(String::from_utf8_lossy(&line).into_owned()));
            }
            match self.body.frame().await {
                Some(Ok(frame)) => {
                    if let Some(data) = frame.data_ref() {
                        self.buf.extend_from_slice(data);
                    }
                }
                Some(Err(e)) => return Err(io(e.to_string())),
                None => self.done = true,
            }
        }
    }
}
