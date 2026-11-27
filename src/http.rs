#![forbid(unsafe_code)]

//! Pure Safe Rust HTTP/1.1 client over `std::net::TcpStream` (zero external crates).
//! Supports `GET` and `POST`, custom headers, `Content-Length`, and `Transfer-Encoding: chunked`.
//! For `https://` URLs (e.g. `check-redirects --live`), delegates safely to system `curl` without shell interpolation.

use std::io::{Read, Write};
use std::net::{TcpStream, ToSocketAddrs};
use std::process::Command;
use std::time::Duration;

#[derive(Debug, Clone)]
pub struct HttpResponse {
    pub status: u16,
    pub body: String,
}

fn parse_http_url(url: &str) -> Result<(String, u16, String), String> {
    let rest = url
        .strip_prefix("http://")
        .ok_or_else(|| format!("expected http:// URL, got '{url}'"))?;
    let (host_port, path) = match rest.find('/') {
        Some(idx) => (&rest[..idx], &rest[idx..]),
        None => (rest, "/"),
    };
    let (host, port) = if let Some(idx) = host_port.rfind(':') {
        let h = &host_port[..idx];
        let p = host_port[idx + 1..]
            .parse::<u16>()
            .map_err(|_| format!("invalid port in URL '{url}'"))?;
        (h.to_string(), p)
    } else {
        (host_port.to_string(), 80)
    };
    Ok((host, port, path.to_string()))
}

fn decode_chunked(raw: &[u8]) -> Result<Vec<u8>, String> {
    let mut out = Vec::new();
    let mut pos = 0;
    while pos < raw.len() {
        let line_end = raw[pos..]
            .windows(2)
            .position(|w| w == b"\r\n")
            .ok_or("malformed chunked response (missing CRLF)")?;
        let size_str = std::str::from_utf8(&raw[pos..pos + line_end])
            .map_err(|e| format!("utf8 chunk size: {e}"))?
            .split(';')
            .next()
            .unwrap_or("")
            .trim();
        let size = usize::from_str_radix(size_str, 16)
            .map_err(|e| format!("invalid chunk size '{size_str}': {e}"))?;
        pos += line_end + 2;
        if size == 0 {
            break;
        }
        if pos + size > raw.len() {
            return Err("truncated chunked body".to_string());
        }
        out.extend_from_slice(&raw[pos..pos + size]);
        pos += size;
        if pos + 2 <= raw.len() && &raw[pos..pos + 2] == b"\r\n" {
            pos += 2;
        }
    }
    Ok(out)
}

fn request_http_native(
    method: &str,
    url: &str,
    headers: &[(&str, &str)],
    body: Option<&[u8]>,
    timeout_secs: u64,
) -> Result<HttpResponse, String> {
    let (host, port, path) = parse_http_url(url)?;
    let addr = format!("{host}:{port}")
        .to_socket_addrs()
        .map_err(|e| format!("resolve {host}:{port}: {e}"))?
        .next()
        .ok_or_else(|| format!("no address for {host}:{port}"))?;

    let timeout = Duration::from_secs(timeout_secs);
    let mut stream = TcpStream::connect_timeout(&addr, timeout)
        .map_err(|e| format!("connect {host}:{port}: {e}"))?;
    let _ = stream.set_read_timeout(Some(timeout));
    let _ = stream.set_write_timeout(Some(timeout));

    let mut req = format!("{method} {path} HTTP/1.1\r\nHost: {host}:{port}\r\nConnection: close\r\n");
    for (k, v) in headers {
        req.push_str(&format!("{k}: {v}\r\n"));
    }
    if let Some(b) = body {
        req.push_str(&format!("Content-Length: {}\r\n", b.len()));
    }
    req.push_str("\r\n");

    stream
        .write_all(req.as_bytes())
        .map_err(|e| format!("write headers: {e}"))?;
    if let Some(b) = body {
        stream.write_all(b).map_err(|e| format!("write body: {e}"))?;
    }
    stream.flush().map_err(|e| format!("flush: {e}"))?;

    let mut raw = Vec::new();
    stream
        .read_to_end(&mut raw)
        .map_err(|e| format!("read response: {e}"))?;

    let split = raw
        .windows(4)
        .position(|w| w == b"\r\n\r\n")
        .ok_or("malformed HTTP response (no header terminator)")?;
    let head_str = String::from_utf8_lossy(&raw[..split]);
    let status = head_str
        .lines()
        .next()
        .and_then(|l| l.split_whitespace().nth(1))
        .and_then(|code| code.parse::<u16>().ok())
        .ok_or("missing HTTP status code")?;

    let is_chunked = head_str
        .lines()
        .any(|l| l.to_ascii_lowercase().starts_with("transfer-encoding:") && l.to_ascii_lowercase().contains("chunked"));

    let body_bytes = if is_chunked {
        decode_chunked(&raw[split + 4..])?
    } else {
        raw[split + 4..].to_vec()
    };

    Ok(HttpResponse {
        status,
        body: String::from_utf8_lossy(&body_bytes).into_owned(),
    })
}

fn request_https_curl(
    method: &str,
    url: &str,
    headers: &[(&str, &str)],
    body: Option<&[u8]>,
    timeout_secs: u64,
) -> Result<HttpResponse, String> {
    let mut cmd = Command::new("curl");
    cmd.arg("-sS")
        .arg("-X")
        .arg(method)
        .arg("--max-time")
        .arg(timeout_secs.to_string())
        .arg("-w")
        .arg("\n%{http_code}");
    for (k, v) in headers {
        cmd.arg("-H").arg(format!("{k}: {v}"));
    }
    if let Some(b) = body {
        cmd.arg("--data-binary")
            .arg(String::from_utf8_lossy(b).into_owned());
    }
    cmd.arg(url);

    let out = cmd.output().map_err(|e| format!("curl exec failed: {e}"))?;
    let stdout = String::from_utf8_lossy(&out.stdout);
    let (body_part, code_part) = stdout.rsplit_once('\n').unwrap_or(("", stdout.trim()));
    let status = code_part.trim().parse::<u16>().unwrap_or(0);
    if status == 0 {
        let stderr = String::from_utf8_lossy(&out.stderr);
        return Err(format!("HTTPS request failed: {}", stderr.trim()));
    }
    Ok(HttpResponse {
        status,
        body: body_part.to_string(),
    })
}

pub fn get(url: &str, headers: &[(&str, &str)], timeout_secs: u64) -> Result<HttpResponse, String> {
    if url.starts_with("http://") {
        request_http_native("GET", url, headers, None, timeout_secs)
    } else if url.starts_with("https://") {
        request_https_curl("GET", url, headers, None, timeout_secs)
    } else {
        Err(format!("unsupported URL scheme: '{url}'"))
    }
}

pub fn post_json(
    url: &str,
    json_body: &str,
    extra_headers: &[(&str, &str)],
    timeout_secs: u64,
) -> Result<HttpResponse, String> {
    let mut hdrs = vec![("Content-Type", "application/json")];
    hdrs.extend_from_slice(extra_headers);
    if url.starts_with("http://") {
        request_http_native("POST", url, &hdrs, Some(json_body.as_bytes()), timeout_secs)
    } else if url.starts_with("https://") {
        request_https_curl("POST", url, &hdrs, Some(json_body.as_bytes()), timeout_secs)
    } else {
        Err(format!("unsupported URL scheme: '{url}'"))
    }
}
