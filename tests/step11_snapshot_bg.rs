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

/// Snapshot sincrono di riferimento della catena di `n` (in un'altra cartella).
fn reference(n: &Node, name: &str) -> Vec<u8> {
    let d = dir(name);
    std::fs::create_dir_all(&d).unwrap();
    let c = &n.chain;
    ledger::write_snapshot(&d, c.height, c.genesis_hash, c.state_hash(), c.svm.accounts().iter()).unwrap();
    std::fs::read(d.join("snapshot.bin")).unwrap()
}

/// Un account creato, chiuso (saldo a zero: cancellato) e ricreato: gli snapshot dallo
/// stato ombra coincidono con quelli sincroni, con l'account assente e poi di nuovo presente.
#[test]
fn shadow_follows_closed_and_recreated_accounts() {
    let x = Keypair::new_from_array([9; 32]);
    let (u, lamports) = (user(), LAMPORTS_PER_CNFA);
    // Chiusura: il pagatore è u, x firma il trasferimento di tutto il suo saldo.
    let close = |bh| {
        let ix = solana_system_interface::instruction::transfer(&x.pubkey(), &u.pubkey(), lamports);
        let msg = solana_message::Message::new_with_blockhash(&[ix], Some(&u.pubkey()), &bh);
        solana_transaction::versioned::VersionedTransaction::try_new(solana_message::VersionedMessage::Legacy(msg), &[&u, &x]).unwrap()
    };
    let script = |n: &mut Node, t0: u64, i: u64| {
        let bh = n.chain.last_hash;
        let txs = match i {
            1 | 3 => vec![transfer_tx(&u, &x.pubkey(), lamports, bh)], // crea / ricrea
            2 => vec![close(bh)],
            _ => vec![],
        };
        n.ingress.submit(None, txs);
        n.tick(t0 + i * 100).unwrap();
    };
    let (da, dc) = (dir("shadow_close_a"), dir("shadow_close_c"));
    let ca = config(&da, 2);
    let t0 = ca.genesis.timestamp_ms;
    let (mut a, mut c) = (Node::open(ca, op()).unwrap(), Node::open(config(&dc, 0), op()).unwrap());
    for i in 1..=4 {
        script(&mut a, t0, i);
        script(&mut c, t0, i);
        if i % 2 == 0 {
            a.wait_snapshot().unwrap();
            let present = c.chain.svm.get(&x.pubkey()).is_some();
            assert_eq!(present, i == 4, "blocco {i}: x chiuso al 2, ricreato al 3");
            assert_eq!(std::fs::read(da.join("snapshot.bin")).unwrap(), reference(&c, &format!("shadow_close_ref{i}")), "snapshot {i} uguale al sincrono");
        }
    }
    assert_eq!(a.snapshot_counts(), (0, 2));
}

/// Divergenza simulata nello stato ombra: lo snapshot non viene scritto, l'errore è
/// segnalato e resta valido quello precedente. Due casi: account toccato dopo (lo vede il
/// controllo dell'LtHash a ogni blocco) e mai più toccato (lo vede il ricalcolo completo
/// dell'LtHash della copia, prima di scrivere).
#[test]
fn diverged_shadow_does_not_write_and_reports() {
    for (case, k, expected) in [
        ("touched", user().pubkey(), "diverged"),
        ("untouched", centrifuga::genesis::vault_address("airdrop"), "not written"),
    ] {
        let d = dir(&format!("shadow_diverge_{case}"));
        let cfg = config(&d, 5);
        let t0 = cfg.genesis.timestamp_ms;
        let mut n = Node::open(cfg.clone(), op()).unwrap();
        for i in 1..=5 {
            block(&mut n, t0, i);
        }
        n.wait_snapshot().unwrap();
        let before = std::fs::read(d.join("snapshot.bin")).unwrap();
        n.shadow_for_test().unwrap().send_blocking(centrifuga::shadow::Msg::Tamper(k));
        for i in 6..=10 {
            block(&mut n, t0, i);
        }
        let e = n.wait_snapshot().unwrap_err();
        assert!(e.contains(expected), "{case}: {e}");
        assert_eq!(std::fs::read(d.join("snapshot.bin")).unwrap(), before, "{case}: snapshot precedente intatto");
        assert_eq!(n.snapshot_counts().1, 1, "{case}: scritto solo il primo");
        drop(n);
        let n = Node::open(cfg, op()).unwrap(); // snapshot 5 + journal
        assert_eq!(n.chain.height, 10);
    }
}

/// Thread dell'ombra fermo: il canale si riempie, il nodo non aspetta (tick veloci), gli
/// snapshot successivi si saltano e si contano, la memoria resta limitata dal canale.
#[test]
fn slow_shadow_skips_snapshots_without_slowing_the_node() {
    let d = dir("shadow_slow");
    let cfg = config(&d, 10);
    let t0 = cfg.genesis.timestamp_ms;
    let mut n = Node::open(cfg, op()).unwrap();
    n.shadow_for_test().unwrap().send_blocking(centrifuga::shadow::Msg::Sleep(std::time::Duration::from_secs(3)));
    let started = std::time::Instant::now();
    let mut slowest = 0u128;
    let blocks = centrifuga::shadow::QUEUE_BLOCKS as u64 + 100;
    for i in 1..=blocks {
        let t = std::time::Instant::now();
        n.tick(t0 + i * 100).unwrap();
        slowest = slowest.max(t.elapsed().as_millis());
    }
    let elapsed = started.elapsed();
    assert!(elapsed < std::time::Duration::from_secs(3), "il nodo non ha aspettato l'ombra: {elapsed:?}");
    assert!(slowest < 200, "tick più lento {slowest} ms");
    assert!(n.shadow_for_test().unwrap().lost(), "ombra persa");
    let (skipped, _) = n.snapshot_counts();
    assert!(skipped >= 1, "snapshot saltati: {skipped}");
    let e = n.wait_snapshot().unwrap_err();
    assert!(e.contains("lost"), "{e}");
}
