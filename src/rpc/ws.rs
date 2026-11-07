//! WebSocket: accountSubscribe e signatureSubscribe (più i relativi unsubscribe).

use std::collections::HashMap;

use futures_util::{SinkExt, StreamExt};
use serde_json::{Value, json};
use solana_address::Address;
use solana_signature::Signature;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::broadcast::error::RecvError;
use tokio_tungstenite::tungstenite::Message;

use super::{Gateway, encode};
use crate::genesis::parse_address;

enum Sub {
    Account(Address, Value),
    Signature(Signature),
}

pub async fn serve(l: TcpListener, gw: Gateway) {
    while let Ok((s, _)) = l.accept().await {
        tokio::spawn(conn(s, gw.clone()));
    }
}

fn notification(method: &str, id: u64, slot: u64, value: Value) -> Message {
    let n = json!({"jsonrpc": "2.0", "method": method, "params": {"result": {"context": {"slot": slot}, "value": value}, "subscription": id}});
    Message::text(n.to_string())
}

/// Valore dell'account com'è ora, codificato secondo la config dell'abbonamento.
async fn account_value(gw: &Gateway, k: Address, c: Value) -> (u64, Value) {
    gw.query(move |n| {
        let v = n.chain.svm.get(&k).map(|a| {
            let slice = c["dataSlice"]["offset"].as_u64().zip(c["dataSlice"]["length"].as_u64()).map(|(o, l)| (o as usize, l as usize));
            encode::account(&n.chain.svm, a, c["encoding"].as_str(), slice).unwrap_or(Value::Null)
        });
        // Account chiuso: Solana notifica un account vuoto.
        let empty =
            json!({"lamports": 0, "owner": "11111111111111111111111111111111", "data": ["", "base64"], "executable": false, "rentEpoch": 0, "space": 0});
        (n.chain.height, v.unwrap_or(empty))
    })
    .await
    .unwrap_or((0, Value::Null))
}

async fn conn(s: TcpStream, gw: Gateway) {
    let Ok(ws) = tokio_tungstenite::accept_async(s).await else { return };
    let (mut tx, mut rx) = ws.split();
    let mut events = gw.events.subscribe();
    let mut subs: HashMap<u64, Sub> = HashMap::new();
    let mut next_id = 0u64;
    loop {
        tokio::select! {
            msg = rx.next() => {
                let req: Value = match msg {
                    Some(Ok(Message::Text(t))) => serde_json::from_str(&t).unwrap_or(Value::Null),
                    Some(Ok(Message::Ping(p))) => { let _ = tx.send(Message::Pong(p)).await; continue }
                    Some(Ok(_)) => continue,
                    _ => return,
                };
                let (id, p) = (req["id"].clone(), &req["params"]);
                let method = req["method"].as_str().unwrap_or("");
                let result: Result<Value, String> = match method {
                    "accountSubscribe" => p[0].as_str().ok_or("pubkey".to_string()).and_then(parse_address).map(|k| {
                        next_id += 1;
                        subs.insert(next_id, Sub::Account(k, p[1].clone()));
                        json!(next_id)
                    }),
                    "signatureSubscribe" => p[0].as_str().and_then(|s| s.parse::<Signature>().ok()).ok_or("signature".to_string()).map(|s| {
                        next_id += 1;
                        subs.insert(next_id, Sub::Signature(s));
                        json!(next_id)
                    }),
                    "accountUnsubscribe" | "signatureUnsubscribe" => Ok(json!(p[0].as_u64().and_then(|i| subs.remove(&i)).is_some())),
                    _ => Err("Method not found".into()),
                };
                gw.log("ws", method, match &result {
                    Ok(_) => "ok",
                    Err(e) if e == "Method not found" => "UNSUPPORTED",
                    Err(_) => "error",
                });
                let out = match result {
                    Ok(v) => json!({"jsonrpc": "2.0", "id": id, "result": v}),
                    Err(e) => json!({"jsonrpc": "2.0", "id": id, "error": {"code": -32601, "message": e}}),
                };
                if tx.send(Message::text(out.to_string())).await.is_err() {
                    return;
                }
                // Firma già eseguita prima dell'abbonamento: notifica subito.
                if method == "signatureSubscribe"
                    && let Some(Sub::Signature(s)) = subs.get(&next_id) {
                        let s = *s;
                        let done = gw.query(move |n| n.index.status(&s).map(|(slot, err)| (*slot, json!(err)))).await.flatten();
                        if let Some((slot, err)) = done {
                            subs.remove(&next_id);
                            let _ = tx.send(notification("signatureNotification", next_id, slot, json!({"err": err}))).await;
                        }
                    }
            }
            ev = events.recv() => {
                let ev = match ev {
                    Ok(ev) => ev,
                    Err(RecvError::Lagged(_)) => continue,
                    Err(RecvError::Closed) => return,
                };
                let mut out = Vec::new();
                let mut done = Vec::new();
                for (id, sub) in &subs {
                    match sub {
                        Sub::Account(k, c) if ev.changed.contains(k) => {
                            let (slot, v) = account_value(&gw, *k, c.clone()).await;
                            out.push(notification("accountNotification", *id, slot, v));
                        }
                        Sub::Signature(s) => {
                            if let Some((_, err)) = ev.sigs.iter().find(|(x, _)| x == s) {
                                out.push(notification("signatureNotification", *id, ev.slot, json!({"err": err})));
                                done.push(*id);
                            }
                        }
                        _ => {}
                    }
                }
                for id in done {
                    subs.remove(&id);
                }
                for m in out {
                    if tx.send(m).await.is_err() {
                        return;
                    }
                }
            }
        }
    }
}
