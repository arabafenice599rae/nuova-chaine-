//! Finestra del recupero dello stream (`Node.recent`): limite in blocchi e in byte,
//! vale il primo raggiunto. Fuori dalla finestra il client riceve un errore e la
//! replica riparte dallo stato completo; dentro, recupera con il delta.

use std::path::PathBuf;
use std::time::Duration;

use centrifuga::genesis::{Config, LAMPORTS_PER_CNFA, Prefund, transfer_tx};
use centrifuga::node::{self, Node, recent_entry_bytes};
use centrifuga::stream::{OUT_OF_WINDOW, Replica};
use futures_util::StreamExt;
use serde_json::Value;
use solana_account::ReadableAccount;
use solana_address::Address;
use solana_keypair::Keypair;
use solana_signer::Signer;
use tokio_tungstenite::tungstenite::Message;

const EXAMPLE: &str = include_str!("../centrifuga.example.toml");

fn config(name: &str, op: &Keypair, users: &[&Keypair], f: impl FnOnce(&mut Config)) -> Config {
    let mut cfg: Config = toml::from_str(EXAMPLE).unwrap();
    cfg.genesis.network = "devnet".into();
    cfg.genesis.chain_id = "centrifuga:devnet".into();
    cfg.genesis.operator = op.pubkey().to_string();
    cfg.genesis.timestamp_ms = node::now_ms() - 86_400_000; // la replica rifiuta il futuro
    for u in users {
        cfg.genesis.prefund.push(Prefund { pubkey: u.pubkey().to_string(), lamports: 100 * LAMPORTS_PER_CNFA });
    }
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(name);
    let _ = std::fs::remove_dir_all(&dir);
    cfg.node.data_dir = dir;
    f(&mut cfg);
    cfg
}

fn copy(k: &Keypair) -> Keypair {
    Keypair::try_from(k.to_bytes().as_slice()).unwrap()
}

/// Con il limite in byte raggiunto la finestra non cresce più: byte e blocchi restano
/// fermi mentre la chain avanza (stessa memoria con 300 o 900 blocchi prodotti).
#[test]
fn byte_limit_keeps_window_bounded() {
    let (op, user) = (Keypair::new(), Keypair::new());
    let max_bytes = 20 * recent_entry_bytes(20); // ~20 blocchi da 10 tx (20 account cambiati)
    let cfg = config("catchup_bytes", &op, &[&user], |c| {
        c.node.stream_catchup_blocks = 36_000; // il limite in blocchi non si raggiunge
        c.node.stream_catchup_bytes = max_bytes;
        c.node.snapshot_every = 0;
    });
    let t0 = cfg.genesis.timestamp_ms;
    let mut n = Node::open(cfg, copy(&op)).unwrap();
    let mut peak = (0, 0);
    for i in 1..=900u64 {
        let txs = (0..10).map(|j| transfer_tx(&user, &Address::new_unique(), 1 + j, n.chain.last_hash)).collect();
        n.ingress.submit(None, txs);
        n.tick(t0 + i * 100).unwrap();
        assert!(n.recent_bytes <= max_bytes, "blocco {i}: {} byte oltre il limite {max_bytes}", n.recent_bytes);
        let sum: usize = n.recent.iter().map(|r| recent_entry_bytes(r.2.len())).sum();
        assert_eq!(n.recent_bytes, sum, "contatore dei byte coerente");
        if i == 300 {
            peak = (n.recent.len(), n.recent_bytes);
        }
    }
    assert!(peak.0 < 40, "finestra limitata dai byte: {} blocchi", peak.0);
    let end = (n.recent.len(), n.recent_bytes);
    assert!(end.0 <= peak.0 + 1 && end.1 <= max_bytes, "nessuna crescita tra 300 e 900 blocchi: {peak:?} -> {end:?}");
    assert_eq!(n.recent.back().unwrap().0, n.chain.height, "l'ultimo blocco è sempre nella finestra");
}

/// Il limite in blocchi vale anche quando i byte sono lontani dal limite.
#[test]
fn block_limit_still_applies() {
    let op = Keypair::new();
    let cfg = config("catchup_blocks", &op, &[], |c| {
        c.node.stream_catchup_blocks = 25;
        c.node.snapshot_every = 0;
    });
    let t0 = cfg.genesis.timestamp_ms;
    let mut n = Node::open(cfg, copy(&op)).unwrap();
    for i in 1..=100u64 {
        n.tick(t0 + i * 100).unwrap();
    }
    assert_eq!(n.recent.len(), 25);
    assert!(n.recent_bytes < 128 << 20);
}

type Ws = tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>;

async fn next_text(ws: &mut Ws) -> Option<String> {
    loop {
        match tokio::time::timeout(Duration::from_secs(10), ws.next()).await.expect("stream fermo")? {
            Ok(Message::Text(t)) => return Some(t.to_string()),
            Ok(Message::Close(_)) | Err(_) => return None,
            Ok(_) => {}
        }
    }
}

/// Segue lo stream finché la replica arriva a `until`; restituisce i tipi di sync.
async fn follow(ws: &mut Ws, rep: &mut Replica, until: u64) -> Vec<&'static str> {
    let mut syncs = vec![];
    while rep.height < until {
        let t = next_text(ws).await.expect("connessione chiusa");
        let v: Value = serde_json::from_str(&t).unwrap();
        if v["type"] == "sync" {
            syncs.push(if v["full"] == true { "full" } else { "delta" });
        }
        rep.apply(&t).unwrap_or_else(|e| panic!("replica: {e}"));
    }
    syncs
}

async fn connect(url: &str, rep: &Replica) -> Ws {
    let u = match rep.resume_height() {
        0 => url.to_string(),
        h => format!("{url}?from_height={h}"),
    };
    tokio_tungstenite::connect_async(&u).await.unwrap().0
}

/// Dentro la finestra (limitata dai byte) il client recupera con il delta; fuori riceve
/// l'errore, la replica chiede da sé lo stato completo alla connessione successiva e
/// arriva allo stesso stato del nodo.
#[tokio::test(flavor = "multi_thread")]
async fn replica_in_and_out_of_byte_window() {
    let (op, user) = (Keypair::new(), Keypair::new());
    let cfg = config("catchup_stream", &op, &[&user], |c| {
        c.genesis.slot_ms = 20;
        c.genesis.heartbeat_slots = 1;
        c.node.rpc_addr = "127.0.0.1:19200".into();
        c.node.stream_addr = "127.0.0.1:19202".into();
        c.node.stream_catchup_blocks = 36_000;
        c.node.stream_catchup_bytes = 60 * recent_entry_bytes(3); // ~60 blocchi vuoti o pochi con tx
    });
    let url = format!("ws://{}/", cfg.node.stream_addr);
    let rules = cfg.genesis.params().unwrap().rules;
    let running = node::start(cfg, copy(&op)).unwrap();
    let gw = running.gateway.clone();
    tokio::spawn(running.serve_network());
    tokio::time::sleep(Duration::from_millis(200)).await;
    let height = || async { gw.query(|n: &mut Node| n.chain.height).await.unwrap() };
    let send = |lamports: u64, to: Address| {
        let (gw, user) = (gw.clone(), copy(&user));
        async move {
            let bh = gw.query(|n: &mut Node| n.chain.last_hash).await.unwrap();
            let tx = transfer_tx(&user, &to, lamports, bh);
            assert!(gw.query(move |n: &mut Node| n.ingress.submit(None, vec![tx])).await.unwrap()[0].is_ok());
        }
    };
    let to = Address::new_unique();
    let mut rep = Replica::new(op.pubkey(), rules);
    assert_eq!(rep.resume_height(), 0);

    // 1. Primo collegamento: stato completo.
    let mut ws = connect(&url, &rep).await;
    let h = height().await;
    assert_eq!(follow(&mut ws, &mut rep, h + 3).await, ["full"]);
    drop(ws);

    // 2. Dentro la finestra: delta cumulativo, hash verificato contro l'header firmato.
    for i in 0..3 {
        send(LAMPORTS_PER_CNFA + i, to).await;
        tokio::time::sleep(Duration::from_millis(40)).await;
    }
    let mut ws = connect(&url, &rep).await;
    let h = height().await;
    assert_eq!(follow(&mut ws, &mut rep, h + 3).await, ["delta"]);
    drop(ws);

    // 3. Fuori dalla finestra (i byte la riducono a ~60 blocchi): errore chiaro e chiusura.
    let gap = rep.height + 300;
    send(7, to).await;
    while height().await < gap {
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    let oldest = gw.query(|n: &mut Node| n.recent.front().unwrap().0).await.unwrap();
    assert!(rep.resume_height() < oldest, "il punto di ripresa è uscito dalla finestra");
    let mut ws = connect(&url, &rep).await;
    rep.apply(&next_text(&mut ws).await.unwrap()).unwrap(); // identità della chain
    let err = next_text(&mut ws).await.unwrap();
    let v: Value = serde_json::from_str(&err).unwrap();
    assert_eq!(v["type"], "error");
    assert_eq!(v["code"], OUT_OF_WINDOW);
    assert!(v["oldest"].as_u64().unwrap() >= oldest);
    let e = rep.apply(&err).unwrap_err();
    assert!(e.contains(OUT_OF_WINDOW), "{e}");
    assert!(next_text(&mut ws).await.is_none(), "il nodo chiude la connessione");
    assert_eq!(rep.resume_height(), 0, "la replica chiede lo stato completo");

    // 4. Riconnessione decisa dalla replica: stato completo, poi blocchi live verificati.
    let mut ws = connect(&url, &rep).await;
    let h = height().await;
    assert_eq!(follow(&mut ws, &mut rep, h + 3).await, ["full"]);
    assert_eq!(rep.state_hash(), rep.full_state_hash());
    let expected = 3 * LAMPORTS_PER_CNFA + 3 + 7;
    assert_eq!(rep.accounts()[&to].lamports(), expected);
    assert_eq!(gw.query(move |n: &mut Node| n.chain.svm.lamports(&to)).await.unwrap(), expected);
    assert_eq!(rep.resume_height(), rep.height + 1, "dopo lo stato completo riprende dal blocco successivo");
}
