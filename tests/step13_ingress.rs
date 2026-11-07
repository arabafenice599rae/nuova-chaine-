//! Ingresso con pool di verifica: il seq assegnato all'arrivo decide l'ordine di
//! esecuzione; il nodo vede solo il prefisso contiguo di tx verificate e i buchi (tx
//! scartate, coda piena) si saltano subito.

use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Instant;

use centrifuga::genesis::{Config, LAMPORTS_PER_CNFA, Prefund, transfer_tx};
use centrifuga::ingress::{Ingress, Item, Limits, Rejected};
use centrifuga::node::Node;
use solana_address::Address;
use solana_hash::Hash;
use solana_keypair::Keypair;
use solana_signature::Signature;
use solana_signer::Signer;

const EXAMPLE: &str = include_str!("../centrifuga.example.toml");
const CLIENTS: usize = 8;

fn op() -> Keypair {
    Keypair::new_from_array([21; 32])
}

fn client(i: usize) -> Keypair {
    Keypair::new_from_array([30 + i as u8; 32])
}

fn config(name: &str, workers: usize) -> Config {
    let mut cfg: Config = toml::from_str(EXAMPLE).unwrap();
    cfg.genesis.operator = op().pubkey().to_string();
    cfg.genesis.network = "devnet".into();
    cfg.genesis.chain_id = "centrifuga:devnet".into();
    for i in 0..CLIENTS {
        cfg.genesis.prefund.push(Prefund { pubkey: client(i).pubkey().to_string(), lamports: 1_000 * LAMPORTS_PER_CNFA });
    }
    cfg.node.snapshot_every = 0;
    cfg.node.ingress_workers = workers;
    cfg.node.ingress_pending_per_payer = u32::MAX;
    let d = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(name);
    let _ = std::fs::remove_dir_all(&d);
    cfg.node.data_dir = d;
    cfg
}

fn txs(k: &Keypair, n: usize, bh: Hash) -> Vec<solana_transaction::versioned::VersionedTransaction> {
    // Destinatari deterministici: stesse tx in ogni run.
    let to = |i: usize| {
        let mut b = [0u8; 32];
        b[..8].copy_from_slice(&(i as u64).to_le_bytes());
        b[8..16].copy_from_slice(&k.pubkey().to_bytes()[..8]);
        Address::new_from_array(b)
    };
    (0..n).map(|i| transfer_tx(k, &to(i), 1 + i as u64, bh)).collect()
}

/// Produce blocchi finché l'ingresso è vuoto; restituisce le firme nell'ordine di esecuzione.
fn run_all(n: &mut Node) -> Vec<Signature> {
    let t0 = n.cfg.genesis.timestamp_ms;
    let mut out = vec![];
    while !n.ingress.is_empty() {
        let h = n.chain.height + 1;
        out.extend(n.tick(t0 + h * 100).unwrap().block.txs.iter().map(|t| t.signatures[0]));
    }
    out
}

/// 100.000 tx da 8 client in parallelo: l'ordine di esecuzione è l'ordine di seq.
#[test]
fn execution_order_is_arrival_seq_with_parallel_clients() {
    let cfg = config("ingress_order", 4);
    let mut n = Node::open(cfg, op()).unwrap();
    let (ing, bh) = (n.ingress.clone(), n.chain.last_hash);
    let seqs: HashMap<Signature, u64> = std::thread::scope(|s| {
        let hs: Vec<_> = (0..CLIENTS)
            .map(|i| {
                let ing = ing.clone();
                s.spawn(move || {
                    let mut m = vec![];
                    for chunk in txs(&client(i), 100_000 / CLIENTS, bh).chunks(100) {
                        let r = ing.submit(None, chunk.to_vec());
                        m.extend(chunk.iter().zip(r).map(|(t, r)| (t.signatures[0], r.unwrap())));
                    }
                    m
                })
            })
            .collect();
        hs.into_iter().flat_map(|h| h.join().unwrap()).collect()
    });
    let order = run_all(&mut n);
    assert_eq!(order.len(), 100_000);
    let executed: Vec<u64> = order.iter().map(|s| seqs[s]).collect();
    assert!(executed.windows(2).all(|w| w[0] < w[1]), "esecuzione fuori ordine di seq");
}

/// Firma non valida in mezzo: lascia un buco che si salta subito; le valide restano in ordine.
#[test]
fn bad_signature_leaves_a_hole_and_blocks_nothing() {
    let ing = Ingress::new(Limits { tps_per_ip: 1_000, pending_per_payer: u32::MAX, workers: 3, queue: 1024 });
    ing.set_blockhashes([Hash::default()].into());
    let mut all = txs(&client(0), 30, Hash::default());
    for i in [3, 4, 17, 29] {
        all[i].signatures[0] = Signature::from([7; 64]);
    }
    let r = ing.submit(None, all.clone());
    let valid: Vec<Signature> = all.iter().zip(&r).filter(|(_, r)| r.is_ok()).map(|(t, _)| t.signatures[0]).collect();
    assert_eq!(r.iter().filter(|r| **r == Err(Rejected::BadSignature)).count(), 4);
    assert_eq!(valid.len(), 26);
    let drained: Vec<Signature> = ing.drain(1_000).iter().map(|(t, _)| t.signatures[0]).collect();
    assert_eq!(drained, valid, "tutte le valide, nell'ordine di arrivo");
    assert!(ing.is_empty());
    // Dopo i buchi l'ingresso continua: la tx successiva si preleva subito.
    let next = txs(&client(1), 1, Hash::default());
    assert!(ing.submit(None, next.clone())[0].is_ok());
    assert_eq!(ing.drain(10)[0].0.signatures[0], next[0].signatures[0]);
}

/// Coda d'ingresso piena: errore chiaro al client, il seq diventa un buco e nessuna tx
/// successiva resta bloccata.
#[test]
fn full_queue_rejects_clearly_and_blocks_nothing() {
    let ing = Ingress::new(Limits { tps_per_ip: 1_000, pending_per_payer: u32::MAX, workers: 1, queue: 2 });
    ing.set_blockhashes([Hash::default()].into());
    let burst = txs(&client(2), 2_000, Hash::default());
    let rxs: Vec<_> = burst.iter().map(|t| ing.send(None, Item::Tx(t.clone()), Instant::now())).collect();
    let res: Vec<_> = rxs.into_iter().map(|rx| rx.blocking_recv().unwrap()).collect();
    let full = res.iter().filter(|r| **r == Err(Rejected::QueueFull)).count();
    assert!(full > 0, "2.000 tx su una coda da 2: qualcuna rifiutata");
    let accepted: Vec<Signature> = res.iter().filter_map(|r| r.as_ref().ok().map(|(_, s)| *s)).collect();
    assert_eq!(accepted.len() + full, 2_000);
    let next = txs(&client(3), 1, Hash::default());
    assert!(ing.submit(None, next.clone())[0].is_ok());
    let drained: Vec<Signature> = ing.drain(10_000).iter().map(|(t, _)| t.signatures[0]).collect();
    assert_eq!(drained[..accepted.len()], accepted[..], "accettate in ordine di seq");
    assert_eq!(drained.last(), Some(&next[0].signatures[0]), "la tx dopo i buchi non resta bloccata");
}

/// Riesecuzione: stesse tx, stesso ordine di arrivo, con 1 worker e con 4: stessi blocchi
/// e stesso LtHash.
#[test]
fn same_lthash_with_one_or_many_workers() {
    let mut results = vec![];
    for (name, w) in [("ingress_w1", 1), ("ingress_w4", 4)] {
        let mut n = Node::open(config(name, w), op()).unwrap();
        let bh = n.chain.last_hash;
        let all: Vec<_> = (0..CLIENTS).flat_map(|i| txs(&client(i), 1_000, bh)).collect();
        for chunk in all.chunks(250) {
            assert!(n.ingress.submit(None, chunk.to_vec()).iter().all(Result::is_ok));
        }
        let order = run_all(&mut n);
        results.push((order, n.chain.state_hash(), n.chain.last_hash));
    }
    assert_eq!(results[0].0, results[1].0, "stesso ordine di esecuzione");
    assert_eq!(results[0].1, results[1].1, "stesso LtHash");
    assert_eq!(results[0].2, results[1].2, "stesso ultimo blocco");
}
