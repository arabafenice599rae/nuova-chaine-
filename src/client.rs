//! Minimal JSON-RPC client for the CLI: HTTP/1.1 over std::net, HTTPS with rustls
//! (TLS 1.3, Mozilla public roots compiled in). Certificate and host name are always
//! verified. Plain `http://` is accepted only towards localhost / 127.0.0.1.

use std::io::{ErrorKind, Read, Write};
use std::net::TcpStream;
use std::sync::{Arc, OnceLock};
use std::time::Duration;

use rustls::pki_types::ServerName;
use rustls::{ClientConfig, ClientConnection, RootCertStore, StreamOwned};
use serde_json::{Value, json};

pub type Tls = Arc<ClientConfig>;

/// Public CAs (webpki-roots): the only roots the binary uses.
pub fn public_tls() -> Tls {
    static CFG: OnceLock<Tls> = OnceLock::new();
    CFG.get_or_init(|| tls_with(webpki_roots::TLS_SERVER_ROOTS.iter().cloned().collect())).clone()
}

/// TLS configuration with the given roots (tests load a test CA this way).
pub fn tls_with(roots: RootCertStore) -> Tls {
    let provider = Arc::new(rustls::crypto::ring::default_provider());
    let builder = ClientConfig::builder_with_provider(provider).with_safe_default_protocol_versions().expect("TLS 1.3");
    Arc::new(builder.with_root_certificates(roots).with_no_client_auth())
}

/// Where to connect: TLS or not, `host:port`, host name (for SNI and verification), path.
#[derive(Debug, PartialEq)]
pub struct Endpoint {
    pub tls: bool,
    pub addr: String,
    pub host: String,
    pub path: String,
}

fn is_local(host: &str) -> bool {
    host == "localhost" || host == "127.0.0.1"
}

/// Parses the URL and applies the policy, before any connection: without a scheme
/// it is `https://` (`http://` for localhost); `http://` to a remote host is refused.
pub fn endpoint(url: &str) -> Result<Endpoint, String> {
    let (scheme, rest) = match url.split_once("://") {
        Some(("https", r)) => (Some(true), r),
        Some(("http", r)) => (Some(false), r),
        Some((s, _)) => return Err(format!("unsupported scheme {s}://")),
        None => (None, url),
    };
    let (auth, path) = rest.split_once('/').map_or((rest, "/".to_string()), |(a, p)| (a, format!("/{p}")));
    let (host, port) = match auth.rsplit_once(':') {
        Some((h, p)) if !h.ends_with(':') && p.parse::<u16>().is_ok() => (h, Some(p)),
        _ => (auth, None),
    };
    if host.is_empty() {
        return Err(format!("invalid URL: {url}"));
    }
    let tls = scheme.unwrap_or(!is_local(host));
    if !tls && !is_local(host) {
        return Err(format!("refusing {url}: plain http:// is allowed only to localhost or 127.0.0.1, use https://"));
    }
    let port = port.unwrap_or(if tls { "443" } else { "80" });
    Ok(Endpoint { tls, addr: format!("{host}:{port}"), host: host.to_string(), path })
}

pub fn call(url: &str, method: &str, params: Value) -> Result<Value, String> {
    call_with(&public_tls(), url, method, params)
}

pub fn call_with(tls: &Tls, url: &str, method: &str, params: Value) -> Result<Value, String> {
    let ep = endpoint(url)?;
    let body = json!({"jsonrpc": "2.0", "id": 1, "method": method, "params": params}).to_string();
    let req = format!(
        "POST {} HTTP/1.1\r\nHost: {}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        ep.path,
        ep.addr,
        body.len()
    );
    let tcp = TcpStream::connect(&ep.addr).map_err(|e| format!("{}: {e}", ep.addr))?;
    tcp.set_read_timeout(Some(Duration::from_secs(10))).ok();
    tcp.set_write_timeout(Some(Duration::from_secs(10))).ok();
    let resp = if ep.tls {
        let name = ServerName::try_from(ep.host.clone()).map_err(|e| format!("{}: {e}", ep.host))?;
        let conn = ClientConnection::new(tls.clone(), name).map_err(|e| e.to_string())?;
        exchange(&mut StreamOwned::new(conn, tcp), &req)
    } else {
        exchange(&mut { tcp }, &req)
    }
    .map_err(|e| format!("{}: {e}", ep.host))?;
    let resp = String::from_utf8_lossy(&resp);
    let (_, json) = resp.split_once("\r\n\r\n").ok_or("invalid HTTP response")?;
    let v: Value = serde_json::from_str(json).map_err(|e| format!("response: {e}"))?;
    match v.get("error") {
        Some(e) => Err(format!("{method}: {e}")),
        None => Ok(v["result"].clone()),
    }
}

fn exchange(s: &mut (impl Read + Write), req: &str) -> std::io::Result<Vec<u8>> {
    s.write_all(req.as_bytes())?;
    let mut resp = Vec::new();
    match s.read_to_end(&mut resp) {
        // Closed without close_notify: a truncated body fails the JSON parse.
        Err(e) if e.kind() == ErrorKind::UnexpectedEof && !resp.is_empty() => Ok(resp),
        r => r.map(|_| resp),
    }
}
