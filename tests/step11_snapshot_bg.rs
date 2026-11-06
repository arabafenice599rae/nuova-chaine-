//! Snapshot in background: il thread del nodo copia la mappa degli account e continua;
//! un altro thread scrive. Lo snapshot è quello esatto della sua altezza, con il suo
//! hash di stato (LtHash) nell'header: il nodo lo ricalcola all'avvio e rifiuta un file
//! che non coincide. Un arresto durante la scrittura lascia valido lo snapshot precedente.

use std::path::{Path, PathBuf};

use centrifuga::genesis::{Config, LAMPORTS_PER_CNFA, Prefund, transfer_tx};
use centrifuga::ledger;
use centrifuga::node::Node;
use solana_address::Address;
use solana_keypair::Keypair;
use solana_signer::Signer;

const EXAMPLE: &str = include_str!("../centrifuga.example.toml");

fn op() -> Keypair {
    Keypair::new_from_array([5; 32])
}

fn user() -> Keypair {
    Keypair::new_from_array([6; 32])
}

fn config(dir: &Path, every: u64) -> Config {
    let mut cfg: Config = toml::from_str(EXAMPLE).unwrap();
    cfg.genesis.operator = op().pubkey().to_string();
    cfg.genesis.network = "devnet".into();
    cfg.genesis.chain_id = "centrifuga:devnet".into();
    cfg.genesis.prefund.push(Prefund { pubkey: user().pubkey().to_string(), lamports: 100 * LAMPORTS_PER_CNFA });
    cfg.node.snapshot_every = every;
    cfg.node.data_dir = dir.into();
    cfg
}

fn dir(name: &str) -> PathBuf {
    let d = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(name);
    let _ = std::fs::remove_dir_all(&d);
    d
}

/// Blocco `i` con qualche trasferimento verso destinatari fissi (gli stessi account
/// cambiano a ogni blocco, anche subito dopo lo snapshot).
fn block(n: &mut Node, t0: u64, i: u64) {
    let u = user();
    let txs = (0..4).map(|j| transfer_tx(&u, &Address::new_from_array([j as u8 + 1; 32]), 1_000 * i + j, n.chain.last_hash)).collect();
    n.ingress.submit(None, txs);
    n.tick(t0 + i * 100).unwrap();
}

#[test]
fn background_snapshot_is_the_exact_state_of_its_height() {
    let da = dir("snapbg_a");
    let ca = config(&da, 10);
    let t0 = ca.genesis.timestamp_ms;
    let mut a = Node::open(ca, op()).unwrap();
    // Dopo lo snapshot del 20 il nodo continua subito sugli stessi account, senza aspettare.
    for i in 1..=25 {
        block(&mut a, t0, i);
    }
    a.wait_snapshot().unwrap();
    let s = ledger::read_snapshot(&da).unwrap().unwrap();
    assert_eq!(s.height, 20);
    assert!(a.last_snapshot_us > 0, "pausa misurata");
    // Riferimento: stessa catena fino al 20, snapshot sincrono.
    let mut c = Node::open(config(&dir("snapbg_c"), 0), op()).unwrap();
    for i in 1..=20 {
        block(&mut c, t0, i);
    }
    let cc = &c.chain;
    let dc = dir("snapbg_c_ref");
    std::fs::create_dir_all(&dc).unwrap();
    ledger::write_snapshot(&dc, cc.height, cc.genesis_hash, cc.state_hash(), cc.svm.accounts().iter()).unwrap();
    assert_eq!(std::fs::read(da.join("snapshot.bin")).unwrap(), std::fs::read(dc.join("snapshot.bin")).unwrap(), "stesso file dello snapshot sincrono");
    assert_eq!(s.state_hash, cc.state_hash(), "nell'header l'hash di stato della sua altezza");
    assert_ne!(s.state_hash, a.chain.state_hash(), "i blocchi dopo il 20 non entrano nello snapshot");
}

/// Arresto durante la scrittura: restano `snapshot.tmp` e `status.tmp` parziali. Il nodo
/// riparte dallo snapshot precedente più il journal e arriva allo stesso stato.
#[test]
fn crash_during_write_restarts_from_previous_snapshot_and_journal() {
    let d = dir("snapbg_crash");
    let cfg = config(&d, 10);
    let t0 = cfg.genesis.timestamp_ms;
    let mut n = Node::open(cfg.clone(), op()).unwrap();
    for i in 1..=17 {
        block(&mut n, t0, i);
    }
    n.wait_snapshot().unwrap();
    let end = (n.chain.height, n.chain.last_hash, n.chain.state_hash());
    drop(n);
    assert_eq!(ledger::read_snapshot(&d).unwrap().unwrap().height, 10);
    // Scrittura dello snapshot successivo interrotta a metà.
    let full = std::fs::read(d.join("snapshot.bin")).unwrap();
    std::fs::write(d.join("snapshot.tmp"), &full[..full.len() / 2]).unwrap();
    std::fs::write(d.join("status.tmp"), b"partial").unwrap();
    let n = Node::open(cfg.clone(), op()).unwrap();
    assert_eq!((n.chain.height, n.chain.last_hash, n.chain.state_hash()), end, "snapshot 10 + journal: stesso stato");
    assert_eq!(n.chain.state_hash(), n.chain.full_state_hash());
    drop(n);

    // Arresto tra il rename dello snapshot e quello degli stati: status.bin di un'altra
    // altezza viene ignorato, il nodo riparte comunque con lo stesso stato.
    let mut n = Node::open(cfg.clone(), op()).unwrap();
    for i in 18..=20 {
        block(&mut n, t0, i);
    }
    n.wait_snapshot().unwrap();
    let end = (n.chain.height, n.chain.last_hash, n.chain.state_hash());
    drop(n);
    let old_status = n_status_for(&d, 10);
    std::fs::write(d.join("status.bin"), old_status).unwrap();
    let n = Node::open(cfg, op()).unwrap();
    assert_eq!((n.chain.height, n.chain.last_hash, n.chain.state_hash()), end);
}

/// Stati codificati come per uno snapshot a un'altra altezza (solo l'altezza conta).
fn n_status_for(d: &Path, height: u64) -> Vec<u8> {
    let mut b = std::fs::read(d.join("status.bin")).unwrap();
    b[..8].copy_from_slice(&height.to_le_bytes());
    b
}

/// Uno snapshot modificato (un saldo cambiato, header intatto) viene rifiutato.
#[test]
fn snapshot_with_wrong_state_hash_is_refused() {
    let d = dir("snapbg_tamper");
    let cfg = config(&d, 10);
    let t0 = cfg.genesis.timestamp_ms;
    let mut n = Node::open(cfg.clone(), op()).unwrap();
    for i in 1..=12 {
        block(&mut n, t0, i);
    }
    drop(n);
    let s = ledger::read_snapshot(&d).unwrap().unwrap();
    let mut accounts = s.accounts;
    let k = user().pubkey(); // pagatore prefinanziato: c'è di sicuro
    let a = accounts.iter_mut().find(|(x, _)| *x == k).unwrap();
    let l = solana_account::ReadableAccount::lamports(&a.1);
    solana_account::WritableAccount::set_lamports(&mut a.1, l + 1);
    ledger::write_snapshot(&d, s.height, s.genesis_hash, s.state_hash, accounts.iter().map(|(k, a)| (k, a))).unwrap();
    let e = Node::open(cfg.clone(), op()).err().expect("snapshot modificato rifiutato");
    assert!(e.contains("differs from its header"), "{e}");
    // Formato v1 (senza hash di stato): errore chiaro.
    let mut b = std::fs::read(d.join("snapshot.bin")).unwrap();
    b[..8].copy_from_slice(b"CNFASNP1");
    std::fs::write(d.join("snapshot.bin"), b).unwrap();
    let e = Node::open(cfg, op()).err().unwrap();
    assert!(e.contains("format v1"), "{e}");
}
