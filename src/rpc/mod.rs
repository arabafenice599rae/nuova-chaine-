//! Gateway JSON-RPC per i wallet (HTTP) + WebSocket (accountSubscribe,
//! signatureSubscribe). Solo trasporto: i metodi stanno in `methods.rs`.

pub mod client;
pub mod encode;
pub mod index;
pub mod methods;
mod ws;

use std::convert::Infallible;
use std::fs::File;
use std::io::Write;
use std::net::{IpAddr, SocketAddr};
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex};
use std::time::Instant;

use http_body_util::{BodyExt, Full, Limited};
use hyper::body::{Bytes, Incoming};
use hyper::{Method, Request, Response, StatusCode};
use serde_json::{Value, json};
use solana_address::Address;
use solana_signature::Signature;
use solana_transaction::TransactionError;
use tokio::sync::{broadcast, oneshot};

use crate::exec::Executed;
use crate::node::{Node, Query};

const MAX_BODY: usize = 1 << 20;

/// Evento di blocco per gli abbonati WebSocket (e, in futuro, lo stream).
pub struct BlockEvent {
    pub slot: u64,
    pub changed: Vec<Address>,
    pub sigs: Vec<(Signature, Option<TransactionError>)>,
}

impl BlockEvent {
    pub fn new(ex: &Executed) -> BlockEvent {
        BlockEvent {
            slot: ex.block.header.height,
            changed: ex.changed.iter().copied().collect(),
            sigs: ex.block.txs.iter().zip(&ex.results).map(|(t, r)| (t.signatures[0], r.as_ref().err().map(|f| f.err.clone()))).collect(),
        }
    }
}

#[derive(Clone)]
pub struct Gateway {
    pub queries: Sender<Query>,
    pub events: broadcast::Sender<Arc<BlockEvent>>,
    /// Log dei metodi ricevuti (per scoprire cosa chiedono i wallet): `rpc.log`.
    pub log: Option<Arc<Mutex<File>>>,
    /// Indexer dello storico (URL http): il nodo gli inoltra le richieste sullo storico.
    pub indexer: Option<String>,
}

/// POST JSON-RPC verso l'indexer (http://host:porta), sui thread di rete: il thread del
/// nodo non aspetta mai l'indexer.
async fn forward(url: &str, req: &Value) -> Result<Value, String> {
    let host = url.strip_prefix("http://").ok_or("indexer_rpc: solo http://")?.trim_end_matches('/');
    let s = tokio::time::timeout(std::time::Duration::from_secs(2), tokio::net::TcpStream::connect(host))
        .await
        .map_err(|_| "timeout")?
        .map_err(|e| e.to_string())?;
    let (mut tx, conn) = hyper::client::conn::http1::handshake(hyper_util::rt::TokioIo::new(s)).await.map_err(|e| e.to_string())?;
    tokio::spawn(conn);
    let r = Request::post("/").header("host", host).header("content-type", "application/json").body(Full::new(Bytes::from(req.to_string()))).unwrap();
    let resp = tokio::time::timeout(std::time::Duration::from_secs(5), tx.send_request(r)).await.map_err(|_| "timeout")?.map_err(|e| e.to_string())?;
    let body = resp.into_body().collect().await.map_err(|e| e.to_string())?.to_bytes();
    serde_json::from_slice(&body).map_err(|e| e.to_string())
}

/// Esito di una risposta JSON-RPC per il log.
fn outcome(resp: &Value) -> String {
    match resp["error"]["code"].as_i64() {
        None => "ok".into(),
        Some(-32601) => "UNSUPPORTED".into(),
        Some(c) => format!("error {c}"),
    }
}

impl Gateway {
    /// Esegue `f` sul thread del nodo e ne attende il risultato.
    pub async fn query<T: Send + 'static>(&self, f: impl FnOnce(&mut Node) -> T + Send + 'static) -> Option<T> {
        let (tx, rx) = oneshot::channel();
        self.queries.send(Box::new(move |n| drop(tx.send(f(n))))).ok()?;
        rx.await.ok()
    }

    /// Una riga per richiesta: `unix_ms canale metodo esito`.
    pub fn log(&self, channel: &str, method: &str, outcome: &str) {
        if let Some(f) = &self.log {
            let _ = writeln!(f.lock().unwrap(), "{} {channel} {method} {outcome}", crate::node::now_ms());
        }
    }

    pub async fn call(&self, req: Value, peer: Option<IpAddr>, arrived: Instant) -> Value {
        let id = req.get("id").cloned().unwrap_or(Value::Null);
        let Some(method) = req["method"].as_str().map(str::to_owned) else {
            return json!({"jsonrpc": "2.0", "id": id, "error": {"code": -32600, "message": "Invalid request"}});
        };
        let resp = self.call_node(&req, &method, id.clone(), peer, arrived).await;
        let Some(ix) = &self.indexer else { return resp };
        // Storico: la cronologia sempre dall'indexer; tx e stati non più nella finestra.
        let history = req["params"][1]["searchTransactionHistory"].as_bool() == Some(true);
        let missing = match method.as_str() {
            "getSignaturesForAddress" => true,
            "getTransaction" => resp["result"].is_null() && resp.get("error").is_none(),
            "getSignatureStatuses" => history && resp["result"]["value"].as_array().is_some_and(|v| v.iter().any(Value::is_null)),
            _ => false,
        };
        if !missing {
            return resp;
        }
        let from_ix = forward(ix, &req).await;
        self.log("indexer", &method, if from_ix.is_ok() { "ok" } else { "irraggiungibile" });
        match (method.as_str(), from_ix) {
            ("getSignatureStatuses", Ok(v)) => {
                let mut resp = resp;
                if let (Some(mine), Some(theirs)) = (resp["result"]["value"].as_array_mut(), v["result"]["value"].as_array()) {
                    for (m, t) in mine.iter_mut().zip(theirs) {
                        if m.is_null() {
                            *m = t.clone();
                        }
                    }
                }
                resp
            }
            (_, Ok(mut v)) => {
                v["id"] = id;
                v
            }
            (_, Err(e)) => {
                json!({"jsonrpc": "2.0", "id": id, "error": {"code": methods::NO_HISTORY, "message": format!("transaction history temporarily unavailable: indexer unreachable ({e})")}})
            }
        }
    }

    async fn call_node(&self, req: &Value, method: &str, id: Value, peer: Option<IpAddr>, arrived: Instant) -> Value {
        let method = method.to_string();
        let params = req.get("params").cloned().unwrap_or(json!([]));
        let m = method.clone();
        let resp = match self.query(move |n| methods::call(n, &m, &params, peer, arrived)).await {
            Some(Ok(v)) => json!({"jsonrpc": "2.0", "id": id, "result": v}),
            Some(Err(e)) => {
                // `data: null` manda in errore web3.js: si omette se vuoto.
                let mut err = json!({"code": e.code, "message": e.message});
                if !e.data.is_null() {
                    err["data"] = e.data;
                }
                json!({"jsonrpc": "2.0", "id": id, "error": err})
            }
            None => json!({"jsonrpc": "2.0", "id": id, "error": {"code": -32603, "message": "node unavailable"}}),
        };
        self.log("http", &method, &outcome(&resp));
        resp
    }
}

fn reply(status: StatusCode, body: Vec<u8>) -> Response<Full<Bytes>> {
    Response::builder()
        .status(status)
        .header("content-type", "application/json")
        .header("access-control-allow-origin", "*")
        .header("access-control-allow-methods", "POST, GET, OPTIONS")
        .header("access-control-allow-headers", "*")
        .body(Full::new(Bytes::from(body)))
        .unwrap()
}

async fn handle(gw: Gateway, peer: IpAddr, req: Request<Incoming>) -> Result<Response<Full<Bytes>>, Infallible> {
    let arrived = Instant::now();
    if req.method() == Method::OPTIONS {
        return Ok(reply(StatusCode::NO_CONTENT, vec![]));
    }
    // Dietro un reverse proxy locale (HTTPS) il mittente reale è in X-Forwarded-For.
    let peer = match req.headers().get("x-forwarded-for").and_then(|v| v.to_str().ok()) {
        Some(xff) if peer.is_loopback() => xff.split(',').next().and_then(|ip| ip.trim().parse().ok()).unwrap_or(peer),
        _ => peer,
    };
    if req.method() == Method::GET && req.uri().path() == "/health" {
        return Ok(reply(StatusCode::OK, b"ok".to_vec()));
    }
    if req.method() == Method::GET && req.uri().path() == "/chain.json" {
        return Ok(match gw.query(|n| methods::chain_info(n)).await {
            Some(v) => reply(StatusCode::OK, serde_json::to_vec(&v).unwrap()),
            None => reply(StatusCode::SERVICE_UNAVAILABLE, vec![]),
        });
    }
    let Ok(body) = Limited::new(req.into_body(), MAX_BODY).collect().await else {
        return Ok(reply(StatusCode::PAYLOAD_TOO_LARGE, vec![]));
    };
    let out = match serde_json::from_slice::<Value>(&body.to_bytes()) {
        Ok(Value::Array(reqs)) => {
            let mut v = Vec::with_capacity(reqs.len());
            for r in reqs {
                v.push(gw.call(r, Some(peer), arrived).await);
            }
            json!(v)
        }
        Ok(r) => gw.call(r, Some(peer), arrived).await,
        Err(_) => json!({"jsonrpc": "2.0", "id": null, "error": {"code": -32700, "message": "Parse error"}}),
    };
    Ok(reply(StatusCode::OK, serde_json::to_vec(&out).unwrap()))
}

/// HTTP su `addr`, WebSocket su porta + 1 (convenzione dei client Solana).
pub async fn serve(gw: Gateway, addr: SocketAddr) -> std::io::Result<()> {
    let http = tokio::net::TcpListener::bind(addr).await?;
    let ws = tokio::net::TcpListener::bind(SocketAddr::new(addr.ip(), addr.port() + 1)).await?;
    eprintln!("rpc: http://{addr}  ws://{}:{}", addr.ip(), addr.port() + 1);
    tokio::spawn(ws::serve(ws, gw.clone()));
    loop {
        let (s, peer) = http.accept().await?;
        let gw = gw.clone();
        tokio::spawn(async move {
            let svc = hyper::service::service_fn(move |r| handle(gw.clone(), peer.ip(), r));
            let io = hyper_util::rt::TokioIo::new(s);
            let _ = hyper::server::conn::http1::Builder::new().serve_connection(io, svc).await;
        });
    }
}
