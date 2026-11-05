//! Confronto tra tre hash di stato sugli stessi blocchi reali del test di carico:
//!   A. LtHash a ogni blocco (quello nel nodo);
//!   B. Merkle incrementale a ogni blocco, con prove per account;
//!   C. ibrido: LtHash a ogni blocco + radice Merkle firmata ogni secondo, calcolata
//!      su un thread separato con i soli account cambiati in quel secondo.
//!   cargo run --release --example compare -- <accounts> [blocks] [txs_per_block]
//! Il nodo esegue i blocchi con LtHash. Per B il costo di LtHash è tolto e sostituito
//! da quello dell'aggiornamento Merkle sugli stessi account cambiati. C gira in un
//! secondo passaggio sugli stessi blocchi, con il thread Merkle davvero in esecuzione
//! (compete per la CPU con il nodo).

#[path = "common/merkle.rs"]
mod merkle;

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::mpsc;
use std::time::Instant;

use centrifuga::exec::state_hash::account_lt_hash;
use centrifuga::genesis::{self, Config, LAMPORTS_PER_CNFA, Prefund, transfer_tx};
use centrifuga::node::Node;
use merkle::Merkle;
use solana_account::{Account, AccountSharedData};
use solana_address::Address;
use solana_keypair::Keypair;
use solana_lattice_hash::lt_hash::LtHash;
use solana_signer::Signer;

const EXAMPLE: &str = include_str!("../centrifuga.example.toml");
const SENDERS: usize = 2_000;
/// Blocchi per secondo con slot da 100 ms: la radice dell'ibrido.
const PER_ROOT: usize = 10;

fn rss_mb() -> u64 {
    let s = std::fs::read_to_string("/proc/self/status").unwrap_or_default();
    s.lines().find(|l| l.starts_with("VmRSS:")).and_then(|l| l.split_whitespace().nth(1)?.parse::<u64>().ok()).unwrap_or(0) / 1024
}

fn addr(i: u64) -> Address {
    let mut b = [0u8; 32];
    b[..8].copy_from_slice(&i.to_le_bytes());
    b[8] = 0xCA;
    Address::new_from_array(b)
}

fn pct(v: &[f64], q: f64) -> f64 {
    let mut s = v.to_vec();
    s.sort_by(f64::total_cmp);
    s[((s.len() - 1) as f64 * q).round() as usize] * 1e3
}

fn mean(v: &[f64]) -> f64 {
    v.iter().sum::<f64>() / v.len() as f64
}

struct Row {
    name: &'static str,
    block: Vec<f64>,
}

fn main() {
    let args: Vec<u64> = std::env::args().skip(1).map(|a| a.replace('_', "").parse().expect("number")).collect();
    let (n, blocks, per_block) = (args[0], *args.get(1).unwrap_or(&100) as usize, *args.get(2).unwrap_or(&5_000));
    let op = Keypair::new();
    let senders: Vec<Keypair> = (0..SENDERS).map(|_| Keypair::new()).collect();
    let mut cfg: Config = toml::from_str(EXAMPLE).unwrap();
    cfg.genesis.operator = op.pubkey().to_string();
    cfg.genesis.network = "devnet".into();
    cfg.genesis.chain_id = "centrifuga:devnet".into();
    cfg.genesis.timestamp_ms = centrifuga::node::now_ms() - 86_400_000;
    cfg.genesis.prefund = senders.iter().map(|k| Prefund { pubkey: k.pubkey().to_string(), lamports: 1_000 * LAMPORTS_PER_CNFA }).collect();
    cfg.node.max_block_txs = per_block as usize;
    cfg.node.snapshot_every = 0;
    cfg.node.ingress_pending_per_payer = u32::MAX;
    let dir = std::env::var("LOAD_DIR").map(PathBuf::from).unwrap_or_else(|_| std::env::temp_dir().join("centrifuga-compare"));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    cfg.node.data_dir = dir.clone();

    let mut chain = genesis::build(&cfg.genesis).unwrap().0;
    let rent = chain.svm.rent_exempt(0);
    let system = solana_sdk_ids::system_program::id();
    for i in 0..n {
        chain.svm.set(addr(i), Account { lamports: rent, owner: system, ..Account::default() });
    }
    chain.rehash();
    centrifuga::ledger::write_snapshot(&dir, 0, chain.genesis_hash, chain.svm.accounts().iter()).unwrap();
    let snap_mb = std::fs::metadata(centrifuga::ledger::snapshot_path(&dir)).unwrap().len() as f64 / 1e6;
    drop(chain);
    let kp = || Keypair::try_from(op.to_bytes().as_slice()).unwrap();

    // Riavvio con LtHash (lettura snapshot + LtHash completo).
    let t = Instant::now();
    let mut node = Node::open(cfg.clone(), kp()).unwrap();
    let restart_lt = t.elapsed().as_secs_f64();
    let bh = node.chain.last_hash;
    let mut seed = 0x9E37_79B9_7F4A_7C15u64;
    let mut rnd = || {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        seed
    };
    let all: Vec<Vec<_>> = (0..blocks as u64)
        .map(|b| (0..per_block).map(|j| transfer_tx(&senders[(j as usize) % SENDERS], &addr(rnd() % n.max(1)), 1 + b * per_block + j, bh)).collect())
        .collect();

    // Albero di Merkle (B e C): costruzione = costo in più al riavvio; RAM in più.
    let r0 = rss_mb();
    let t = Instant::now();
    let mut mk = Merkle::build(node.chain.svm.accounts());
    let mk_build = t.elapsed().as_secs_f64();
    let mk_ram = rss_mb().saturating_sub(r0);
    let tree_mb = (mk.levels.iter().map(|l| l.len()).sum::<usize>() * 32) as f64 / 1e6;

    // RESERVE=1: indice RPC delle tx dimensionato in anticipo (toglie i ridimensionamenti
    // della mappa, uguali per le tre varianti, e isola il costo dell'hash).
    let reserve = std::env::var("RESERVE").is_ok();
    if reserve {
        node.index.by_sig.reserve(blocks * per_block as usize);
    }
    // Passaggio 1: A misurato, B ricostruito sugli stessi account cambiati.
    let (mut a, mut b, mut verify) = (vec![], vec![], 0f64);
    let mut now = centrifuga::node::now_ms();
    for txs in all.clone() {
        let t = Instant::now();
        assert!(node.ingress.submit(None, txs).iter().all(|x| x.is_ok()));
        verify += t.elapsed().as_secs_f64();
        now += 100;
        let t = Instant::now();
        let ex = node.tick(now).unwrap();
        let tick = t.elapsed().as_secs_f64();
        assert_eq!(ex.block.txs.len(), per_block as usize);
        // Costo LtHash di questo blocco: due LtHash per account cambiato (via e dentro).
        let acc = node.chain.svm.accounts();
        let t = Instant::now();
        let mut lt = LtHash::identity();
        for k in &ex.changed {
            if let Some(x) = acc.get(k) {
                lt.mix_out(&account_lt_hash(k, x));
                lt.mix_in(&account_lt_hash(k, x));
            }
        }
        std::hint::black_box(lt.checksum());
        let lt_cost = t.elapsed().as_secs_f64();
        let list: Vec<(Address, Option<&AccountSharedData>)> = ex.changed.iter().map(|k| (*k, acc.get(k))).collect();
        let t = Instant::now();
        mk.update(&list);
        std::hint::black_box(mk.root());
        let mk_cost = t.elapsed().as_secs_f64();
        a.push(tick);
        b.push(tick - lt_cost + mk_cost);
    }
    let verify_ms = verify / blocks as f64 * 1e3;
    let a_dbg = a.clone();
    // Prove (B e C hanno lo stesso formato): 10.000 account casuali.
    let root = mk.root();
    let acc = node.chain.svm.accounts();
    let ks: Vec<Address> = (0..10_000).map(|_| addr(rnd() % n)).collect();
    let proofs: Vec<_> = ks.iter().map(|k| mk.proof(k)).collect();
    let t = Instant::now();
    for (k, (i, sib)) in ks.iter().zip(&proofs) {
        assert!(merkle::verify(&root, k, &acc[k], *i, sib));
    }
    let verify_us = t.elapsed().as_secs_f64() * 1e6 / ks.len() as f64;
    let proof_bytes = 8 + 32 * proofs[0].1.len();
    assert_eq!(mk.root(), mk.full_root(acc), "Merkle incrementale = completo");
    drop(node);

    // Passaggio 2: C con il thread Merkle davvero in esecuzione, sugli stessi blocchi.
    std::fs::remove_file(dir.join("journal.bin")).unwrap();
    let mut node = Node::open(cfg.clone(), kp()).unwrap();
    if reserve {
        node.index.by_sig.reserve(blocks * per_block as usize);
    }
    let mut tree = Merkle::build(node.chain.svm.accounts());
    let (tx, rx) = mpsc::channel::<Vec<(Address, Option<AccountSharedData>)>>();
    let worker = std::thread::spawn(move || {
        let mut times = vec![];
        for batch in rx {
            let t = Instant::now();
            let list: Vec<(Address, Option<&AccountSharedData>)> = batch.iter().map(|(k, a)| (*k, a.as_ref())).collect();
            tree.update(&list);
            std::hint::black_box(tree.root()); // qui la radice verrebbe firmata (~20 µs)
            times.push(t.elapsed().as_secs_f64());
        }
        times
    });
    let mut c = vec![];
    let mut pending: HashMap<Address, Option<AccountSharedData>> = HashMap::new();
    let mut now = centrifuga::node::now_ms();
    for (i, txs) in all.into_iter().enumerate() {
        assert!(node.ingress.submit(None, txs).iter().all(|x| x.is_ok()));
        now += 100;
        let t = Instant::now();
        let ex = node.tick(now).unwrap();
        // Sul thread del nodo: solo copia (dati condivisi via Arc) degli account cambiati.
        let acc = node.chain.svm.accounts();
        for k in &ex.changed {
            pending.insert(*k, acc.get(k).cloned());
        }
        if i % PER_ROOT == PER_ROOT - 1 {
            tx.send(pending.drain().collect()).unwrap();
        }
        c.push(t.elapsed().as_secs_f64());
    }
    drop(tx);
    let root_times = worker.join().unwrap();

    if std::env::var("SLOW").is_ok() {
        let mut v: Vec<(usize, f64)> = a_dbg.iter().cloned().enumerate().collect();
        v.sort_by(|x, y| y.1.total_cmp(&x.1));
        eprintln!("blocchi più lenti (indice, ms): {:?}", v.iter().take(6).map(|(i, t)| (*i, (t * 1e3) as u64)).collect::<Vec<_>>());
    }
    let rows = [Row { name: "LtHash", block: a }, Row { name: "Merkle", block: b }, Row { name: "ibrido", block: c }];
    let lt_tps = per_block as f64 / (verify_ms / 1e3 + mean(&rows[0].block));
    for r in &rows {
        let tps = per_block as f64 / (verify_ms / 1e3 + mean(&r.block));
        let merkle = r.name != "LtHash";
        println!(
            "| {n} | {} | {:.1} | {:.1} | {:.1} | {:.0} | {:.0} ({:+.1}%) | {} | {snap_mb:.0} | {:.1} | {} | {} |",
            r.name,
            pct(&r.block, 0.5),
            pct(&r.block, 0.95),
            pct(&r.block, 0.99),
            per_block as f64 / mean(&r.block),
            tps,
            (tps / lt_tps - 1.0) * 100.0,
            if merkle { format!("+{mk_ram} (albero {tree_mb:.0})") } else { "+0".into() },
            restart_lt + if merkle { mk_build } else { 0.0 },
            if merkle { format!("{proof_bytes} + account") } else { "—".into() },
            if merkle { format!("{verify_us:.1}") } else { "—".into() },
        );
    }
    println!(
        "verifica firme {verify_ms:.1} ms/blocco; ibrido: radice ogni {PER_ROOT} blocchi in {:.1} ms medi, max {:.1} ms (thread separato)",
        mean(&root_times) * 1e3,
        root_times.iter().cloned().fold(0.0, f64::max) * 1e3
    );
}
