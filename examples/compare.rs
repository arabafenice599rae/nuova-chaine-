//! Confronto tra tre hash di stato sugli stessi blocchi reali, con ricerca a gradini
//! del carico massimo sostenibile (p99 del blocco < 100 ms):
//!   A. LtHash a ogni blocco (quello nel nodo);
//!   B. Merkle incrementale a ogni blocco, con prove per account;
//!   C. ibrido: LtHash a ogni blocco + radice Merkle ogni secondo (10 blocchi), su un
//!      thread separato con i soli account cambiati.
//!   cargo run --release --example compare -- <accounts> [blocks_per_step]
//! A e B dallo stesso passaggio: B = blocco - hash LtHash misurato + aggiornamento
//! Merkle sugli stessi account. C in un secondo passaggio sugli stessi blocchi, con il
//! thread Merkle davvero in esecuzione (compete per la CPU con il nodo).

#[path = "common/merkle.rs"]
mod merkle;

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::mpsc;
use std::time::Instant;

use centrifuga::exec::Phases;
use centrifuga::genesis::{self, Config, LAMPORTS_PER_CNFA, Prefund, transfer_tx};
use centrifuga::node::Node;
use merkle::Merkle;
use solana_account::{Account, AccountSharedData};
use solana_address::Address;
use solana_keypair::Keypair;
use solana_signer::Signer;
use solana_transaction::versioned::VersionedTransaction;

const EXAMPLE: &str = include_str!("../centrifuga.example.toml");
const SENDERS: usize = 2_000;
const PER_ROOT: usize = 10;
const STEPS: [usize; 10] = [500, 1000, 1500, 2000, 2500, 3000, 3500, 4000, 4500, 5000];
const LIMIT_MS: f64 = 100.0;

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
    s[((s.len() - 1) as f64 * q).round() as usize]
}

/// Un blocco: tempo totale e fasi (ms) per la variante.
#[derive(Clone, Copy, Default)]
struct B {
    total: f64,
    exec: f64,
    hash: f64,
    sign: f64,
    journal: f64,
    other: f64,
}

fn ms(d: std::time::Duration) -> f64 {
    d.as_secs_f64() * 1e3
}

fn from(p: &Phases, hash: f64) -> B {
    let (exec, sign, journal, other) = (ms(p.exec), ms(p.sign), ms(p.journal), ms(p.other));
    B { total: exec + hash + sign + journal + other, exec, hash, sign, journal, other }
}

struct Step {
    load: usize,
    a: Vec<B>,
    b: Vec<B>,
    c: Vec<B>,
    root_ms: Vec<f64>,
}

fn main() {
    let args: Vec<u64> = std::env::args().skip(1).map(|a| a.replace('_', "").parse().expect("number")).collect();
    let (n, blocks) = (args[0], *args.get(1).unwrap_or(&100) as usize);
    let op = Keypair::new();
    let senders: Vec<Keypair> = (0..SENDERS).map(|_| Keypair::new()).collect();
    let mut cfg: Config = toml::from_str(EXAMPLE).unwrap();
    cfg.genesis.operator = op.pubkey().to_string();
    cfg.genesis.network = "devnet".into();
    cfg.genesis.chain_id = "centrifuga:devnet".into();
    cfg.genesis.timestamp_ms = centrifuga::node::now_ms() - 86_400_000;
    cfg.genesis.prefund = senders.iter().map(|k| Prefund { pubkey: k.pubkey().to_string(), lamports: 1_000 * LAMPORTS_PER_CNFA }).collect();
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
    let open = |cfg: &Config| {
        let _ = std::fs::remove_file(dir.join("journal.bin"));
        Node::open(cfg.clone(), kp()).unwrap()
    };

    let t = Instant::now();
    let node = open(&cfg);
    let restart_lt = t.elapsed().as_secs_f64();
    let base_ram = rss_mb();
    let bh = node.chain.last_hash;
    let r0 = rss_mb();
    let t = Instant::now();
    let mk0 = Merkle::build(node.chain.svm.accounts());
    let mk_build = t.elapsed().as_secs_f64();
    let mk_ram = rss_mb().saturating_sub(r0);
    // Dimensione calcolata: livelli (32 byte per nodo) + indice pubkey -> foglia (~52 byte per voce).
    let mk_calc = (mk0.levels.iter().map(|l| l.len()).sum::<usize>() * 32 + mk0.index.len() * 52) / 1_000_000;
    // Prove (B e C hanno lo stesso formato).
    let mut seed = 0x9E37_79B9_7F4A_7C15u64;
    let mut rnd = move || {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        seed
    };
    let root = mk0.root();
    let acc = node.chain.svm.accounts();
    let ks: Vec<Address> = (0..10_000).map(|_| addr(rnd() % n)).collect();
    let proofs: Vec<_> = ks.iter().map(|k| mk0.proof(k)).collect();
    let t = Instant::now();
    for (k, (i, sib)) in ks.iter().zip(&proofs) {
        assert!(merkle::verify(&root, k, &acc[k], *i, sib));
    }
    let verify_us = t.elapsed().as_secs_f64() * 1e6 / ks.len() as f64;
    let proof_bytes = 8 + 32 * proofs[0].1.len();
    drop((node, mk0));

    // Tx firmate in anticipo per il carico massimo; ogni gradino usa i primi `load` di ogni blocco.
    let max = *STEPS.last().unwrap();
    let all: Vec<Vec<VersionedTransaction>> = (0..blocks as u64)
        .map(|b| (0..max as u64).map(|j| transfer_tx(&senders[(j as usize) % SENDERS], &addr(rnd() % n.max(1)), 1 + b * max as u64 + j, bh)).collect())
        .collect();

    let mut steps: Vec<Step> = vec![];
    let mut peak_ram = 0;
    for load in STEPS {
        cfg.node.max_block_txs = load;
        // Passaggio 1: A e B.
        let mut node = open(&cfg);
        let mut mk = Merkle::build(node.chain.svm.accounts());
        let (mut a, mut b) = (vec![], vec![]);
        let mut now = centrifuga::node::now_ms();
        for txs in &all {
            assert!(node.ingress.submit(None, txs[..load].to_vec()).iter().all(|x| x.is_ok()));
            now += 100;
            let ex = node.tick(now).unwrap();
            assert_eq!(ex.block.txs.len(), load);
            let acc = node.chain.svm.accounts();
            let list: Vec<(Address, Option<&AccountSharedData>)> = ex.changed.iter().map(|k| (*k, acc.get(k))).collect();
            let t = Instant::now();
            mk.update(&list);
            std::hint::black_box(mk.root());
            let mk_ms = t.elapsed().as_secs_f64() * 1e3;
            a.push(from(&ex.phases, ms(ex.phases.hash)));
            b.push(from(&ex.phases, mk_ms));
        }
        assert_eq!(mk.root(), mk.full_root(node.chain.svm.accounts()), "Merkle incrementale = completo");
        peak_ram = peak_ram.max(rss_mb());
        drop((node, mk));
        // Passaggio 2: C con il thread Merkle in esecuzione.
        let mut node = open(&cfg);
        let mut tree = Merkle::build(node.chain.svm.accounts());
        let (tx, rx) = mpsc::channel::<Vec<(Address, Option<AccountSharedData>)>>();
        let worker = std::thread::spawn(move || {
            let mut times = vec![];
            for batch in rx {
                let t = Instant::now();
                let list: Vec<(Address, Option<&AccountSharedData>)> = batch.iter().map(|(k, a)| (*k, a.as_ref())).collect();
                tree.update(&list);
                std::hint::black_box(tree.root()); // qui la radice verrebbe firmata
                times.push(t.elapsed().as_secs_f64() * 1e3);
            }
            times
        });
        let mut c = vec![];
        let mut pending: HashMap<Address, Option<AccountSharedData>> = HashMap::new();
        let mut now = centrifuga::node::now_ms();
        for (i, txs) in all.iter().enumerate() {
            assert!(node.ingress.submit(None, txs[..load].to_vec()).iter().all(|x| x.is_ok()));
            now += 100;
            let ex = node.tick(now).unwrap();
            let t = Instant::now();
            let acc = node.chain.svm.accounts();
            for k in &ex.changed {
                pending.insert(*k, acc.get(k).cloned());
            }
            if i % PER_ROOT == PER_ROOT - 1 {
                tx.send(pending.drain().collect()).unwrap();
            }
            c.push(from(&ex.phases, ms(ex.phases.hash) + t.elapsed().as_secs_f64() * 1e3));
        }
        drop(tx);
        let root_ms = worker.join().unwrap();
        drop(node);
        let p99 = |v: &[B]| pct(&v.iter().map(|x| x.total).collect::<Vec<_>>(), 0.99);
        eprintln!("n={n} load={load}: p99 A {:.1} B {:.1} C {:.1} ms", p99(&a), p99(&b), p99(&c));
        let all_over = p99(&a) >= LIMIT_MS && p99(&b) >= LIMIT_MS && p99(&c) >= LIMIT_MS;
        steps.push(Step { load, a, b, c, root_ms });
        if all_over {
            break;
        }
    }

    // Per ogni variante: gradino più alto con p99 < 100 ms (carico sostenuto).
    fn pick(s: &Step, v: usize) -> &Vec<B> {
        match v {
            0 => &s.a,
            1 => &s.b,
            _ => &s.c,
        }
    }
    let names = ["LtHash", "Merkle", "ibrido"];
    for v in 0..3 {
        let ok = steps.iter().filter(|s| pct(&pick(s, v).iter().map(|x| x.total).collect::<Vec<_>>(), 0.99) < LIMIT_MS).last();
        let Some(s) = ok else {
            println!("| {n} | {} | < {} tx/blocco | | | | | | | | |", names[v], STEPS[0]);
            continue;
        };
        let bl = pick(s, v);
        let tot: Vec<f64> = bl.iter().map(|x| x.total).collect();
        let mean = |f: fn(&B) -> f64| bl.iter().map(f).sum::<f64>() / bl.len() as f64;
        let merkle = v > 0;
        println!(
            "| {n} | {} | {} | {} | {:.1} / {:.1} / {:.1} | {:.1} / {:.1} / {:.2} / {:.1} / {:.1} | {} | {snap_mb:.0} | {:.1} | {} |",
            names[v],
            s.load,
            s.load * 10,
            pct(&tot, 0.5),
            pct(&tot, 0.95),
            pct(&tot, 0.99),
            mean(|x| x.exec),
            mean(|x| x.hash),
            mean(|x| x.sign),
            mean(|x| x.journal),
            mean(|x| x.other),
            if merkle { format!("{base_ram} + {} (calcolati {mk_calc})", mk_ram) } else { format!("{base_ram}") },
            restart_lt + if merkle { mk_build } else { 0.0 },
            if merkle { format!("{proof_bytes} B, {verify_us:.1} µs") } else { "—".into() },
        );
        if v == 2 {
            let r = &s.root_ms;
            println!("ibrido: radice ogni secondo in {:.1} ms medi, max {:.1} ms (thread separato)", r.iter().sum::<f64>() / r.len() as f64, r.iter().cloned().fold(0.0, f64::max));
        }
    }
    // Picchi al carico più alto misurato: fasi del blocco più lento (LtHash).
    let last = steps.last().unwrap();
    let mut slow: Vec<(usize, B)> = last.a.iter().cloned().enumerate().collect();
    slow.sort_by(|x, y| y.1.total.total_cmp(&x.1.total));
    for (i, x) in slow.iter().take(4) {
        println!(
            "picco a {} tx/blocco: blocco {i} {:.0} ms = esecuzione {:.0} + hash {:.1} + firma {:.2} + journal {:.1} + altro {:.0}",
            last.load, x.total, x.exec, x.hash, x.sign, x.journal, x.other
        );
    }
    println!("RAM di picco del processo: {peak_ram} MB");
}
