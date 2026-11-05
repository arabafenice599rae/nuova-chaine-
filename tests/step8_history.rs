//! Storico delle tx: in RAM solo una finestra, il resto su disco. getTransaction e
//! getSignaturesForAddress restano corretti per le tx uscite dalla finestra, dopo un
//! riavvio (snapshot + journal, solo journal), dopo un arresto a metà scrittura e dopo
//! la ricostruzione dell'indice delle firme.

use std::path::PathBuf;
use std::time::Instant;

use centrifuga::genesis::{Config, LAMPORTS_PER_CNFA, Prefund, transfer_tx};
use centrifuga::node::Node;
use centrifuga::rpc::methods::call;
use serde_json::{Value, json};
use solana_address::Address;
use solana_keypair::Keypair;
use solana_signature::Signature;
use solana_signer::Signer;

const EXAMPLE: &str = include_str!("../centrifuga.example.toml");

fn rpc(n: &mut Node, m: &str, p: Value) -> Value {
    call(n, m, &p, None, Instant::now()).map_err(|e| e.message).unwrap()
}

fn history(n: &mut Node, a: &Address, extra: Value) -> Vec<String> {
    let mut cfg = json!({"limit": 1000});
    if let Value::Object(m) = extra {
        cfg.as_object_mut().unwrap().extend(m);
    }
    rpc(n, "getSignaturesForAddress", json!([a.to_string(), cfg])).as_array().unwrap().iter().map(|r| r["signature"].as_str().unwrap().to_string()).collect()
}

#[test]
fn history_beyond_ram_window_and_restarts() {
    let (op, a, b) = (Keypair::new(), Keypair::new(), Keypair::new());
    let mut cfg: Config = toml::from_str(EXAMPLE).unwrap();
    cfg.genesis.operator = op.pubkey().to_string();
    cfg.genesis.network = "devnet".into();
    cfg.genesis.chain_id = "centrifuga:devnet".into();
    for k in [&a, &b] {
        cfg.genesis.prefund.push(Prefund { pubkey: k.pubkey().to_string(), lamports: 10 * LAMPORTS_PER_CNFA });
    }
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("history");
    let _ = std::fs::remove_dir_all(&dir);
    cfg.node.data_dir = dir.clone();
    cfg.node.snapshot_every = 7;
    cfg.node.rpc_recent_txs = 5; // finestra minuscola: quasi tutto dal disco
    let kp = || Keypair::try_from(op.to_bytes().as_slice()).unwrap();
    let mut n = Node::open(cfg.clone(), kp()).unwrap();
    let t0 = cfg.genesis.timestamp_ms;
    let to = Address::new_unique();
    // (firma, getTransaction mentre è ancora in RAM), in ordine di esecuzione.
    let mut seen: Vec<(Signature, Value)> = vec![];
    let mut a_sigs = vec![];
    for i in 1..=20u64 {
        let txs: Vec<_> = (0..3).map(|j| transfer_tx(if j == 2 { &b } else { &a }, &to, 1_000 * i + j, n.chain.last_hash)).collect();
        n.ingress.submit(None, txs.clone());
        let ex = n.tick(t0 + i * 100).unwrap();
        assert_eq!(ex.block.txs.len(), 3);
        for t in &txs {
            let s = t.signatures[0];
            seen.push((s, rpc(&mut n, "getTransaction", json!([s.to_string()]))));
            if t.message.static_account_keys()[0] == a.pubkey() {
                a_sigs.push(s.to_string());
            }
        }
        assert!(n.index.recent_len() <= 5, "RAM limitata alla finestra");
    }
    a_sigs.reverse(); // dalla più recente
    let mut all_to: Vec<String> = seen.iter().map(|(s, _)| s.to_string()).collect();
    all_to.reverse();

    let last = seen.last().unwrap().0.to_string();
    let check = |n: &mut Node, what: &str| {
        // L'ultima tx è nella finestra in RAM anche dopo un riavvio (conferma dei wallet).
        assert_eq!(rpc(n, "getSignatureStatuses", json!([[last]]))["value"][0]["slot"], 20, "{what}: stato recente");
        for (s, expected) in &seen {
            assert_eq!(&rpc(n, "getTransaction", json!([s.to_string()])), expected, "{what}: getTransaction {s}");
        }
        assert_eq!(history(n, &a.pubkey(), json!({})), a_sigs, "{what}: cronologia di a");
        assert_eq!(history(n, &to, json!({})), all_to, "{what}: cronologia del destinatario");
        assert_eq!(history(n, &to, json!({"limit": 4})), all_to[..4], "{what}: limit");
        assert_eq!(history(n, &to, json!({"before": all_to[10], "until": all_to[20]})), all_to[11..20], "{what}: before/until");
        // Stati: una tx vecchia è nota solo con searchTransactionHistory, come in Agave.
        let old = seen[0].0.to_string();
        assert!(rpc(n, "getSignatureStatuses", json!([[old]]))["value"][0].is_null(), "{what}");
        let st = rpc(n, "getSignatureStatuses", json!([[old], {"searchTransactionHistory": true}]));
        assert_eq!(st["value"][0]["slot"], 1, "{what}");
        assert_eq!(rpc(n, "getEpochInfo", json!([]))["transactionCount"], 60, "{what}: nessun duplicato");
    };
    check(&mut n, "in esecuzione");
    drop(n);

    // Riavvio da snapshot (altezza 14) + journal.
    let mut n = Node::open(cfg.clone(), kp()).unwrap();
    check(&mut n, "riavvio da snapshot");
    drop(n);
    // Riavvio dal solo journal.
    std::fs::remove_file(dir.join("snapshot.bin")).unwrap();
    let mut n = Node::open(cfg.clone(), kp()).unwrap();
    check(&mut n, "riavvio dal journal");
    drop(n);
    // Arresto a metà scrittura dell'ultimo blocco del registro: si tronca e si riregistra
    // dal journal.
    let log = dir.join("txlog.bin");
    let len = std::fs::metadata(&log).unwrap().len();
    std::fs::OpenOptions::new().write(true).open(&log).unwrap().set_len(len - 10).unwrap();
    let mut n = Node::open(cfg.clone(), kp()).unwrap();
    assert_eq!(std::fs::metadata(&log).unwrap().len(), len, "blocco troncato e riscritto");
    check(&mut n, "dopo un arresto a metà scrittura");
    drop(n);
    // Indice delle firme perso: si ricostruisce dal registro.
    std::fs::remove_file(dir.join("txsig.idx")).unwrap();
    let mut n = Node::open(cfg, kp()).unwrap();
    check(&mut n, "indice ricostruito");
}

#[test]
fn signature_index_grows_without_pauses_and_finds_everything() {
    // Molte firme: l'hash lineare si divide un secchio alla volta.
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("history_big");
    let _ = std::fs::remove_dir_all(&dir);
    let (op, a) = (Keypair::new(), Keypair::new());
    let mut cfg: Config = toml::from_str(EXAMPLE).unwrap();
    cfg.genesis.operator = op.pubkey().to_string();
    cfg.genesis.network = "devnet".into();
    cfg.genesis.chain_id = "centrifuga:devnet".into();
    cfg.genesis.prefund.push(Prefund { pubkey: a.pubkey().to_string(), lamports: 1_000 * LAMPORTS_PER_CNFA });
    cfg.node.data_dir = dir;
    cfg.node.snapshot_every = 0;
    cfg.node.rpc_recent_txs = 0;
    cfg.node.ingress_pending_per_payer = u32::MAX;
    let mut n = Node::open(cfg.clone(), Keypair::try_from(op.to_bytes().as_slice()).unwrap()).unwrap();
    let mut sigs = vec![];
    let mut worst = 0f64;
    for i in 1..=40u64 {
        let txs: Vec<_> = (0..2_500u64).map(|j| transfer_tx(&a, &Address::new_from_array([(j % 251) as u8 + 1; 32]), i * 10_000 + j, n.chain.last_hash)).collect();
        sigs.extend(txs.iter().map(|t| t.signatures[0]));
        n.ingress.submit(None, txs);
        let t = Instant::now();
        n.tick(cfg.genesis.timestamp_ms + i * 100).unwrap();
        worst = worst.max(t.elapsed().as_secs_f64());
    }
    // 100.000 firme, molti split: tutte ritrovabili dal disco.
    for s in sigs.iter().step_by(97) {
        assert_eq!(n.index.get(s).unwrap().tx.signatures[0], *s);
    }
    assert!(n.index.get(&Signature::from([7u8; 64])).is_none());
    assert!(worst < 2.0, "blocco più lento {worst:.2} s");
}
