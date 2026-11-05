//! Test di durata sul nodo reale (`node::start`: ciclo di slot, RPC, stream).
//!   cargo run --release --example soak -- <accounts> <tps> <minutes> [--rpc]
//!   cargo run --release --example soak -- --reopen <dir>     (riavvio e riesecuzione)
//! Carico: thread client firmano trasferimenti al ritmo richiesto e li consegnano
//!   - senza --rpc: direttamente all'ingresso del nodo (3 thread). Misura il LIMITE DEL
//!     NODO, non il throughput di un utente: niente HTTP, JSON, base64.
//!   - con --rpc: via HTTP/JSON-RPC `sendTransaction` (16 thread, una richiesta per tx),
//!     come un client vero: è il numero end-to-end (client sulla stessa macchina). Ogni minuto: tx inviate, accettate, incluse, TPS effettivi,
//! blocco p50/p95/p99 (misurato dentro il nodo), RAM, journal, e due controlli:
//!   - getSignatureStatuses (RPC) trova una tx di ~290 blocchi fa e non una di >300;
//!   - ogni 5 minuti `centrifuga wallet send` sotto carico: fee, premio, rimborso e costo
//!     netto coerenti con il saldo letto via RPC.
//! Alla fine scrive soak.meta; `--reopen` misura il riavvio da snapshot + journal e la
//! riesecuzione completa dallo stato iniziale, e confronta gli hash con il nodo vivo.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use centrifuga::genesis::{self, Config, LAMPORTS_PER_CNFA, Prefund, transfer_tx};
use centrifuga::node::{self, Node};
use centrifuga::rpc::client::call;
use serde_json::json;
use sha2::{Digest, Sha256};
use solana_account::Account;
use solana_address::Address;
use solana_hash::Hash;
use solana_keypair::Keypair;
use solana_signature::Signature;
use solana_signer::Signer;

const EXAMPLE: &str = include_str!("../centrifuga.example.toml");
const SENDERS: u64 = 4_000;
const SIGNERS: u64 = 3;
const RPC: &str = "127.0.0.1:19400";

fn key(i: u64) -> Keypair {
    Keypair::new_from_array(Sha256::digest(i.to_le_bytes()).into())
}

fn addr(i: u64) -> Address {
    let mut b = [0u8; 32];
    b[..8].copy_from_slice(&i.to_le_bytes());
    b[8] = 0xCA;
    Address::new_from_array(b)
}

fn rss_mb() -> u64 {
    let s = std::fs::read_to_string("/proc/self/status").unwrap_or_default();
    s.lines().find(|l| l.starts_with("VmRSS:")).and_then(|l| l.split_whitespace().nth(1)?.parse::<u64>().ok()).unwrap_or(0) / 1024
}

fn pct(v: &mut [f64], q: f64) -> f64 {
    v.sort_by(f64::total_cmp);
    v.get(((v.len().max(1) - 1) as f64 * q).round() as usize).copied().unwrap_or(0.0)
}

fn mb(p: &Path) -> f64 {
    std::fs::metadata(p).map_or(0.0, |m| m.len() as f64 / 1e6)
}

/// Stessa configurazione nel test e nella riapertura (chiavi deterministiche).
fn config(dir: &Path, timestamp_ms: u64) -> Config {
    let mut cfg: Config = toml::from_str(EXAMPLE).unwrap();
    cfg.genesis.operator = key(u64::MAX).pubkey().to_string();
    cfg.genesis.network = "devnet".into();
    cfg.genesis.chain_id = "centrifuga:devnet".into();
    cfg.genesis.timestamp_ms = timestamp_ms;
    cfg.genesis.prefund = (0..=SENDERS).map(|i| Prefund { pubkey: key(i).pubkey().to_string(), lamports: 1_000 * LAMPORTS_PER_CNFA }).collect();
    cfg.node.data_dir = dir.into();
    cfg.node.rpc_addr = RPC.into();
    cfg.node.stream_addr = "127.0.0.1:19402".into();
    cfg.node.ingress_pending_per_payer = u32::MAX;
    cfg.node.ingress_tps_per_ip = u32::MAX;
    cfg
}

fn initial_state(cfg: &Config, n: u64) -> genesis::GenesisConfig {
    let mut chain = genesis::build(&cfg.genesis).unwrap().0;
    let rent = chain.svm.rent_exempt(0);
    for i in 0..n {
        chain.svm.set(addr(i), Account { lamports: rent, owner: solana_sdk_ids::system_program::id(), ..Account::default() });
    }
    centrifuga::ledger::write_snapshot(&cfg.node.data_dir, 0, chain.genesis_hash, chain.svm.accounts().iter()).unwrap();
    cfg.genesis.clone()
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.first().map(String::as_str) == Some("--reopen") {
        return reopen(Path::new(&args[1]));
    }
    let via_rpc = args.iter().any(|x| x == "--rpc");
    let signers_n: u64 = if via_rpc { 16 } else { SIGNERS };
    let a: Vec<u64> = args.iter().filter(|x| !x.starts_with("--")).map(|x| x.replace('_', "").parse().expect("number")).collect();
    let (n, tps, minutes) = (a[0], a[1], a[2]);
    let dir = std::env::var("LOAD_DIR").map(PathBuf::from).unwrap_or_else(|_| std::env::temp_dir().join("centrifuga-soak"));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let ts = node::now_ms() - 86_400_000;
    let cfg = config(&dir, ts);
    initial_state(&cfg, n);
    std::fs::copy(dir.join("snapshot.bin"), dir.join("snapshot0.bin")).unwrap();
    let ram0 = rss_mb();
    let running = node::start(cfg.clone(), key(u64::MAX)).unwrap();
    let gw = running.gateway.clone();
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.spawn(running.serve_network());
    std::thread::sleep(Duration::from_millis(500));
    let ingress = rt.block_on(gw.query(|n: &mut Node| n.ingress.clone())).unwrap();
    let bh = Arc::new(Mutex::new(rt.block_on(gw.query(|n: &mut Node| n.chain.last_hash)).unwrap()));
    let height = Arc::new(AtomicU64::new(0));
    let (sent, accepted, stop) = (Arc::new(AtomicU64::new(0)), Arc::new(AtomicU64::new(0)), Arc::new(AtomicBool::new(false)));
    // Campioni (firma, altezza all'invio) per il controllo della finestra degli stati.
    let samples: Arc<Mutex<Vec<(Signature, u64)>>> = Arc::default();
    println!(
        "{n} account, {tps} tx/s richiesti, {minutes} minuti, invio {}, rpc_recent_txs {}, snapshot ogni {} blocchi",
        if via_rpc { "via HTTP/JSON-RPC (end-to-end)" } else { "diretto all'ingresso (limite del nodo)" },
        cfg.node.rpc_recent_txs,
        cfg.node.snapshot_every
    );
    let mut signers = vec![];
    for w in 0..signers_n {
        let (ingress, bh, height, sent, accepted, stop, samples) = (ingress.clone(), bh.clone(), height.clone(), sent.clone(), accepted.clone(), stop.clone(), samples.clone());
        let rate = tps as f64 / signers_n as f64;
        let senders: Vec<Keypair> = (0..SENDERS).filter(|i| i % signers_n == w).map(key).collect();
        signers.push(std::thread::spawn(move || {
            let (start, mut done, mut k) = (Instant::now(), 0u64, w);
            while !stop.load(Ordering::Relaxed) {
                // Ritmo: tx dovute finora, a lotti da 50.
                let due = (start.elapsed().as_secs_f64() * rate) as u64;
                if done >= due {
                    std::thread::sleep(Duration::from_millis(2));
                    continue;
                }
                let h = *bh.lock().unwrap();
                let batch: Vec<_> = (0..50)
                    .map(|_| {
                        k += signers_n;
                        let from = &senders[((k / signers_n) % senders.len() as u64) as usize];
                        transfer_tx(from, &addr(k.wrapping_mul(0x9E37_79B9_7F4A_7C15) % n.max(1)), 1 + k % 100_000, h)
                    })
                    .collect();
                if k % 1_000 < 50 {
                    samples.lock().unwrap().push((batch[0].signatures[0], height.load(Ordering::Relaxed)));
                }
                let ok = if via_rpc {
                    use base64::Engine;
                    let url = format!("http://{RPC}");
                    batch
                        .iter()
                        .filter(|t| {
                            let raw = base64::engine::general_purpose::STANDARD.encode(wincode::serialize(*t).unwrap());
                            call(&url, "sendTransaction", json!([raw, {"encoding": "base64", "skipPreflight": true}])).is_ok()
                        })
                        .count() as u64
                } else {
                    ingress.submit(None, batch).iter().filter(|r| r.is_ok()).count() as u64
                };
                sent.fetch_add(50, Ordering::Relaxed);
                accepted.fetch_add(ok, Ordering::Relaxed);
                done += 50;
            }
        }));
    }

    // Wallet sotto carico.
    let gh = genesis::build(&cfg.genesis).unwrap().0.genesis_hash;
    let wkey = dir.join("wallet-key.json");
    let b: Vec<String> = key(SENDERS).to_bytes().iter().map(|x| x.to_string()).collect();
    std::fs::write(&wkey, format!("[{}]", b.join(","))).unwrap();
    std::fs::write(dir.join("wallet.toml"), format!("rpc = \"http://{RPC}\"\nkeypair = \"{}\"\nchain_id = \"centrifuga:devnet\"\ngenesis_hash = \"{gh}\"\n", wkey.display())).unwrap();
    let url = format!("http://{RPC}");
    let balance = |k: &Address| call(&url, "getBalance", json!([k.to_string()])).unwrap()["value"].as_u64().unwrap();
    let parse = |s: &str| -> u64 { (s.trim().trim_end_matches(" CnFA").parse::<f64>().unwrap() * 1e9).round() as u64 };

    println!("| min | inviate | accettate | incluse | TPS effettivi | blocco p50 / p95 / p99 ms | RSS MB | journal MB | stati ok (290 blocchi) / scaduti ok (>300) | wallet |");
    let (t0, mut ticks, mut all_ticks) = (Instant::now(), vec![], vec![]);
    let (mut last_h, mut last_inc, mut last_sent, mut last_acc) = (0u64, 0u64, 0u64, 0u64);
    let (mut st_ok, mut st_bad, mut exp_ok, mut exp_bad, mut wallet_ok, mut wallet_bad) = (0, 0, 0, 0, 0, 0);
    let (mut minute, mut wallet_line) = (1u64, String::new());
    while t0.elapsed() < Duration::from_secs(minutes * 60) {
        std::thread::sleep(Duration::from_millis(40));
        let (h, us, inc, lh) = rt.block_on(gw.query(|n: &mut Node| (n.chain.height, n.last_tick_us, n.index.count, n.chain.last_hash))).unwrap();
        if h != last_h {
            ticks.push(us as f64 / 1e3);
            all_ticks.push(us as f64 / 1e3);
            last_h = h;
            *bh.lock().unwrap() = lh;
            height.store(h, Ordering::Relaxed);
        }
        if t0.elapsed() < Duration::from_secs(minute * 60) {
            continue;
        }
        // Finestra degli stati: una tx di 280-295 blocchi fa c'è, una di oltre 310 no.
        let pick = |lo: u64, hi: u64| samples.lock().unwrap().iter().rev().find(|(_, sh)| h.saturating_sub(*sh) >= lo && h.saturating_sub(*sh) <= hi).map(|x| x.0);
        if let Some(s) = pick(282, 292) {
            let v = call(&url, "getSignatureStatuses", json!([[s.to_string()]])).unwrap();
            if v["value"][0]["slot"].is_u64() { st_ok += 1 } else { st_bad += 1 }
        }
        if let Some(s) = pick(320, 400) {
            let v = call(&url, "getSignatureStatuses", json!([[s.to_string()]])).unwrap();
            if v["value"][0].is_null() { exp_ok += 1 } else { exp_bad += 1 }
        }
        if minute % 5 == 0 {
            let me = key(SENDERS).pubkey();
            let before = balance(&me);
            // Il binario accanto a target/release/examples/soak.
            let bin = std::env::current_exe().unwrap().parent().unwrap().parent().unwrap().join("centrifuga");
            let o = Command::new(bin)
                .args(["wallet", "send", &Address::new_unique().to_string(), "0.001", "--yes", "--config"])
                .arg(dir.join("wallet.toml"))
                .output()
                .unwrap();
            let after = balance(&me);
            let text = String::from_utf8_lossy(&o.stdout).to_string() + &String::from_utf8_lossy(&o.stderr);
            // "fee X, reward Y, refund Z, net cost N": il saldo deve calare di importo + costo netto.
            let line = text.lines().find(|l| l.starts_with("fee ")).unwrap_or("").to_string();
            let parts: Vec<&str> = line.split(", ").map(|p| p.rsplitn(3, ' ').collect::<Vec<_>>()).map(|v| if v.len() >= 2 { v[1] } else { "" }).collect();
            let good = o.status.success() && parts.len() == 4 && before - after == 1_000_000 + parse(parts[3]) && parse(parts[0]) == 5_000;
            if good { wallet_ok += 1 } else { wallet_bad += 1 }
            wallet_line = if good { format!("ok ({line})") } else { format!("ERRORE: {text}") };
        }
        let (s, ac) = (sent.load(Ordering::Relaxed), accepted.load(Ordering::Relaxed));
        println!(
            "| {minute} | {} | {} | {} | {:.0} | {:.1} / {:.1} / {:.1} | {} | {:.0} | {st_ok} / {exp_ok} (errori {st_bad} / {exp_bad}) | {wallet_line} |",
            s - last_sent,
            ac - last_acc,
            inc - last_inc,
            (inc - last_inc) as f64 / 60.0,
            pct(&mut ticks, 0.5),
            pct(&mut ticks, 0.95),
            pct(&mut ticks, 0.99),
            rss_mb(),
            mb(&dir.join("journal.bin")),
        );
        (last_sent, last_acc, last_inc) = (s, ac, inc);
        ticks.clear();
        wallet_line.clear();
        minute += 1;
    }
    stop.store(true, Ordering::Relaxed);
    for s in signers {
        s.join().unwrap();
    }
    // Coda svuotata: tutte le accettate devono essere incluse.
    while !ingress.is_empty() {
        std::thread::sleep(Duration::from_millis(50));
    }
    std::thread::sleep(Duration::from_millis(300));
    let (h, lh, sh, inc, stl) = rt.block_on(gw.query(|n: &mut Node| (n.chain.height, n.chain.last_hash, n.chain.state_hash(&Default::default()), n.index.count, n.index.statuses_len()))).unwrap();
    let (s, ac) = (sent.load(Ordering::Relaxed), accepted.load(Ordering::Relaxed));
    let hours = t0.elapsed().as_secs_f64() / 3600.0;
    let j = mb(&dir.join("journal.bin"));
    println!("totale: inviate {s}, accettate {ac}, rifiutate all'ingresso {}, incluse {inc}, non incluse {}", s - ac, ac.saturating_sub(inc));
    println!("TPS effettivi medi {:.0} (richiesti {tps}); altezza {h}", inc as f64 / (hours * 3600.0));
    println!("blocco (tutta la prova): p50 {:.1} / p95 {:.1} / p99 {:.1} / max {:.1} ms", pct(&mut all_ticks, 0.5), pct(&mut all_ticks, 0.95), pct(&mut all_ticks, 0.99), pct(&mut all_ticks, 1.0));
    println!("RAM: all'inizio {ram0} MB, prima del nodo; alla fine {} MB; stati in RAM {stl}", rss_mb());
    println!("journal {j:.0} MB = {:.0} byte/tx, {:.0} MB/ora; snapshot {:.0} MB; status.bin {:.1} MB", j * 1e6 / inc.max(1) as f64, j / hours, mb(&dir.join("snapshot.bin")), mb(&dir.join("status.bin")));
    println!("stati: {st_ok} trovati e {exp_ok} scaduti come atteso (errori {st_bad} / {exp_bad}); wallet sotto carico: {wallet_ok} ok, {wallet_bad} errori");
    std::fs::write(dir.join("soak.meta"), format!("{n} {ts} {h} {lh} {sh}")).unwrap();
    std::process::exit(0); // il nodo gira sul suo thread: si chiude con il processo
}

/// Riavvio da snapshot + journal e riesecuzione completa dallo stato iniziale.
fn reopen(dir: &Path) {
    let meta = std::fs::read_to_string(dir.join("soak.meta")).unwrap();
    let m: Vec<&str> = meta.split(' ').collect();
    let (ts, h): (u64, u64) = (m[1].parse().unwrap(), m[2].parse().unwrap());
    let (lh, sh): (Hash, Hash) = (m[3].parse().unwrap(), m[4].parse().unwrap());
    let cfg = config(dir, ts);
    let t = Instant::now();
    let mut node = Node::open(cfg.clone(), key(u64::MAX)).unwrap();
    let restart = t.elapsed().as_secs_f64();
    let same = (node.chain.height, node.chain.last_hash, node.chain.state_hash(&Default::default())) == (h, lh, sh);
    drop(node);
    println!("riavvio da snapshot + journal: {restart:.1} s, altezza {h}, hash {}", if same { "uguali" } else { "DIVERSI" });
    let rd = dir.join("replay");
    let _ = std::fs::remove_dir_all(&rd);
    std::fs::create_dir_all(&rd).unwrap();
    std::fs::copy(dir.join("snapshot0.bin"), rd.join("snapshot.bin")).unwrap();
    std::fs::hard_link(dir.join("journal.bin"), rd.join("journal.bin")).unwrap();
    let mut cfg = cfg;
    cfg.node.data_dir = rd.clone();
    let t = Instant::now();
    let mut node = Node::open(cfg, key(u64::MAX)).unwrap();
    let replay = t.elapsed().as_secs_f64();
    let same = (node.chain.height, node.chain.last_hash, node.chain.state_hash(&Default::default())) == (h, lh, sh);
    println!("riesecuzione completa del journal ({h} blocchi): {replay:.1} s, hash {}", if same { "uguali" } else { "DIVERSI" });
}
