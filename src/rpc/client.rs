//! Client JSON-RPC minimo per la CLI: HTTP/1.1 in chiaro su std::net (solo `http://`).

use std::io::{Read, Write};
use std::net::TcpStream;
use std::time::Duration;

use serde_json::{Value, json};

pub fn call(url: &str, method: &str, params: Value) -> Result<Value, String> {
    let rest = url.strip_prefix("http://").ok_or("solo URL http:// (per https usare un tunnel locale)")?;
    let (host, path) = rest.split_once('/').map_or((rest, "/".to_string()), |(h, p)| (h, format!("/{p}")));
    let body = json!({"jsonrpc": "2.0", "id": 1, "method": method, "params": params}).to_string();
    let mut s = TcpStream::connect(host).map_err(|e| format!("{host}: {e}"))?;
    s.set_read_timeout(Some(Duration::from_secs(10))).ok();
    let req = format!(
        "POST {path} HTTP/1.1\r\nHost: {host}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    s.write_all(req.as_bytes()).map_err(|e| e.to_string())?;
    let mut resp = Vec::new();
    s.read_to_end(&mut resp).map_err(|e| e.to_string())?;
    let resp = String::from_utf8_lossy(&resp);
    let (_, json) = resp.split_once("\r\n\r\n").ok_or("risposta HTTP non valida")?;
    let v: Value = serde_json::from_str(json).map_err(|e| format!("risposta: {e}"))?;
    match v.get("error") {
        Some(e) => Err(format!("{method}: {e}")),
        None => Ok(v["result"].clone()),
    }
}
