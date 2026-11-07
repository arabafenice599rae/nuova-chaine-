//! Journal su un thread separato (group commit). Invarianti:
//! 1. un blocco va su RPC, stream e conferme solo dopo che è durevole;
//! 2. lo snapshot non supera mai l'altezza durevole;
//! 3. crash con blocchi eseguiti e non durevoli: al riavvio si riparte dall'ultimo blocco
//!    durevole, nessuna conferma persa, nessun blocco doppio alla stessa altezza.

use std::path::{Path, PathBuf};

use centrifuga::genesis::{Config, LAMPORTS_PER_CNFA, Prefund, transfer_tx};
use centrifuga::ledger;
use centrifuga::node::Node;
use serde_json::json;
use solana_address::Address;
use solana_keypair::Keypair;
use solana_signature::Signature;
use solana_signer::Signer;

const EXAMPLE: &str = include_str!("../centrifuga.example.toml");

fn op() -> Keypair {
    Keypair::new_from_array([11; 32])
}

fn user() -> Keypair {
    Keypair::new_from_array([12; 32])
}

fn config(name: &str, every: u64) -> Config {
    let mut cfg: Config = toml::from_str(EXAMPLE).unwrap();
    cfg.genesis.operator = op().pubkey().to_string();
    cfg.genesis.network = "devnet".into();
    cfg.genesis.chain_id = "centrifuga:devnet".into();
    cfg.genesis.prefund.push(Prefund { pubkey: user().pubkey().to_string(), lamports: 100 * LAMPORTS_PER_CNFA });
    cfg.node.snapshot_every = every;
    let d = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(name);
    let _ = std::fs::remove_dir_all(&d);
    cfg.node.data_dir = d;
    cfg
}

/// Accoda un trasferimento e produce il blocco (senza aspettare la durabilità).
fn produce(n: &mut Node, t0: u64, i: u64) -> Signature {
    let tx = transfer_tx(&user(), &Address::new_from_array([7; 32]), LAMPORTS_PER_CNFA + i, n.chain.last_hash);
    let sig = tx.signatures[0];
    n.ingress.submit(None, vec![tx]);
    n.produce(t0 + i * 100).unwrap();
    sig
}

fn rpc(n: &mut Node, method: &str) -> serde_json::Value {
    centrifuga::rpc::methods::call(n, method, &json!([]), None, std::time::Instant::now()).unwrap()
}

fn status(n: &mut Node, s: &Signature) -> serde_json::Value {
    centrifuga::rpc::methods::call(n, "getSignatureStatuses", &json!([[s.to_string()]]), None, std::time::Instant::now()).unwrap()["value"][0].clone()
}

#[test]
fn block_published_only_after_durable() {
    let cfg = config("jt_publish", 0);
    let t0 = cfg.genesis.timestamp_ms;
    let mut n = Node::open(cfg, op()).unwrap();
    n.tick(t0 + 100).unwrap();
    let durable = (n.durable_height, n.durable_hash);
    n.hold_journal_for_test(true); // disco fermo
    let sig = produce(&mut n, t0, 2);
    assert_eq!(n.chain.height, 2, "eseguito");
    std::thread::sleep(std::time::Duration::from_millis(50));
    assert!(n.publish(false).unwrap().is_empty(), "niente pubblicato prima della durabilità");
    assert_eq!((n.durable_height, n.durable_hash), durable);
    assert_eq!(rpc(&mut n, "getSlot"), json!(1), "getSlot: altezza durevole");
    assert_eq!(rpc(&mut n, "getBlockHeight"), json!(1));
    assert_eq!(rpc(&mut n, "getLatestBlockhash")["value"]["blockhash"], json!(durable.1.to_string()));
    assert!(status(&mut n, &sig).is_null(), "nessuno stato per una tx non durevole");
    n.hold_journal_for_test(false);
    let done = n.publish(true).unwrap();
    assert_eq!(done.len(), 1);
    assert_eq!(n.durable_height, 2);
    assert_eq!(rpc(&mut n, "getSlot"), json!(2));
    let st = status(&mut n, &sig);
    assert_eq!(st["slot"], json!(2));
    assert_eq!(st["confirmationStatus"], json!("finalized"));
}

#[test]
fn snapshot_never_beyond_durable_height() {
    let cfg = config("jt_snapshot", 2);
    let t0 = cfg.genesis.timestamp_ms;
    let dir = cfg.node.data_dir.clone();
    let mut n = Node::open(cfg, op()).unwrap();
    let snap = |d: &Path| ledger::read_snapshot(d).unwrap().map_or(0, |s| s.height);
    let q = n.queue_blocks() as u64;
    n.hold_journal_for_test(true);
    for i in 1..=q {
        produce(&mut n, t0, i);
        n.publish(false).unwrap();
        assert_eq!(snap(&dir), 0, "blocco {i}: nessuno snapshot oltre l'altezza durevole {}", n.durable_height);
    }
    n.hold_journal_for_test(false);
    n.publish(true).unwrap();
    n.wait_snapshot().unwrap(); // lo snapshot lo scrive lo stato ombra
    assert_eq!(n.durable_height, q);
    let h = snap(&dir);
    assert!(h >= 2 && h <= n.durable_height, "snapshot {h}, durevole {}", n.durable_height);
    assert_eq!(h, n.chain.height, "scritto a coda vuota: stato e altezza coincidono");
}

#[test]
fn crash_with_blocks_queued_loses_no_confirmation_and_no_height_twice() {
    let cfg = config("jt_crash", 0);
    let t0 = cfg.genesis.timestamp_ms;
    let mut n = Node::open(cfg.clone(), op()).unwrap();
    let mut confirmed = vec![];
    for i in 1..=2 {
        confirmed.push(produce(&mut n, t0, i));
        n.publish(true).unwrap();
    }
    let q = n.queue_blocks() as u64;
    n.hold_journal_for_test(true);
    let lost: Vec<_> = (3..=2 + q).map(|i| produce(&mut n, t0, i)).collect();
    assert_eq!(n.chain.height, 2 + q, "{q} blocchi eseguiti e in coda");
    assert_eq!(n.durable_height, 2);
    for s in &lost {
        assert!(status(&mut n, s).is_null(), "mai confermata");
    }
    n.crash_for_test();

    let mut n = Node::open(cfg.clone(), op()).unwrap();
    assert_eq!(n.chain.height, 2, "riparte dall'ultimo blocco durevole");
    for s in &confirmed {
        assert_eq!(status(&mut n, s)["confirmationStatus"], json!("finalized"), "nessuna conferma persa");
    }
    for s in &lost {
        assert!(status(&mut n, s).is_null());
    }
    for i in 3..=5 {
        produce(&mut n, t0 + 10_000, i);
        n.publish(true).unwrap();
    }
    drop(n);
    let mut heights = vec![];
    ledger::Journal::open(&cfg.node.data_dir.join("journal.bin"), |b, _| {
        heights.push(b.header.height);
        Ok(())
    })
    .unwrap();
    let mut sorted = heights.clone();
    sorted.dedup();
    assert_eq!(sorted, heights, "nessuna altezza due volte: {heights:?}");
    assert!(heights.windows(2).all(|w| w[0] < w[1]), "{heights:?}");
}
