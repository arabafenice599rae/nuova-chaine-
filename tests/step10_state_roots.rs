//! Determinismo delle radici di stato candidate (solo misure, nessuna nel nodo):
//! la radice deve dipendere solo dallo stato, non dalla storia. Merkle sparso
//! (posizione = hash della pubkey) contro Merkle denso (posizione = ordine di
//! creazione), sullo stesso stato raggiunto per strade diverse.

#[path = "../examples/common/merkle.rs"]
#[allow(dead_code)]
mod merkle;
#[path = "../examples/common/smt.rs"]
#[allow(dead_code)]
mod smt;

use std::collections::HashMap;
use std::path::PathBuf;

use centrifuga::genesis::{self, Config, LAMPORTS_PER_CNFA, Prefund, transfer_tx};
use centrifuga::node::Node;
use solana_account::AccountSharedData;
use solana_address::Address;
use solana_keypair::Keypair;
use solana_message::{Message, VersionedMessage};
use solana_signer::Signer;
use solana_transaction::versioned::VersionedTransaction;

const EXAMPLE: &str = include_str!("../centrifuga.example.toml");

fn acc(l: u64) -> AccountSharedData {
    AccountSharedData::new(l, 0, &solana_sdk_ids::system_program::id())
}

#[test]
fn sparse_root_ignores_insertion_order_and_deletions() {
    let keys: Vec<Address> = (0..5_000).map(|_| Address::new_unique()).collect();
    let state: HashMap<Address, AccountSharedData> = keys.iter().enumerate().map(|(i, k)| (*k, acc(1_000 + i as u64))).collect();
    // Tre strade per lo stesso stato: costruzione in blocco, inserimenti in ordine e al
    // contrario; più un account creato, chiuso e ricreato, e uno creato e chiuso.
    let built = smt::Sparse::build(&state).root();
    let mut fwd = smt::Sparse::new();
    let (ghost, again) = (Address::new_unique(), keys[7]);
    for k in &keys {
        fwd.update(&[(*k, state.get(k))]);
    }
    fwd.update(&[(ghost, Some(&acc(5)))]);
    fwd.update(&[(ghost, None)]); // chiuso: come se non fosse mai esistito
    fwd.update(&[(again, None)]);
    fwd.update(&[(again, state.get(&again))]); // ricreato identico
    let mut rev = smt::Sparse::new();
    for k in keys.iter().rev() {
        rev.update(&[(*k, state.get(k))]);
    }
    assert_eq!(fwd.root(), built, "ordine e chiusure non contano");
    assert_eq!(rev.root(), built, "ordine inverso");
    // Prove: appartenenza di un account, assenza di uno chiuso e di uno mai esistito.
    let p = fwd.proof(&keys[3]);
    assert!(smt::verify(&built, &keys[3], state.get(&keys[3]), &p));
    assert!(!smt::verify(&built, &keys[3], Some(&acc(1)), &p), "valore sbagliato rifiutato");
    for k in [ghost, Address::new_unique()] {
        let p = fwd.proof(&k);
        assert!(smt::verify(&built, &k, None, &p), "assenza provata");
        assert!(!smt::verify(&built, &k, Some(&acc(5)), &p), "presenza falsa rifiutata");
    }
    // Riferimento: il Merkle denso dipende dall'ordine di creazione.
    let mut dense = merkle::Merkle::build(&HashMap::new());
    let mut rev_state = state.clone();
    rev_state.insert(Address::new_unique(), acc(1)); // spazio
    let mut dense_rev = merkle::Merkle::build(&HashMap::new());
    for k in &keys {
        dense.update(&[(*k, state.get(k))]);
    }
    for k in keys.iter().rev() {
        dense_rev.update(&[(*k, state.get(k))]);
    }
    assert_ne!(dense.root(), dense_rev.root(), "il denso dipende dall'ordine (solo riferimento)");
}

#[test]
fn sparse_root_same_from_blocks_snapshot_and_journal() {
    let (op, a, b) = (Keypair::new(), Keypair::new(), Keypair::new());
    let mut cfg: Config = toml::from_str(EXAMPLE).unwrap();
    cfg.genesis.operator = op.pubkey().to_string();
    cfg.genesis.network = "devnet".into();
    cfg.genesis.chain_id = "centrifuga:devnet".into();
    for k in [&a, &b] {
        cfg.genesis.prefund.push(Prefund { pubkey: k.pubkey().to_string(), lamports: 100 * LAMPORTS_PER_CNFA });
    }
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("state_roots");
    let _ = std::fs::remove_dir_all(&dir);
    cfg.node.data_dir = dir.clone();
    cfg.node.snapshot_every = 6;
    let kp = || Keypair::try_from(op.to_bytes().as_slice()).unwrap();
    let mut n = Node::open(cfg.clone(), kp()).unwrap();
    let mut live = smt::Sparse::build(n.chain.svm.accounts());
    let mut dense = merkle::Merkle::build(n.chain.svm.accounts());
    let (x, y) = (Keypair::new(), Address::new_unique());
    let t0 = cfg.genesis.timestamp_ms;
    for i in 1..=16u64 {
        let bh = n.chain.last_hash;
        let txs: Vec<VersionedTransaction> = match i % 5 {
            // x nasce, poi viene svuotato a zero (chiuso) da a che paga la fee, poi ricreato.
            1 => vec![transfer_tx(&a, &x.pubkey(), 2 * LAMPORTS_PER_CNFA, bh)],
            2 => {
                let ix = solana_system_interface::instruction::transfer(&x.pubkey(), &y, 2 * LAMPORTS_PER_CNFA);
                let msg = Message::new_with_blockhash(&[ix], Some(&a.pubkey()), &bh);
                vec![VersionedTransaction::try_new(VersionedMessage::Legacy(msg), &[&a, &x]).unwrap()]
            }
            _ => (0..4).map(|j| transfer_tx(if j % 2 == 0 { &a } else { &b }, &Address::new_unique(), 1_000_000 + i, bh)).collect(),
        };
        n.ingress.submit(None, txs);
        let ex = n.tick(t0 + i * 100).unwrap();
        assert!(ex.results.iter().all(|r| r.is_ok()));
        let acc = n.chain.svm.accounts();
        let list: Vec<(Address, Option<&AccountSharedData>)> = ex.changed.iter().map(|k| (*k, acc.get(k))).collect();
        live.update(&list);
        dense.update(&list);
        if i % 5 == 2 {
            assert_eq!(n.chain.svm.lamports(&x.pubkey()), 0, "x chiuso");
        }
    }
    assert!(n.chain.svm.lamports(&x.pubkey()) > 0, "x ricreato");
    let (root, height) = (live.root(), n.chain.height);
    let fresh = smt::Sparse::build(n.chain.svm.accounts()).root();
    assert_eq!(root, fresh, "incrementale = ricostruito dallo stato");
    assert_ne!(dense.root(), merkle::Merkle::build(n.chain.svm.accounts()).root(), "denso: dipende dalla storia (riferimento)");
    drop(n);
    // Dopo il riavvio da snapshot (altezza 12) + journal: stessa radice ricostruita.
    let n = Node::open(cfg.clone(), kp()).unwrap();
    assert_eq!(n.chain.height, height);
    assert_eq!(smt::Sparse::build(n.chain.svm.accounts()).root(), root, "da snapshot + journal");
    drop(n);
    // Riesecuzione completa dal journal su una catena nuova, con albero incrementale.
    std::fs::remove_file(dir.join("snapshot.bin")).unwrap();
    let mut c = genesis::build(&cfg.genesis).unwrap().0;
    let mut replay = smt::Sparse::build(c.svm.accounts());
    let mut file = std::fs::File::open(dir.join("journal.bin")).unwrap();
    let mut off = 0;
    while let Ok(b) = centrifuga::ledger::read_block_at(&mut file, off) {
        off += b.encode().len() as u64 + 4;
        let ex = c.apply(&b).unwrap();
        let list: Vec<(Address, Option<&AccountSharedData>)> = ex.changed.iter().map(|k| (*k, c.svm.get(k))).collect();
        replay.update(&list);
    }
    assert_eq!(c.height, height);
    assert_eq!(replay.root(), root, "riesecuzione del journal");
}
