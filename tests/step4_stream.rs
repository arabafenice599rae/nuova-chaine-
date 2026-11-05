//! Passo 4: stream (replica dallo stream, riconnessione, client lenti) e limiti di ingresso.

use std::path::PathBuf;
use std::time::{Duration, Instant};

use centrifuga::genesis::{Config, LAMPORTS_PER_CNFA, Prefund, transfer_tx};
use centrifuga::ingress::Rejected;
use centrifuga::node::{self, Node};
use centrifuga::rpc::Gateway;
use centrifuga::stream::Replica;
use futures_util::StreamExt;
use serde_json::Value;
use solana_account::ReadableAccount;
use solana_address::Address;
use solana_hash::Hash;
use solana_keypair::Keypair;
use solana_signer::Signer;
use tokio_tungstenite::tungstenite::Message;

const EXAMPLE: &str = include_str!("../centrifuga.example.toml");

fn config(name: &str, port: u16, op: &Keypair, user: &Keypair, f: impl FnOnce(&mut Config)) -> Config {
    let mut cfg: Config = toml::from_str(EXAMPLE).unwrap();
    cfg.genesis.network = "devnet".into();
    cfg.genesis.chain_id = "centrifuga:devnet".into();
    cfg.genesis.operator = op.pubkey().to_string();
    cfg.genesis.timestamp_ms = node::now_ms() - 86_400_000; // la replica rifiuta il futuro
    cfg.genesis.prefund.push(Prefund { pubkey: user.pubkey().to_string(), lamports: 100 * LAMPORTS_PER_CNFA });
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(name);
    let _ = std::fs::remove_dir_all(&dir);
    cfg.node.data_dir = dir;
    cfg.node.rpc_addr = format!("127.0.0.1:{port}");
    cfg.node.stream_addr = format!("127.0.0.1:{}", port + 2);
    cfg.genesis.heartbeat_slots = 1; // qui si testa lo stream: un blocco a ogni slot
    f(&mut cfg);
    cfg
}

fn copy(k: &Keypair) -> Keypair {
    Keypair::try_from(k.to_bytes().as_slice()).unwrap()
}

/// Avvia nodo, RPC e stream; restituisce il gateway per interrogare il nodo.
async fn start(cfg: Config, op: &Keypair) -> Gateway {
    let running = node::start(cfg, copy(op)).unwrap();
    let gw = running.gateway.clone();
    tokio::spawn(running.serve_network());
    tokio::time::sleep(Duration::from_millis(200)).await;
    gw
}

async fn height(gw: &Gateway) -> (u64, Hash) {
    gw.query(|n: &mut Node| (n.chain.height, n.chain.last_hash)).await.unwrap()
}

async fn send_transfer(gw: &Gateway, from: &Keypair, to: Address, lamports: u64) {
    let bh = height(gw).await.1;
    let tx = transfer_tx(from, &to, lamports, bh);
    let r = gw.query(move |n: &mut Node| n.ingress.submit(None, vec![tx])).await.unwrap();
    assert!(r[0].is_ok());
}

type Ws = tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>;

/// Legge messaggi e li applica alla replica finché non raggiunge `until`.
/// Restituisce i tipi di sync ricevuti ("full" / "delta").
async fn follow(ws: &mut Ws, rep: &mut Replica, until: u64) -> Vec<&'static str> {
    let mut syncs = Vec::new();
    while rep.height < until {
        let m = tokio::time::timeout(Duration::from_secs(10), ws.next()).await.expect("stream fermo").unwrap().unwrap();
        let Message::Text(t) = m else { continue };
        let v: Value = serde_json::from_str(&t).unwrap();
        if v["type"] == "sync" {
            syncs.push(if v["full"] == true { "full" } else { "delta" });
        }
        rep.apply(&t).unwrap_or_else(|e| panic!("replica: {e}"));
    }
    syncs
}

#[tokio::test(flavor = "multi_thread")]
async fn replica_rebuilds_state_from_stream_only() {
    let (op, user) = (Keypair::new(), Keypair::new());
    let cfg = config("stream_replica", 19100, &op, &user, |c| {
        c.genesis.slot_ms = 20;
        c.node.stream_catchup_blocks = 200;
    });
    let url = format!("ws://{}/", cfg.node.stream_addr);
    let rules = cfg.genesis.params().unwrap().rules;
    let gw = start(cfg, &op).await;
    let to = Address::new_unique();
    let mut rep = Replica::new(op.pubkey(), rules);

    // 1. Client nuovo: stato completo, poi blocchi live con delta (hash verificato a ogni blocco).
    let (mut ws, _) = tokio_tungstenite::connect_async(&url).await.unwrap();
    send_transfer(&gw, &user, to, LAMPORTS_PER_CNFA).await;
    let h = height(&gw).await.0;
    assert_eq!(follow(&mut ws, &mut rep, h + 5).await, ["full"]);
    drop(ws);

    // 2. Disconnesso per qualche blocco (con tx): riconnessione nella finestra -> delta cumulativo.
    for i in 0..3 {
        send_transfer(&gw, &user, to, LAMPORTS_PER_CNFA + i).await;
        tokio::time::sleep(Duration::from_millis(60)).await;
    }
    let url2 = format!("{url}?from_height={}", rep.height + 1);
    let (mut ws, _) = tokio_tungstenite::connect_async(&url2).await.unwrap();
    let h = height(&gw).await.0;
    assert_eq!(follow(&mut ws, &mut rep, h + 3).await, ["delta"]);
    drop(ws);

    // 3. Fuori dalla finestra (200 blocchi): stato completo.
    let gap = rep.height + 260;
    while height(&gw).await.0 < gap {
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    let url3 = format!("{url}?from_height={}", rep.height + 1);
    let (mut ws, _) = tokio_tungstenite::connect_async(&url3).await.unwrap();
    let h = height(&gw).await.0;
    assert_eq!(follow(&mut ws, &mut rep, h + 3).await, ["full"]);

    // Stesso stato del nodo: hash verificato contro l'header firmato, e saldo atteso.
    let expected = 4 * LAMPORTS_PER_CNFA + 3;
    assert_eq!(rep.accounts()[&to].lamports(), expected);
    let node_balance = gw.query(move |n: &mut Node| n.chain.svm.lamports(&to)).await.unwrap();
    assert_eq!(node_balance, expected);
}

#[tokio::test(flavor = "multi_thread")]
async fn tampered_stream_is_rejected() {
    let (op, user) = (Keypair::new(), Keypair::new());
    let cfg = config("stream_tamper", 19110, &op, &user, |c| c.genesis.slot_ms = 20);
    let url = format!("ws://{}/", cfg.node.stream_addr);
    let rules = cfg.genesis.params().unwrap().rules;
    let _gw = start(cfg, &op).await;
    let (mut ws, _) = tokio_tungstenite::connect_async(&url).await.unwrap();
    let mut rep = Replica::new(op.pubkey(), rules);
    async fn next(ws: &mut Ws) -> String {
        loop {
            if let Some(Ok(Message::Text(t))) = ws.next().await {
                return t.to_string();
            }
        }
    }
    rep.apply(&next(&mut ws).await).unwrap(); // identità della chain
    rep.apply(&next(&mut ws).await).unwrap(); // blocco di ancoraggio
    rep.apply(&next(&mut ws).await).unwrap(); // sync completo
    let good = next(&mut ws).await;
    let mut v: Value = serde_json::from_str(&good).unwrap();
    v["accounts"][0]["lamports"] = Value::from(v["accounts"][0]["lamports"].as_u64().unwrap() + 1);
    let mut other = Replica::new(Keypair::new().pubkey(), rules);
    assert!(other.apply(&good).unwrap_err().contains("invalid signature"), "operatore sbagliato");
    assert!(rep.apply(&v.to_string()).unwrap_err().contains("state hash mismatch"), "delta alterato");
}

#[tokio::test(flavor = "multi_thread")]
async fn slow_client_is_dropped_without_slowing_blocks() {
    let (op, user) = (Keypair::new(), Keypair::new());
    let cfg = config("stream_slow", 19120, &op, &user, |c| {
        c.genesis.slot_ms = 10;
        c.node.stream_max_lag_blocks = 5;
    });
    let addr = cfg.node.stream_addr.clone();
    let gw = start(cfg, &op).await;
    // Un account da 1 MB modificato a ogni blocco: ogni messaggio riempie i buffer TCP.
    let big = Keypair::new();
    let (h, bh) = height(&gw).await;
    let space = 1 << 20;
    let rent = gw.query(move |n: &mut Node| n.chain.svm.rent_exempt(space)).await.unwrap();
    let create = solana_system_interface::instruction::create_account(&user.pubkey(), &big.pubkey(), rent, space as u64, &solana_sdk_ids::system_program::id());
    let msg = solana_message::Message::new_with_blockhash(&[create], Some(&user.pubkey()), &bh);
    let tx = solana_transaction::versioned::VersionedTransaction::try_new(solana_message::VersionedMessage::Legacy(msg), &[&user, &big]).unwrap();
    assert!(gw.query(move |n: &mut Node| n.ingress.submit(None, vec![tx])).await.unwrap()[0].is_ok());
    while height(&gw).await.0 < h + 2 {
        tokio::time::sleep(Duration::from_millis(10)).await;
    }

    // Client che non legge, con buffer di ricezione minimo.
    let sock = tokio::net::TcpSocket::new_v4().unwrap();
    sock.set_recv_buffer_size(4096).unwrap();
    let tcp = sock.connect(addr.parse().unwrap()).await.unwrap();
    let h0 = height(&gw).await.0;
    let (mut ws, _) = tokio_tungstenite::client_async(format!("ws://{addr}/?from_height={}", h0 + 1), tcp).await.unwrap();
    let t0 = Instant::now();
    for i in 0..30 {
        send_transfer(&gw, &user, big.pubkey(), 1 + i).await;
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    // Il nodo è andato avanti ben oltre il ritardo ammesso (5 blocchi) senza aspettare.
    let h1 = height(&gw).await.0;
    assert!(h1 > h0 + 20, "produzione ferma: {h0} -> {h1} in {:?}", t0.elapsed());

    // Il client ora legge: riceve ciò che era nei buffer, poi trova la connessione chiusa.
    let mut last = 0;
    let ended = tokio::time::timeout(Duration::from_secs(20), async {
        while let Some(Ok(m)) = ws.next().await {
            if let Message::Text(t) = m {
                let v: Value = serde_json::from_str(&t).unwrap();
                if v["type"] != "chain" {
                    last = v["header"]["height"].as_u64().or(v["height"].as_u64()).unwrap();
                }
            }
        }
    })
    .await;
    assert!(ended.is_ok(), "il client lento non è stato disconnesso");
    let now = height(&gw).await.0;
    assert!(last + 5 < now, "ultimo blocco ricevuto {last}, nodo a {now}");
}

#[test]
fn ingress_limits_per_ip_and_per_payer() {
    use centrifuga::ingress::{Ingress, Limits};
    let ing = Ingress::new(Limits { tps_per_ip: 5, pending_per_payer: 3 });
    let (a, b) = (Keypair::new(), Keypair::new());
    let tx = |k: &Keypair, i: u64| transfer_tx(k, &Address::new_unique(), i, Hash::default());
    let ip: std::net::IpAddr = "10.0.0.1".parse().unwrap();
    // Per pagatore: al massimo 3 in coda, anche da fonti diverse.
    let r = ing.submit(None, (0..4).map(|i| tx(&a, i)).collect());
    assert_eq!(r[3], Err(Rejected::TooManyPending));
    assert!(r[..3].iter().all(Result::is_ok));
    ing.drain(10);
    assert!(ing.submit(None, vec![tx(&a, 9)])[0].is_ok(), "la coda si libera dopo l'esecuzione");
    // Per IP: burst di 2 secondi (10 tx), poi rifiutate; altri IP non sono toccati.
    let keys: Vec<Keypair> = (0..12).map(|_| Keypair::new()).collect();
    let r = ing.submit(Some(ip), keys.iter().map(|k| tx(k, 1)).collect());
    assert_eq!(r.iter().filter(|x| **x == Err(Rejected::RateLimited)).count(), 2);
    assert!(ing.submit(Some("10.0.0.2".parse().unwrap()), vec![tx(&b, 1)])[0].is_ok());
    // Firma non valida.
    let mut bad = tx(&b, 2);
    bad.signatures[0] = Keypair::new().sign_message(b"x");
    assert_eq!(ing.submit(None, vec![bad])[0], Err(Rejected::BadSignature));
}

#[tokio::test(flavor = "multi_thread")]
async fn balance_cli_prints_token_amount() {
    let (op, user) = (Keypair::new(), Keypair::new());
    let cfg = config("balance_cli", 19130, &op, &user, |c| {
        c.genesis.prefund[0].lamports = 12_500_000_000;
    });
    let url = format!("http://{}", cfg.node.rpc_addr);
    let _gw = start(cfg, &op).await;
    let out = tokio::task::spawn_blocking(move || {
        std::process::Command::new(env!("CARGO_BIN_EXE_centrifuga")).args(["balance", &user.pubkey().to_string(), &url]).output().unwrap()
    })
    .await
    .unwrap();
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    assert_eq!(String::from_utf8_lossy(&out.stdout), "12.5 CnFA\n");
}

#[tokio::test(flavor = "multi_thread")]
async fn node_latency_is_reported_by_get_transaction() {
    let (op, user) = (Keypair::new(), Keypair::new());
    let cfg = config("node_latency", 19140, &op, &user, |c| c.genesis.slot_ms = 50);
    let url = format!("http://{}", cfg.node.rpc_addr);
    let gw = start(cfg, &op).await;
    let tx = transfer_tx(&user, &Address::new_unique(), LAMPORTS_PER_CNFA, height(&gw).await.1);
    let raw = base64::Engine::encode(&base64::engine::general_purpose::STANDARD, wincode::serialize(&tx).unwrap());
    let sig = tx.signatures[0].to_string();
    let v = tokio::task::spawn_blocking(move || {
        use centrifuga::rpc::client::call;
        call(&url, "sendTransaction", serde_json::json!([raw, {"encoding": "base64"}])).unwrap();
        loop {
            std::thread::sleep(Duration::from_millis(20));
            let t = call(&url, "getTransaction", serde_json::json!([sig])).unwrap();
            if !t.is_null() {
                return t;
            }
        }
    })
    .await
    .unwrap();
    let us = v["centrifugaNodeLatencyUs"].as_u64().expect("campo mancante");
    assert!(us > 0 && us < 2_000_000, "{us} µs");
}
