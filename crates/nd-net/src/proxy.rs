use std::net::IpAddr;
use std::time::Duration;

use nd_driver::{ProxyConfig, ProxyKind};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

use crate::TunnelError;

pub type Proxy<'a> = Option<(&'a ProxyConfig, Option<&'a str>)>;

fn fault(message: impl Into<String>) -> TunnelError {
    TunnelError::Proxy {
        message: message.into(),
    }
}

fn io(e: &std::io::Error) -> TunnelError {
    fault(e.to_string())
}

pub(crate) async fn dial(
    proxy: Proxy<'_>,
    host: &str,
    port: u16,
) -> Result<TcpStream, TunnelError> {
    let Some((p, password)) = proxy else {
        return tokio::time::timeout(Duration::from_secs(15), TcpStream::connect((host, port)))
            .await
            .map_err(|_| TunnelError::Connect {
                message: format!("{host}:{port}: timed out"),
            })?
            .map_err(|e| TunnelError::Connect {
                message: format!("{host}:{port}: {e}"),
            });
    };
    let work = async {
        let mut s = TcpStream::connect((p.host.as_str(), p.port))
            .await
            .map_err(|e| fault(format!("{}:{}: {e}", p.host, p.port)))?;
        let user = p.user.as_deref().filter(|u| !u.is_empty());
        match p.kind {
            ProxyKind::Socks5 => socks5(&mut s, host, port, user, password).await?,
            ProxyKind::Http => http_connect(&mut s, host, port, user, password).await?,
        }
        Ok(s)
    };
    tokio::time::timeout(Duration::from_secs(15), work)
        .await
        .map_err(|_| fault(format!("{}:{}: timed out", p.host, p.port)))?
}

async fn socks5(
    s: &mut TcpStream,
    host: &str,
    port: u16,
    user: Option<&str>,
    password: Option<&str>,
) -> Result<(), TunnelError> {
    let methods: &[u8] = if user.is_some() {
        &[5, 2, 0, 2]
    } else {
        &[5, 1, 0]
    };
    s.write_all(methods).await.map_err(|e| io(&e))?;
    let mut pick = [0u8; 2];
    s.read_exact(&mut pick).await.map_err(|e| io(&e))?;
    match pick {
        [5, 0] => {}
        [5, 2] => {
            let u = user.unwrap_or_default().as_bytes();
            let p = password.unwrap_or_default().as_bytes();
            let (Ok(ul), Ok(pl)) = (u8::try_from(u.len()), u8::try_from(p.len())) else {
                return Err(fault("SOCKS5: user or password longer than 255 bytes"));
            };
            let mut auth = vec![1, ul];
            auth.extend_from_slice(u);
            auth.push(pl);
            auth.extend_from_slice(p);
            s.write_all(&auth).await.map_err(|e| io(&e))?;
            let mut ok = [0u8; 2];
            s.read_exact(&mut ok).await.map_err(|e| io(&e))?;
            if ok[1] != 0 {
                return Err(TunnelError::ProxyAuth {
                    message: "SOCKS5: user or password refused".into(),
                });
            }
        }
        [5, 0xff] => {
            return Err(TunnelError::ProxyAuth {
                message: "SOCKS5: the proxy wants a user and password".into(),
            });
        }
        _ => return Err(fault("SOCKS5: the proxy does not speak SOCKS5")),
    }
    let mut req = vec![5, 1, 0];
    match host.trim_matches(['[', ']']).parse::<IpAddr>() {
        Ok(IpAddr::V4(ip)) => {
            req.push(1);
            req.extend_from_slice(&ip.octets());
        }
        Ok(IpAddr::V6(ip)) => {
            req.push(4);
            req.extend_from_slice(&ip.octets());
        }
        Err(_) => {
            let len = u8::try_from(host.len()).map_err(|_| fault("SOCKS5: host name too long"))?;
            req.push(3);
            req.push(len);
            req.extend_from_slice(host.as_bytes());
        }
    }
    req.extend_from_slice(&port.to_be_bytes());
    s.write_all(&req).await.map_err(|e| io(&e))?;
    let mut head = [0u8; 4];
    s.read_exact(&mut head).await.map_err(|e| io(&e))?;
    if head[1] != 0 {
        let why = match head[1] {
            2 => "not allowed by the proxy rules",
            3 => "network unreachable",
            4 => "host unreachable",
            5 => "connection refused",
            6 => "TTL expired",
            _ => "request failed",
        };
        return Err(fault(format!("SOCKS5: {host}:{port}: {why}")));
    }
    let rest = match head[3] {
        1 => 4 + 2,
        4 => 16 + 2,
        3 => {
            let mut len = [0u8; 1];
            s.read_exact(&mut len).await.map_err(|e| io(&e))?;
            usize::from(len[0]) + 2
        }
        _ => return Err(fault("SOCKS5: malformed reply")),
    };
    let mut skip = vec![0u8; rest];
    s.read_exact(&mut skip).await.map_err(|e| io(&e))?;
    Ok(())
}

async fn http_connect(
    s: &mut TcpStream,
    host: &str,
    port: u16,
    user: Option<&str>,
    password: Option<&str>,
) -> Result<(), TunnelError> {
    let authority = if host.contains(':') && !host.starts_with('[') {
        format!("[{host}]:{port}")
    } else {
        format!("{host}:{port}")
    };
    let auth = user.map_or_else(String::new, |u| {
        let token = base64(format!("{u}:{}", password.unwrap_or_default()).as_bytes());
        format!("Proxy-Authorization: Basic {token}\r\n")
    });
    let req = format!("CONNECT {authority} HTTP/1.1\r\nHost: {authority}\r\n{auth}\r\n");
    s.write_all(req.as_bytes()).await.map_err(|e| io(&e))?;
    let mut head = Vec::new();
    while !head.ends_with(b"\r\n\r\n") {
        if head.len() > 16 * 1024 {
            return Err(fault("HTTP proxy: reply too long"));
        }
        let mut b = [0u8; 1];
        s.read_exact(&mut b).await.map_err(|e| io(&e))?;
        head.push(b[0]);
    }
    let text = String::from_utf8_lossy(&head);
    let status = text.lines().next().unwrap_or_default();
    match status.split_whitespace().nth(1) {
        Some(c) if c.starts_with('2') => Ok(()),
        Some("407") => Err(TunnelError::ProxyAuth {
            message: format!("HTTP proxy: {status}"),
        }),
        _ => Err(fault(format!("HTTP proxy: {status}"))),
    }
}

fn base64(bytes: &[u8]) -> String {
    const A: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let n = chunk
            .iter()
            .enumerate()
            .fold(0u32, |n, (i, b)| n | u32::from(*b) << (16 - 8 * i));
        for i in 0..4 {
            if i <= chunk.len() {
                out.push(char::from(A[(n >> (18 - 6 * i)) as usize & 63]));
            } else {
                out.push('=');
            }
        }
    }
    out
}
