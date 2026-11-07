//! Nodo e storico separati. Il nodo tiene in RAM solo gli stati della finestra dei
//! blockhash e le ultime tx; lo storico è dell'indexer, che legge il journal e
//! riesegue. Il nodo funziona con l'indexer spento; l'indexer recupera da solo e dà
//! le stesse risposte del nodo, anche dopo riavvii, arresti a metà e conservazione.

use std::path::{Path, PathBuf};
use std::time::Instant;

use centrifuga::genesis::{Config, IndexerConfig, LAMPORTS_PER_CNFA, Prefund, transfer_tx};
use centrifuga::indexer::{self, Indexer};
use centrifuga::node::Node;
use centrifuga::rpc::methods::{self, call};
use serde_json::{Value, json};
use solana_address::Address;
use solana_keypair::Keypair;
use solana_signature::Signature;
use solana_signer::Signer;

const EXAMPLE: &str = include_str!("../centrifuga.example.toml");

fn rpc(n: &mut Node, m: &str, p: Value) -> Result<Value, methods::RpcError> {
    call(n, m, &p, None, Instant::now())
}

struct Run {
    cfg: Config,
    op: Keypair,
    /// (firma, getTransaction quando era ancora nel nodo), in ordine di esecuzione.
    seen: Vec<(Signature, Value)>,
    a_sigs: Vec<String>,
    to_sigs: Vec<String>,
    a: Address,
    to: Address,
}

/// Nodo con finestra degli stati di 20 blocchi e 5 tx in RAM; 30 blocchi da 3 tx.
fn run(name: &str) -> Run {
    let (op, a, b) = (Keypair::new(), Keypair::new(), Keypair::new());
    let mut cfg: Config = toml::from_str(EXAMPLE).unwrap();
    cfg.genesis.operator = op.pubkey().to_string();
    cfg.genesis.network = "devnet".into();
    cfg.genesis.chain_id = "centrifuga:devnet".into();
    cfg.genesis.blockhash_window = 20;
    for k in [&a, &b] {
        cfg.genesis.prefund.push(Prefund { pubkey: k.pubkey().to_string(), lamports: 10 * LAMPORTS_PER_CNFA });
    }
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(name);
    let _ = std::fs::remove_dir_all(&dir);
    cfg.node.data_dir = dir.join("node");
    cfg.node.snapshot_every = 7;
    cfg.node.rpc_recent_txs = 5;
    cfg.indexer =
        Some(IndexerConfig { data_dir: dir.join("indexer"), rpc_addr: "127.0.0.1:0".into(), retention_blocks: 0, segment_txs: 10, snapshot_every: 9 });
    let mut n = Node::open(cfg.clone(), Keypair::try_from(op.to_bytes().as_slice()).unwrap()).unwrap();
    let to = Address::new_unique();
    let (mut seen, mut a_sigs) = (vec![], vec![]);
    for i in 1..=30u64 {
        let txs: Vec<_> = (0..3).map(|j| transfer_tx(if j == 2 { &b } else { &a }, &to, 1_000 * i + j, n.chain.last_hash)).collect();
        n.ingress.submit(None, txs.clone());
        assert_eq!(n.tick(cfg.genesis.timestamp_ms + i * 100).unwrap().block.txs.len(), 3);
        for t in &txs {
            let s = t.signatures[0];
            seen.push((s, rpc(&mut n, "getTransaction", json!([s.to_string()])).unwrap()));
            if t.message.static_account_keys()[0] == a.pubkey() {
                a_sigs.push(s.to_string());
            }
        }
        assert!(n.index.recent_len() <= 5, "tx in RAM limitate");
        assert!(n.index.statuses_len() <= 21 * 3, "stati limitati alla finestra");
    }
    a_sigs.reverse();
    let mut to_sigs: Vec<String> = seen.iter().map(|(s, _)| s.to_string()).collect();
    to_sigs.reverse();
    Run { cfg, op, seen, a_sigs, to_sigs, a: a.pubkey(), to }
}

#[test]
fn node_keeps_only_a_bounded_window() {
    let r = run("history_node");
    let mut n = Node::open(r.cfg.clone(), Keypair::try_from(r.op.to_bytes().as_slice()).unwrap()).unwrap();
    let check = |n: &mut Node, what: &str| {
        let (old, last) = (r.seen[0].0.to_string(), r.seen.last().unwrap().0.to_string());
        let st = rpc(n, "getSignatureStatuses", json!([[old, last]])).unwrap();
        assert!(st["value"][0].is_null(), "{what}: stato fuori dalla finestra");
        assert_eq!(st["value"][1]["slot"], 30, "{what}: stato recente");
        assert!(rpc(n, "getTransaction", json!([r.seen[0].0.to_string()])).unwrap().is_null(), "{what}: tx vecchia non nel nodo");
        let e = rpc(n, "getSignaturesForAddress", json!([r.to.to_string()])).unwrap_err();
        assert_eq!(e.code, methods::NO_HISTORY, "{what}");
        assert!(e.message.contains("no indexer"), "{what}: {}", e.message);
        assert_eq!(rpc(n, "getEpochInfo", json!([])).unwrap()["transactionCount"], 90, "{what}");
    };
    check(&mut n, "dopo il riavvio da snapshot (stati da status.bin)");
    // Il nodo non scrive lo storico sul proprio disco.
    let files: Vec<String> = std::fs::read_dir(&r.cfg.node.data_dir).unwrap().flatten().map(|e| e.file_name().into_string().unwrap()).collect();
    assert!(files.iter().all(|f| ["journal.bin", "snapshot.bin", "status.bin"].contains(&f.as_str())), "{files:?}");
}

fn ix_call(ix: &Indexer, m: &str, p: Value) -> Value {
    indexer::call(ix.archive.lock().unwrap().as_ref(), &json!({"jsonrpc": "2.0", "id": 1, "method": m, "params": p}))["result"].clone()
}

fn history(ix: &Indexer, a: &Address, cfg: Value) -> Vec<String> {
    ix_call(ix, "getSignaturesForAddress", json!([a.to_string(), cfg]))
        .as_array()
        .unwrap()
        .iter()
        .map(|x| x["signature"].as_str().unwrap().to_string())
        .collect()
}

#[test]
fn indexer_matches_node() {
    let (storage, r) = ("segments", run("history_ix"));
    let journal = r.cfg.node.data_dir.join("journal.bin");
    let check = |ix: &Indexer, what: &str| {
        for (s, expected) in &r.seen {
            assert_eq!(&ix_call(ix, "getTransaction", json!([s.to_string()])), expected, "{storage} {what}: {s}");
        }
        assert_eq!(history(ix, &r.a, json!({})), r.a_sigs, "{storage} {what}");
        assert_eq!(history(ix, &r.to, json!({})), r.to_sigs, "{storage} {what}");
        assert_eq!(history(ix, &r.to, json!({"limit": 4})), r.to_sigs[..4], "{storage} {what}");
        assert_eq!(history(ix, &r.to, json!({"before": r.to_sigs[10], "until": r.to_sigs[20]})), r.to_sigs[11..20], "{storage} {what}");
        let st = ix_call(ix, "getSignatureStatuses", json!([[r.seen[0].0.to_string()]]));
        assert_eq!(st["value"][0]["slot"], 1, "{storage} {what}");
    };
    // L'indexer parte dopo il nodo (era "spento") e recupera dal journal.
    let ix = Indexer::open(&r.cfg, &journal).unwrap();
    assert_eq!(ix.chain.height, 30);
    check(&ix, "recupero");
    drop(ix);
    // Riavvio dell'indexer (dal suo snapshot + journal): nessun duplicato.
    let ix = Indexer::open(&r.cfg, &journal).unwrap();
    check(&ix, "riavvio");
    drop(ix);
    // Arresto a metà scrittura: coda dell'archivio troncata, poi riregistrata.
    let arch = r.cfg.indexer.as_ref().unwrap().data_dir.join("archive");
    let last = std::fs::read_dir(&arch)
        .unwrap()
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "log"))
        .max_by_key(|p| std::fs::metadata(p).unwrap().modified().unwrap())
        .unwrap();
    let len = std::fs::metadata(&last).unwrap().len();
    std::fs::OpenOptions::new().write(true).open(&last).unwrap().set_len(len.saturating_sub(10)).unwrap();
    let ix = Indexer::open(&r.cfg, &journal).unwrap();
    check(&ix, "dopo un arresto a metà");
}

#[test]
fn retention_drops_old_segments_only() {
    let mut r = run("history_retention");
    r.cfg.indexer.as_mut().unwrap().retention_blocks = 10;
    let ix = Indexer::open(&r.cfg, &r.cfg.node.data_dir.join("journal.bin")).unwrap();
    // Con altezze < 1000 la conservazione non scatta da sola: si applica a mano.
    ix.archive.lock().unwrap().prune(20).unwrap();
    assert!(ix_call(&ix, "getTransaction", json!([r.seen[0].0.to_string()])).is_null(), "vecchia eliminata");
    let last = r.seen.last().unwrap();
    assert_eq!(&ix_call(&ix, "getTransaction", json!([last.0.to_string()])), &last.1, "recente conservata");
    let _ = Path::new("");
}
