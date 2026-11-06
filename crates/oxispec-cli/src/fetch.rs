//! A plain HTTP GET with the standard library, for reading the description a
//! running service serves. HTTPS is not supported: no TLS library is
//! linked. Export the description to a file in CI for those services.

use std::io::{Read, Write};
use std::net::{TcpStream, ToSocketAddrs};
use std::time::Duration;

use anyhow::{Context, Result, anyhow, bail};

/// `http://host:port/path` as its parts.
fn split(url: &str) -> Result<(String, u16, String)> {
    if url.starts_with("https://") {
        bail!(
            "https is not supported; export the description to a file (for example with curl) and use --against FILE"
        );
    }
    let rest = url
        .strip_prefix("http://")
        .ok_or_else(|| anyhow!("{url} is not an http:// address"))?;
    let (authority, path) = match rest.find('/') {
        Some(i) => (&rest[..i], &rest[i..]),
        None => (rest, "/"),
    };
    let (host, port) = match authority.rsplit_once(':') {
        Some((h, p)) => (h, p.parse().with_context(|| format!("bad port in {url}"))?),
        None => (authority, 80),
    };
    if host.is_empty() {
        bail!("no host in {url}");
    }
    Ok((host.to_string(), port, path.to_string()))
}

fn dechunk(body: &[u8]) -> Result<Vec<u8>> {
    let mut out = Vec::new();
    let mut rest = body;
    loop {
        let end = rest
            .windows(2)
            .position(|w| w == b"\r\n")
            .ok_or_else(|| anyhow!("the chunked body is cut short"))?;
        let size_text = std::str::from_utf8(&rest[..end])?;
        let size = usize::from_str_radix(size_text.split(';').next().unwrap_or("").trim(), 16)
            .context("bad chunk size")?;
        rest = &rest[end + 2..];
        if size == 0 {
            return Ok(out);
        }
        if rest.len() < size + 2 {
            bail!("the chunked body is cut short");
        }
        out.extend_from_slice(&rest[..size]);
        rest = &rest[size + 2..];
    }
}

// spec: REQ-API-0005
/// The body of a 200 response, as text.
pub fn get(url: &str) -> Result<String> {
    let (host, port, path) = split(url)?;
    let addr = (host.as_str(), port)
        .to_socket_addrs()
        .with_context(|| format!("cannot find {host}"))?
        .next()
        .ok_or_else(|| anyhow!("cannot find {host}"))?;
    let mut stream = TcpStream::connect_timeout(&addr, Duration::from_secs(5))
        .with_context(|| format!("cannot connect to {host}:{port}"))?;
    stream.set_read_timeout(Some(Duration::from_secs(15)))?;
    write!(
        stream,
        "GET {path} HTTP/1.1\r\nHost: {host}\r\nAccept: application/json, application/yaml, */*\r\nUser-Agent: oxispec\r\nConnection: close\r\n\r\n"
    )?;
    let mut raw = Vec::new();
    stream
        .take(32 * 1024 * 1024)
        .read_to_end(&mut raw)
        .with_context(|| format!("cannot read the answer of {url}"))?;
    let split_at = raw
        .windows(4)
        .position(|w| w == b"\r\n\r\n")
        .ok_or_else(|| anyhow!("{url} did not answer with HTTP"))?;
    let head = String::from_utf8_lossy(&raw[..split_at]).to_string();
    let body = &raw[split_at + 4..];
    let status = head
        .lines()
        .next()
        .and_then(|l| l.split_whitespace().nth(1))
        .unwrap_or("");
    if status != "200" {
        bail!(
            "{url} answered {}",
            head.lines().next().unwrap_or("with nothing")
        );
    }
    let chunked = head.lines().any(|l| {
        let l = l.to_ascii_lowercase();
        l.starts_with("transfer-encoding:") && l.contains("chunked")
    });
    let body = if chunked {
        dechunk(body)?
    } else {
        body.to_vec()
    };
    String::from_utf8(body).context("the answer is not text")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_address_is_split_and_https_is_refused() {
        assert_eq!(
            split("http://localhost:8080/v3/api-docs").unwrap(),
            ("localhost".into(), 8080, "/v3/api-docs".into())
        );
        assert_eq!(split("http://svc").unwrap(), ("svc".into(), 80, "/".into()));
        assert!(
            split("https://svc/x")
                .unwrap_err()
                .to_string()
                .contains("https")
        );
        assert!(split("ftp://svc").is_err());
    }

    #[test]
    fn a_chunked_body_is_joined() {
        let body = b"5\r\nhello\r\n6\r\n world\r\n0\r\n\r\n";
        assert_eq!(dechunk(body).unwrap(), b"hello world");
        assert!(dechunk(b"5\r\nhe").is_err());
    }
}
