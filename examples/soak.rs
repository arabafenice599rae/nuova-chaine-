//! Test di durata: carico costante in tempo reale (uno slot ogni 100 ms) per N minuti.
//!   cargo run --release --example soak -- <accounts> <tps> <minutes>
//! Ogni minuto: RSS, tempo del blocco (p50, p99, massimo), tx incluse, dimensione di
//! journal e storico. Alla fine: pausa degli snapshot, crescita per ora, riavvio da
//! snapshot e riesecuzione completa del journal. Le tx sono firmate da due thread
//! (lato client) con il blockhash più recente.

use std::path::PathBuf;
use std::sync::{Arc, Mutex, mpsc};
use std::time::{Duration, Instant};

use centrifuga::genesis::{self, Config, LAMPORTS_PER_CNFA, Prefund, transfer_tx};
use centrifuga::node::Node;
use solana_account::Account;
use solana_address::Address;
use solana_hash::Hash;
use solana_keypair::Keypair;
use solana_signer::Signer;

const EXAMPLE: &str = include_str!("../centrifuga.example.toml");
const SENDERS: usize = 4_000;

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

fn size_mb(dir: &std::path::Path, names: &[&str]) -> f64 {
    names.iter().filter_map(|n| std::fs::metadata(dir.join(n)).ok()).map(|m| m.len()).sum::<u64>() as f64 / 1e6
}

fn pct(v: &mut [f64], q: f64) -> f64 {
    v.sort_by(f64::total_cmp);
    v.get(((v.len().max(1) - 1) as f64 * q).round() as usize).copied().unwrap_or(0.0)
}

fn main() {
    let a: Vec<u64> = std::env::args().skip(1).map(|x| x.replace('_', "").parse().expect("number")).collect();
    let (n, tps, minutes) = (a[0], a[1], a[2]);
    let per_block = (tps / 10) as usize;
    let op = Keypair::new();
    let senders: Arc<Vec<Keypair>> = Arc::new((0..SENDERS).map(|_| Keypair::new()).collect());
    let mut cfg: Config = toml::from_str(EXAMPLE).unwrap();
    cfg.genesis.operator = op.pubkey().to_string();
    cfg.genesis.network = "devnet".into();
    cfg.genesis.chain_id = "centrifuga:devnet".into();
    cfg.genesis.timestamp_ms = centrifuga::node::now_ms() - 86_400_000;
    cfg.genesis.prefund = senders.iter().map(|k| Prefund { pubkey: k.pubkey().to_string(), lamports: 1_000 * LAMPORTS_PER_CNFA }).collect();
    cfg.node.ingress_pending_per_payer = u32::MAX;
    let dir = std::env::var("LOAD_DIR").map(PathBuf::from).unwrap_or_else(|_| std::env::temp_dir().join("centrifuga-soak"));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    cfg.node.data_dir = dir.clone();
    let mut chain = genesis::build(&cfg.genesis).unwrap().0;
    let rent = chain.svm.rent_exempt(0);
    for i in 0..n {
        chain.svm.set(addr(i), Account { lamports: rent, owner: solana_sdk_ids::system_program::id(), ..Account::default() });
    }
    centrifuga::ledger::write_snapshot(&dir, 0, chain.genesis_hash, chain.svm.accounts().iter()).unwrap();
    drop(chain);
    let kp = || Keypair::try_from(op.to_bytes().as_slice()).unwrap();
    let mut node = Node::open(cfg.clone(), kp()).unwrap();
    println!("{n} account, {tps} tx/s ({per_block} per blocco), {minutes} minuti, snapshot ogni {} blocchi", cfg.node.snapshot_every);

    // Firmatari: lotti da `per_block` tx con il blockhash corrente.
    let bh = Arc::new(Mutex::new(node.chain.last_hash));
    let (tx, rx) = mpsc::sync_channel::<Vec<_>>(30);
    for w in 0..2u64 {
        let (tx, bh, senders) = (tx.clone(), bh.clone(), senders.clone());
        std::thread::spawn(move || {
            let mut i = w;
            loop {
                let h: Hash = *bh.lock().unwrap();
                let batch = (0..per_block as u64)
                    .map(|j| {
                        let k = i * per_block as u64 + j;
                        let to = addr(k.wrapping_mul(0x9E37_79B9_7F4A_7C15) % n.max(1));
                        transfer_tx(&senders[(k % SENDERS as u64) as usize], &to, 1 + k % 100_000, h)
                    })
                    .collect();
                if tx.send(batch).is_err() {
                    return;
                }
                i += 2;
            }
        });
    }

    let start = Instant::now();
    let end = start + Duration::from_secs(minutes * 60);
    let (mut next, mut minute) = (start + Duration::from_millis(100), 1u64);
    let (mut times, mut snaps, mut short, mut included) = (vec![], vec![], 0u64, 0u64);
    let (mut all_p99, mut all_max) = (vec![], vec![]);
    let files = ["journal.bin"];
    let hist = ["txlog.bin", "txsig.idx", "txsig.tab", "txheads"];
    println!("| min | RSS MB | blocco p50 / p99 / max ms | tx incluse | journal MB | storico MB | lotti mancati |");
    while Instant::now() < end {
        std::thread::sleep(next.saturating_duration_since(Instant::now()));
        next += Duration::from_millis(100);
        match rx.try_recv() {
            Ok(batch) => {
                node.ingress.submit(None, batch);
            }
            Err(_) => short += 1,
        }
        let t = Instant::now();
        let ex = node.tick(centrifuga::node::now_ms()).unwrap();
        let dt = t.elapsed().as_secs_f64() * 1e3;
        *bh.lock().unwrap() = node.chain.last_hash;
        included += ex.block.txs.len() as u64;
        let every = cfg.node.snapshot_every;
        if every > 0 && ex.block.header.height % every == 0 {
            snaps.push(dt);
        } else {
            times.push(dt);
        }
        if start.elapsed() >= Duration::from_secs(minute * 60) {
            let (p50, p99, max) = (pct(&mut times, 0.5), pct(&mut times, 0.99), pct(&mut times, 1.0));
            all_p99.push(p99);
            all_max.push(max);
            println!(
                "| {minute} | {} | {p50:.1} / {p99:.1} / {max:.1} | {included} | {:.0} | {:.0} | {short} |",
                rss_mb(),
                size_mb(&dir, &files),
                size_mb(&dir, &hist)
            );
            times.clear();
            minute += 1;
        }
    }
    drop(rx);
    let hours = start.elapsed().as_secs_f64() / 3600.0;
    let (journal, history) = (size_mb(&dir, &files), size_mb(&dir, &hist));
    let height = node.chain.height;
    drop(node);
    println!("snapshot: {} pause, {}", snaps.len(), snaps.iter().map(|s| format!("{s:.0} ms")).collect::<Vec<_>>().join(", "));
    println!("crescita per ora: journal {:.0} MB, storico tx {:.0} MB ({} tx incluse, {:.0} tx/s medi)", journal / hours, history / hours, included, included as f64 / (hours * 3600.0));
    let t = Instant::now();
    let node = Node::open(cfg.clone(), kp()).unwrap();
    println!("riavvio da snapshot + journal: {:.1} s (altezza {})", t.elapsed().as_secs_f64(), node.chain.height);
    drop(node);
    std::fs::remove_file(dir.join("snapshot.bin")).unwrap();
    // Riesecuzione completa: dal genesis senza gli N account iniettati non si può (lo
    // stato iniziale del test sta nello snapshot all'altezza 0), quindi lo snapshot
    // iniziale si riscrive e si riesegue tutto il journal da lì.
    let mut chain = genesis::build(&cfg.genesis).unwrap().0;
    for i in 0..n {
        chain.svm.set(addr(i), Account { lamports: rent, owner: solana_sdk_ids::system_program::id(), ..Account::default() });
    }
    centrifuga::ledger::write_snapshot(&dir, 0, chain.genesis_hash, chain.svm.accounts().iter()).unwrap();
    drop(chain);
    let t = Instant::now();
    let node = Node::open(cfg, kp()).unwrap();
    println!("riesecuzione completa del journal ({height} blocchi): {:.1} s (altezza {})", t.elapsed().as_secs_f64(), node.chain.height);
    let trend = |v: &[f64]| (v.iter().take(5).sum::<f64>() / 5.0, v.iter().rev().take(5).sum::<f64>() / 5.0);
    let (p0, p1) = trend(&all_p99);
    let (m0, m1) = trend(&all_max);
    println!("p99 per minuto: primi 5 {p0:.1} ms, ultimi 5 {p1:.1} ms; massimo per minuto: primi 5 {m0:.1} ms, ultimi 5 {m1:.1} ms");
}
