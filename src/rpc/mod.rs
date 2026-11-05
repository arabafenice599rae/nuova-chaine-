//! Gateway JSON-RPC per i wallet (HTTP) + WebSocket (accountSubscribe,
//! signatureSubscribe). Solo trasporto: i metodi stanno in `methods.rs`.

pub mod encode;
pub mod index;
pub mod methods;
mod ws;

use std::convert::Infallible;
use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::mpsc::Sender;

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
    /// Registra ogni metodo chiamato (per scoprire cosa chiedono i wallet).
    pub trace: bool,
}

impl Gateway {
    /// Esegue `f` sul thread del nodo e ne attende il risultato.
    pub async fn query<T: Send + 'static>(&self, f: impl FnOnce(&mut Node) -> T + Send + 'static) -> Option<T> {
        let (tx, rx) = oneshot::channel();
        self.queries.send(Box::new(move |n| drop(tx.send(f(n))))).ok()?;
        rx.await.ok()
    }

    pub async fn call(&self, req: Value) -> Value {
        let id = req.get("id").cloned().unwrap_or(Value::Null);
        let Some(method) = req["method"].as_str().map(str::to_owned) else {
            return json!({"jsonrpc": "2.0", "id": id, "error": {"code": -32600, "message": "Invalid request"}});
        };
        if self.trace {
            eprintln!("rpc: {method}");
        }
        let params = req.get("params").cloned().unwrap_or(json!([]));
        match self.query(move |n| methods::call(n, &method, &params)).await {
            Some(Ok(v)) => json!({"jsonrpc": "2.0", "id": id, "result": v}),
            Some(Err(e)) => json!({"jsonrpc": "2.0", "id": id, "error": {"code": e.code, "message": e.message, "data": e.data}}),
            None => json!({"jsonrpc": "2.0", "id": id, "error": {"code": -32603, "message": "nodo non disponibile"}}),
        }
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

async fn handle(gw: Gateway, req: Request<Incoming>) -> Result<Response<Full<Bytes>>, Infallible> {
    if req.method() == Method::OPTIONS {
        return Ok(reply(StatusCode::NO_CONTENT, vec![]));
    }
    if req.method() == Method::GET && req.uri().path() == "/health" {
        return Ok(reply(StatusCode::OK, b"ok".to_vec()));
    }
    let Ok(body) = Limited::new(req.into_body(), MAX_BODY).collect().await else {
        return Ok(reply(StatusCode::PAYLOAD_TOO_LARGE, vec![]));
    };
    let out = match serde_json::from_slice::<Value>(&body.to_bytes()) {
        Ok(Value::Array(reqs)) => {
            let mut v = Vec::with_capacity(reqs.len());
            for r in reqs {
                v.push(gw.call(r).await);
            }
            json!(v)
        }
        Ok(r) => gw.call(r).await,
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
        let (s, _) = http.accept().await?;
        let gw = gw.clone();
        tokio::spawn(async move {
            let svc = hyper::service::service_fn(move |r| handle(gw.clone(), r));
            let io = hyper_util::rt::TokioIo::new(s);
            let _ = hyper::server::conn::http1::Builder::new().serve_connection(io, svc).await;
        });
    }
}
