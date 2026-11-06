//! Confronto dei moduli di archiviazione dello storico, sugli stessi record.
//!   cargo run --release --example archive_bench -- [tx] [tx_per_block]
//!   cargo run --release --example archive_bench -- --node <accounts> segments [blocks]
//! (Il confronto con l'archivio a pagine e con redb è in docs/history.md.)
//! --node: limite del nodo (carico massimo con p99 del blocco < 100 ms) senza e con
//! l'indexer in esecuzione sulla stessa macchina, e crescita del disco per ora.
//! Per ogni archivio: scrittura (tx/s, p50/p99 per blocco), byte per tx, lettura per
//! firma (presente e assente), cronologia di un indirizzo, checkpoint, riapertura dopo
//! un arresto pulito e dopo un arresto a metà (coda troncata), conservazione.

use std::path::{Path, PathBuf};
use std::time::Instant;

use centrifuga::archive::{Archive, TxRecord};
use centrifuga::genesis::transfer_tx;
use solana_address::Address;
use solana_hash::Hash;
use solana_keypair::Keypair;
use solana_signature::Signature;
use solana_signer::Signer;

fn pct(v: &mut [f64], q: f64) -> f64 {
    v.sort_by(f64::total_cmp);
    v[((v.len() - 1) as f64 * q).round() as usize]
}

fn open(kind: &str, dir: &Path) -> Box<dyn Archive> {
    match kind {
        "segments" => Box::new(centrifuga::archive::segments::Segments::open(dir, 1_000_000).unwrap()),
        _ => panic!("{kind}"),
    }
}

/// Record come quelli di un trasferimento reale (tx firmata, 2 righe di log, 3 saldi).
fn record(from: &Keypair, to: &Address, i: u64, slot: u64) -> TxRecord {
    let tx = transfer_tx(from, to, 1 + i, Hash::new_from_array([7; 32]));
    TxRecord {
        slot,
        time_ms: 1_800_000_000_000 + slot * 100,
        tx,
        err: None,
        fee: 5_000,
        logs: vec!["Program 11111111111111111111111111111111 invoke [1]".into(), "Program 11111111111111111111111111111111 success".into()],
        compute_units: 150,
        return_data: None,
        pre: vec![1_000_000_000, 0, 1],
        post: vec![999_995_000 - i, 1 + i, 1],
        node_us: None,
    }
}

fn main() {
    let raw: Vec<String> = std::env::args().skip(1).collect();
    if raw.first().map(String::as_str) == Some("--node") {
        return node_limit(raw[1].replace('_', "").parse().unwrap(), &raw[2], raw.get(3).map_or(100, |b| b.parse().unwrap()));
    }
    let a: Vec<u64> = std::env::args().skip(1).map(|x| x.replace('_', "").parse().unwrap()).collect();
    let (total, per) = (*a.first().unwrap_or(&2_000_000), *a.get(1).unwrap_or(&2_000));
    let senders: Vec<Keypair> = (0..2_000).map(|_| Keypair::new()).collect();
    let addrs: Vec<Address> = (0..100_000).map(|_| Address::new_unique()).collect();
    let blocks = total / per;
    // Record pronti in anticipo (la firma costa: non va misurata).
    let all: Vec<Vec<TxRecord>> = (0..blocks).map(|b| (0..per).map(|j| record(&senders[(j % 2_000) as usize], &addrs[((b * per + j) * 7919 % 100_000) as usize], b * per + j, b + 1)).collect()).collect();
    let busy = senders[0].pubkey();
    let sample: Vec<Signature> = (0..10_000u64).map(|i| all[(i * 7 % blocks) as usize][(i * 13 % per) as usize].tx.signatures[0]).collect();
    let base = PathBuf::from(std::env::var("LOAD_DIR").unwrap_or_else(|_| "/tmp/archive-bench".into()));
    let kinds = vec!["segments"];
    println!("{total} tx in blocchi da {per}");
    println!("| archivio | scrittura tx/s | blocco p50 / p99 ms | byte/tx | get p50 / p99 µs | get assente µs | cronologia 10 µs | checkpoint ms | riapertura s | dopo arresto a metà s | conservazione (metà) s |");
    for kind in kinds {
        let dir = base.join(kind);
        let _ = std::fs::remove_dir_all(&dir);
        let mut ar = open(kind, &dir);
        let (mut per_block, t0) = (vec![], Instant::now());
        for (b, recs) in all.iter().enumerate() {
            let t = Instant::now();
            ar.append(b as u64 + 1, recs.clone()).unwrap();
            per_block.push(t.elapsed().as_secs_f64() * 1e3);
        }
        let write_tps = total as f64 / t0.elapsed().as_secs_f64();
        let t = Instant::now();
        ar.checkpoint().unwrap();
        let ckpt = t.elapsed().as_secs_f64() * 1e3;
        let bytes = ar.disk_bytes() as f64 / total as f64;
        let mut gets: Vec<f64> = sample
            .iter()
            .map(|s| {
                let t = Instant::now();
                assert!(ar.get(s).is_some());
                t.elapsed().as_secs_f64() * 1e6
            })
            .collect();
        let t = Instant::now();
        for i in 0..=255u8 {
            assert!(ar.get(&Signature::from([i; 64])).is_none());
        }
        let miss = t.elapsed().as_secs_f64() * 1e6 / 256.0;
        let t = Instant::now();
        for _ in 0..100 {
            assert_eq!(ar.history(&busy, None, None, 10).len(), 10);
        }
        let hist = t.elapsed().as_secs_f64() * 1e6 / 100.0;
        drop(ar);
        let t = Instant::now();
        let ar = open(kind, &dir);
        let reopen = t.elapsed().as_secs_f64();
        assert_eq!(ar.height(), blocks);
        drop(ar);
        // Arresto a metà: coda del file più recente troncata (redb lo rifiuta: l'indexer
        // lo ricostruirebbe dal journal; qui si misura la riapertura o il rifiuto).
        // File dei record: txlog.bin (pagine), ultimo seg-*.log (segmenti), history.redb.
        let mut files: Vec<PathBuf> = std::fs::read_dir(&dir).unwrap().flatten().map(|e| e.path()).filter(|p| p.file_name().unwrap().to_str().unwrap().ends_with(".log") || p.ends_with("txlog.bin") || p.ends_with("history.redb")).collect();
        files.sort();
        let last = files.pop().unwrap();
        let len = std::fs::metadata(&last).unwrap().len();
        std::fs::OpenOptions::new().write(true).open(&last).unwrap().set_len(len - 10).unwrap();
        let t = Instant::now();
        let crash = match std::panic::catch_unwind(|| open(kind, &dir)) {
            Ok(mut ar) => {
                let s = t.elapsed().as_secs_f64();
                // Si riscrive l'ultimo blocco perso, come farebbe l'indexer dal journal.
                let h = ar.height();
                for b in h..blocks {
                    ar.append(b + 1, all[b as usize].clone()).unwrap();
                }
                format!("{s:.2} (ultimo blocco perso: {})", blocks - h)
            }
            Err(_) => "rifiutato: ricostruzione dal journal".into(),
        };
        let prune = match std::panic::catch_unwind(|| open(kind, &dir)) {
            Ok(mut ar) => {
                let t = Instant::now();
                match ar.prune(blocks / 2) {
                    Ok(()) => format!("{:.2}", t.elapsed().as_secs_f64()),
                    Err(e) => format!("non supportata ({e})"),
                }
            }
            Err(_) => "—".into(),
        };
        println!(
            "| {kind} | {write_tps:.0} | {:.1} / {:.1} | {bytes:.0} | {:.0} / {:.0} | {miss:.0} | {hist:.0} | {ckpt:.0} | {reopen:.2} | {crash} | {prune} |",
            pct(&mut per_block, 0.5),
            pct(&mut per_block, 0.99),
            pct(&mut gets, 0.5),
            pct(&mut gets, 0.99)
        );
        let _ = std::fs::remove_dir_all(&dir);
    }
}

fn addr(i: u64) -> Address {
    let mut b = [0u8; 32];
    b[..8].copy_from_slice(&i.to_le_bytes());
    b[8] = 0xCA;
    Address::new_from_array(b)
}

/// Limite del nodo con e senza indexer: stesso stato iniziale, stessi blocchi.
fn node_limit(n: u64, storage: &str, blocks: usize) {
    use centrifuga::genesis::{Config, IndexerConfig, LAMPORTS_PER_CNFA, Prefund};
    use centrifuga::node::Node;
    use std::sync::atomic::{AtomicBool, Ordering};
    const STEPS: [usize; 9] = [100, 250, 500, 1000, 1500, 2000, 2200, 2400, 3000];
    let op = Keypair::new();
    let senders: Vec<Keypair> = (0..2_000).map(|_| Keypair::new()).collect();
    let mut cfg: Config = toml::from_str(include_str!("../centrifuga.example.toml")).unwrap();
    cfg.genesis.operator = op.pubkey().to_string();
    cfg.genesis.network = "devnet".into();
    cfg.genesis.chain_id = "centrifuga:devnet".into();
    cfg.genesis.timestamp_ms = centrifuga::node::now_ms() - 86_400_000;
    cfg.genesis.prefund = senders.iter().map(|k| Prefund { pubkey: k.pubkey().to_string(), lamports: 1_000 * LAMPORTS_PER_CNFA }).collect();
    cfg.node.snapshot_every = 0;
    cfg.node.ingress_pending_per_payer = u32::MAX;
    let base = PathBuf::from(std::env::var("LOAD_DIR").unwrap_or_else(|_| "/tmp/archive-bench".into())).join("node");
    let _ = std::fs::remove_dir_all(&base);
    cfg.node.data_dir = base.join("node");
    std::fs::create_dir_all(&cfg.node.data_dir).unwrap();
    cfg.indexer = Some(IndexerConfig { data_dir: base.join("indexer"), rpc_addr: "127.0.0.1:0".into(), retention_blocks: 0, segment_txs: 1_000_000, snapshot_every: 0 });
    let mut chain = centrifuga::genesis::build(&cfg.genesis).unwrap().0;
    let rent = chain.svm.rent_exempt(0);
    for i in 0..n {
        chain.svm.set(addr(i), solana_account::Account { lamports: rent, owner: solana_sdk_ids::system_program::id(), ..Default::default() });
    }
    let snap0 = base.join("snapshot0.bin");
    centrifuga::ledger::write_snapshot(&base, 0, chain.genesis_hash, chain.full_state_hash(), chain.svm.accounts().iter()).unwrap();
    std::fs::rename(base.join("snapshot.bin"), &snap0).unwrap();
    drop(chain);
    let kp = || Keypair::try_from(op.to_bytes().as_slice()).unwrap();
    let mut seed = 0x9E37_79B9_7F4A_7C15u64;
    let mut rnd = move || {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        seed
    };
    // Stato pulito per ogni passaggio: snapshot iniziale, journal e indexer vuoti.
    let fresh = |cfg: &Config| {
        let _ = std::fs::remove_dir_all(&cfg.node.data_dir);
        let _ = std::fs::remove_dir_all(&cfg.indexer.as_ref().unwrap().data_dir);
        std::fs::create_dir_all(&cfg.node.data_dir).unwrap();
        std::fs::create_dir_all(cfg.indexer.as_ref().unwrap().data_dir.join("state")).unwrap();
        std::fs::copy(&snap0, cfg.node.data_dir.join("snapshot.bin")).unwrap();
        std::fs::copy(&snap0, cfg.indexer.as_ref().unwrap().data_dir.join("state/snapshot.bin")).unwrap();
        Node::open(cfg.clone(), kp()).unwrap()
    };
    let bh = fresh(&cfg).chain.last_hash;
    let max = *STEPS.last().unwrap();
    let all: Vec<Vec<_>> = (0..blocks as u64).map(|b| (0..max as u64).map(|j| transfer_tx(&senders[(j % 2_000) as usize], &addr(rnd() % n), 1 + b * max as u64 + j, bh)).collect()).collect();
    println!("{n} account, archivio {storage}, {blocks} blocchi per gradino");
    println!("| tx/blocco | senza indexer p50 / p99 ms | con indexer p50 / p99 ms | indexer indietro alla fine (blocchi) | journal B/tx | archivio B/tx |");
    let (mut best, mut best_ix, mut bytes) = (0, 0, (0.0, 0.0));
    for load in STEPS {
        let mut cfg = cfg.clone();
        cfg.node.max_block_txs = load;
        let mut res = vec![];
        for with_ix in [false, true] {
            let mut node = fresh(&cfg);
            let stop = std::sync::Arc::new(AtomicBool::new(false));
            let journal = cfg.node.data_dir.join("journal.bin");
            let follower = with_ix.then(|| {
                let (cfg, stop, journal) = (cfg.clone(), stop.clone(), journal.clone());
                std::thread::spawn(move || {
                    let mut ix = centrifuga::indexer::Indexer::open(&cfg, &journal).unwrap();
                    while !stop.load(Ordering::Relaxed) {
                        if !ix.step().unwrap() {
                            std::thread::sleep(std::time::Duration::from_millis(5));
                        }
                    }
                    while ix.step().unwrap() {}
                    ix
                })
            });
            let (mut times, mut now) = (vec![], centrifuga::node::now_ms());
            let wall = Instant::now();
            for (i, txs) in all.iter().enumerate() {
                // Tempo reale: un blocco ogni 100 ms, come il nodo vero (l'indexer gira nelle pause).
                let due = wall + std::time::Duration::from_millis(100 * i as u64);
                std::thread::sleep(due.saturating_duration_since(Instant::now()));
                node.ingress.submit(None, txs[..load].to_vec());
                now += 100;
                let t = Instant::now();
                assert_eq!(node.tick(now).unwrap().block.txs.len(), load);
                times.push(t.elapsed().as_secs_f64() * 1e3);
            }
            let behind = follower.map(|f| {
                let h = node.chain.height;
                let lag_start = Instant::now();
                stop.store(true, Ordering::Relaxed);
                let ix = f.join().unwrap();
                let a = ix.archive.lock().unwrap();
                assert_eq!(a.height(), h, "l'indexer recupera");
                bytes = (std::fs::metadata(&journal).unwrap().len() as f64 / (blocks * load) as f64, a.disk_bytes() as f64 / (blocks * load) as f64);
                let _ = lag_start;
                h - ix.chain.height.min(h)
            });
            res.push((pct(&mut times.clone(), 0.5), pct(&mut times, 0.99), behind));
        }
        let (a, b) = (&res[0], &res[1]);
        println!("| {load} | {:.1} / {:.1} | {:.1} / {:.1} | {} | {:.0} | {:.0} |", a.0, a.1, b.0, b.1, b.2.unwrap_or(0), bytes.0, bytes.1);
        if a.1 < 100.0 {
            best = load;
        }
        if b.1 < 100.0 {
            best_ix = load;
        }
        if a.1 >= 100.0 && b.1 >= 100.0 {
            break;
        }
    }
    let gb_hour = |load: usize, b: f64| load as f64 * 10.0 * 3600.0 * b / 1e9;
    println!("limite del nodo (p99 < 100 ms): senza indexer {} tx/s, con indexer {} tx/s", best * 10, best_ix * 10);
    println!("crescita del disco al limite con indexer ({} tx/s): journal {:.1} GB/ora, archivio {:.1} GB/ora", best_ix * 10, gb_hour(best_ix, bytes.0), gb_hour(best_ix, bytes.1));
}
