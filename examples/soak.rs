//! Test di durata sul nodo reale (`node::start`: ciclo di slot, RPC, stream).
//!   cargo run --release --example soak -- <accounts> <tps> <minutes> [--rpc]
//!   cargo run --release --example soak -- --reopen <dir>     (riavvio e riesecuzione)
//! Carico: thread client firmano trasferimenti al ritmo richiesto e li consegnano
//!   - senza --rpc: direttamente all'ingresso del nodo (3 thread), una tx alla volta con
//!     arrivi di Poisson; BURSTS=1: lotti da 50 (come nei run fino al 7/11/2026). Misura il LIMITE DEL
//!     NODO, non il throughput di un utente: niente HTTP, JSON, base64.
//!   - con --rpc: via HTTP/JSON-RPC `sendTransaction` (16 thread, una richiesta per tx),
//!     come un client vero: è il numero end-to-end (client sulla stessa macchina).
//!
//! Ogni minuto: tx inviate, accettate, incluse, TPS effettivi,
//! blocco p50/p95/p99 (misurato dentro il nodo), RAM, journal, e due controlli:
//!   - getSignatureStatuses (RPC) trova una tx di ~290 blocchi fa e non una di >300;
//!   - ogni 5 minuti `centrifuga wallet send` sotto carico: fee, premio, rimborso e costo
//!     netto coerenti con il saldo letto via RPC.
//!
//! Alla fine scrive soak.meta; `--reopen` misura il riavvio da snapshot + journal e la
//! riesecuzione completa dallo stato iniziale, e confronta gli hash con il nodo vivo.
//! Ogni riga dei risultati va anche nel file $RESULTS (aggiunta in coda con fsync): un
//! arresto del processo o del container non cancella i dati già raccolti. Lancio con
//! scripts/soak-run.sh, che non riparte da capo un run interrotto.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use centrifuga::client::call;
use centrifuga::genesis::{self, Config, LAMPORTS_PER_CNFA, Prefund, transfer_tx};
use centrifuga::node::{self, Node};
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

/// Riga dei risultati: su stdout e in coda a $RESULTS, subito su disco.
fn emit(line: String) {
    use std::io::Write;
    println!("{line}");
    if let Ok(path) = std::env::var("RESULTS") {
        let mut f = std::fs::OpenOptions::new().create(true).append(true).open(path).expect("RESULTS");
        writeln!(f, "{line}").unwrap();
        f.sync_data().unwrap();
    }
}

macro_rules! say {
    ($($t:tt)*) => { emit(format!($($t)*)) };
}

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

/// Tick di CPU (utime + stime, 100 al secondo) per gruppo di thread del processo:
/// [thread del nodo, rete (tokio: RPC e stream), client di carico, altro].
fn cpu_ticks() -> [u64; 4] {
    let mut g = [0u64; 4];
    for t in std::fs::read_dir("/proc/self/task").into_iter().flatten().flatten() {
        let Ok(s) = std::fs::read_to_string(t.path().join("stat")) else { continue };
        let (Some(a), Some(b)) = (s.find('('), s.rfind(')')) else { continue };
        let f: Vec<&str> = s[b + 2..].split(' ').collect();
        let ticks = f.get(11).and_then(|x| x.parse::<u64>().ok()).unwrap_or(0) + f.get(12).and_then(|x| x.parse::<u64>().ok()).unwrap_or(0);
        let name = &s[a + 1..b];
        let i = if name == "centrifuga-node" {
            0
        } else if name.starts_with("tokio-") {
            1
        } else if name.starts_with("soak-client") {
            2
        } else {
            3
        };
        g[i] += ticks;
    }
    g
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
    // Solo per le misure: finestra del recupero dello stream (default dalla config d'esempio).
    if let Ok(v) = std::env::var("SNAPSHOT_IO_IDLE") {
        cfg.node.snapshot_io_idle = v == "1";
    }
    if let Ok(v) = std::env::var("JOURNAL_QUEUE_BLOCKS") {
        cfg.node.journal_queue_blocks = v.parse().expect("JOURNAL_QUEUE_BLOCKS");
    }
    if let Ok(v) = std::env::var("SNAPSHOT_EVERY") {
        cfg.node.snapshot_every = v.parse().expect("SNAPSHOT_EVERY");
    }
    if let Ok(v) = std::env::var("STREAM_CATCHUP_BLOCKS") {
        cfg.node.stream_catchup_blocks = v.parse().expect("STREAM_CATCHUP_BLOCKS");
    }
    cfg
}

fn initial_state(cfg: &Config, n: u64) -> genesis::GenesisConfig {
    let mut chain = genesis::build(&cfg.genesis).unwrap().0;
    let rent = chain.svm.rent_exempt(0);
    for i in 0..n {
        chain.svm.set(addr(i), Account { lamports: rent, owner: solana_sdk_ids::system_program::id(), ..Account::default() });
    }
    centrifuga::ledger::write_snapshot(&cfg.node.data_dir, 0, chain.genesis_hash, chain.full_state_hash(), chain.svm.accounts().iter()).unwrap();
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
    let cpu = std::fs::read_to_string("/proc/cpuinfo")
        .unwrap_or_default()
        .lines()
        .find(|l| l.starts_with("model name"))
        .map(|l| l.split(": ").nth(1).unwrap_or("").to_string())
        .unwrap_or_default();
    say!(
        "macchina: {} vCPU, {cpu}; commit {}; avvio {} ms (unix)",
        std::thread::available_parallelism().map_or(0, |n| n.get()),
        std::env::var("GIT_COMMIT").unwrap_or_else(|_| "?".into()),
        node::now_ms()
    );
    let ts = node::now_ms() - 86_400_000;
    let cfg = config(&dir, ts);
    initial_state(&cfg, n);
    std::fs::copy(dir.join("snapshot.bin"), dir.join("snapshot0.bin")).unwrap();
    // Prima di avviare il nodo: snapshot iniziale e copia su disco, Dirty sotto 16 MB.
    std::fs::File::open(dir.join("snapshot0.bin")).and_then(|f| f.sync_all()).unwrap();
    let dirty_kb = || {
        std::fs::read_to_string("/proc/meminfo")
            .ok()
            .and_then(|m| m.lines().find(|l| l.starts_with("Dirty:")).and_then(|l| l.split_whitespace().nth(1)?.parse::<u64>().ok()))
            .unwrap_or(0)
    };
    let waited = Instant::now();
    while dirty_kb() > 16 * 1024 && waited.elapsed() < Duration::from_secs(300) {
        std::thread::sleep(Duration::from_millis(200));
    }
    say!("avvio: snapshot iniziale su disco, Dirty {} MB dopo {:.1} s di attesa", dirty_kb() / 1024, waited.elapsed().as_secs_f64());
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
    // Latenza invio -> inclusione: stessi campioni, con l'istante dell'invio.
    let pending: Arc<Mutex<Vec<(Signature, Instant)>>> = Arc::default();
    // Ora di invio (ms unix) dei campioni e altezza in cui sono stati inclusi: per scomporre la latenza.
    let sent_ms: Arc<Mutex<std::collections::HashMap<Signature, u64>>> = Arc::default();
    let mut seen: Vec<(Signature, u64, u64)> = vec![];
    say!(
        "{n} account, {tps} tx/s richiesti, {minutes} minuti, invio {}, rpc_recent_txs {}, snapshot ogni {} blocchi, stream_catchup_blocks {}",
        if via_rpc { "via HTTP/JSON-RPC (end-to-end)" } else { "diretto all'ingresso (limite del nodo)" },
        cfg.node.rpc_recent_txs,
        cfg.node.snapshot_every,
        cfg.node.stream_catchup_blocks
    );
    // Altezza all'inizio del carico: i blocchi fino a qui sono "di preparazione".
    let load_from = rt.block_on(gw.query(|n: &mut Node| n.chain.height)).unwrap();
    let mut signers = vec![];
    for w in 0..signers_n {
        let (ingress, bh, height, sent, accepted, stop, samples, pending, sent_ms) =
            (ingress.clone(), bh.clone(), height.clone(), sent.clone(), accepted.clone(), stop.clone(), samples.clone(), pending.clone(), sent_ms.clone());
        let rate = tps as f64 / signers_n as f64;
        let senders: Vec<Keypair> = (0..SENDERS).filter(|i| i % signers_n == w).map(key).collect();
        signers.push(
            std::thread::Builder::new()
                .name(format!("soak-client-{w}"))
                .spawn(move || {
                    let (start, mut done, mut k) = (Instant::now(), 0u64, w);
                    // Default: una tx alla volta a intervalli esponenziali (arrivi di Poisson,
                    // nessuna raffica); campione ~1 tx ogni 1.000. BURSTS=1: lotti da 50.
                    if !via_rpc && !std::env::var("BURSTS").is_ok_and(|v| v == "1") {
                        let (mut rng, mut next) = (0x9E37_79B9_7F4A_7C15u64 ^ (w + 1).wrapping_mul(0xD1B5_4A32_D192_ED03), 0f64);
                        while !stop.load(Ordering::Relaxed) {
                            rng ^= rng << 13;
                            rng ^= rng >> 7;
                            rng ^= rng << 17;
                            let u = ((rng >> 11) as f64 + 1.0) / (1u64 << 53) as f64;
                            next += -u.ln() / rate;
                            let wait = next - start.elapsed().as_secs_f64();
                            if wait > 0.0 {
                                std::thread::sleep(Duration::from_secs_f64(wait));
                            }
                            k += signers_n;
                            let from = &senders[((k / signers_n) % senders.len() as u64) as usize];
                            let tx = transfer_tx(from, &addr(k.wrapping_mul(0x9E37_79B9_7F4A_7C15) % n.max(1)), 1 + k % 100_000, *bh.lock().unwrap());
                            let sig = tx.signatures[0];
                            if k % 1_000 < signers_n {
                                samples.lock().unwrap().push((sig, height.load(Ordering::Relaxed)));
                                pending.lock().unwrap().push((sig, Instant::now()));
                                sent_ms.lock().unwrap().insert(sig, node::now_ms());
                            }
                            let ok = ingress.submit(None, vec![tx]).iter().filter(|r| r.is_ok()).count() as u64;
                            sent.fetch_add(1, Ordering::Relaxed);
                            accepted.fetch_add(ok, Ordering::Relaxed);
                        }
                        return;
                    }
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
                            pending.lock().unwrap().push((batch[0].signatures[0], Instant::now())); // batch[0] parte per prima
                            sent_ms.lock().unwrap().insert(batch[0].signatures[0], node::now_ms());
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
                })
                .unwrap(),
        );
    }

    // Wallet sotto carico.
    let gh = genesis::build(&cfg.genesis).unwrap().0.genesis_hash;
    let wkey = dir.join("wallet-key.json");
    let b: Vec<String> = key(SENDERS).to_bytes().iter().map(|x| x.to_string()).collect();
    std::fs::write(&wkey, format!("[{}]", b.join(","))).unwrap();
    std::fs::write(
        dir.join("wallet.toml"),
        format!("rpc = \"http://{RPC}\"\nkeypair = \"{}\"\nchain_id = \"centrifuga:devnet\"\ngenesis_hash = \"{gh}\"\n", wkey.display()),
    )
    .unwrap();
    let url = format!("http://{RPC}");
    let balance = |k: &Address| call(&url, "getBalance", json!([k.to_string()])).unwrap()["value"].as_u64().unwrap();
    let parse = |s: &str| -> u64 { (s.trim().trim_end_matches(" CnFA").parse::<f64>().unwrap() * 1e9).round() as u64 };

    say!(
        "CPU in % di un core: nodo = thread dei blocchi; rete = runtime tokio (RPC e stream, lato nodo); client = thread che firmano e inviano (in modalità diretta includono la verifica delle firme all'ingresso)"
    );
    say!(
        "| min | inviate | accettate | incluse | TPS effettivi | blocco p50 / p95 / p99 ms | invio→inclusione p50 / p95 / p99 ms | CPU nodo / rete / client / altro % | RSS MB | journal MB | stati ok (290 blocchi) / scaduti ok (>300) | wallet |"
    );
    let (t0, mut ticks, mut all_ticks) = (Instant::now(), vec![], vec![]);
    let mut snap_pauses: Vec<f64> = vec![];
    let every = cfg.node.snapshot_every;
    let (mut last_h, mut last_inc, mut last_sent, mut last_acc) = (0u64, 0u64, 0u64, 0u64);
    let (mut st_ok, mut st_bad, mut exp_ok, mut exp_bad, mut wallet_ok, mut wallet_bad) = (0, 0, 0, 0, 0, 0);
    let (mut minute, mut wallet_line) = (1u64, String::new());
    let (mut lat, mut all_lat) = (vec![], vec![]);
    let (mut cpu_last, mut cpu_total) = (cpu_ticks(), [0u64; 4]);
    // Snapshot: (altezza, RSS subito prima, istante + 60 s); poi la riga con il RSS dopo 1 minuto.
    let (mut rss_last, mut snap_rss, mut snap_lines) = (rss_mb(), Vec::<(u64, u64, Instant)>::new(), Vec::<(u64, u64, u64)>::new());
    let dirty_samples: Mutex<Vec<(u64, u64)>> = Mutex::new(vec![]);
    while t0.elapsed() < Duration::from_secs(minutes * 60) {
        std::thread::sleep(Duration::from_millis(40));
        // Dirty di /proc/meminfo (kB) a ogni giro, per il picco durante le scritture.
        if let Some(kb) = std::fs::read_to_string("/proc/meminfo")
            .ok()
            .and_then(|m| m.lines().find(|l| l.starts_with("Dirty:")).and_then(|l| l.split_whitespace().nth(1)?.parse().ok()))
        {
            dirty_samples.lock().unwrap().push((node::now_ms(), kb));
        }
        let (h, us, inc, lh, snap_us) =
            rt.block_on(gw.query(|n: &mut Node| (n.chain.height, n.last_tick_us, n.index.count, n.chain.last_hash, n.last_snapshot_us))).unwrap();
        if h != last_h {
            ticks.push(us as f64 / 1e3);
            all_ticks.push(us as f64 / 1e3);
            // Multiplo di `every` superato (il blocco dello snapshot può non essere mai
            // osservato: sotto carico il nodo produce il successivo prima della query).
            if every > 0 && last_h > 0 && last_h / every < h / every {
                snap_pauses.push(snap_us as f64 / 1e3); // scrittura dello snapshot, dentro il suo tick
                snap_rss.push((h / every * every, rss_last, Instant::now() + Duration::from_secs(60)));
            }

            last_h = h;
            *bh.lock().unwrap() = lh;
            height.store(h, Ordering::Relaxed);
        }
        // Campioni con lo stato visibile (blocco durevole): latenza dall'invio, controllata a
        // ogni giro (risoluzione ~40 ms), non solo quando cambia l'altezza eseguita.
        let waiting: Vec<(Signature, Instant)> = std::mem::take(&mut *pending.lock().unwrap());
        if !waiting.is_empty() {
            let sigs: Vec<Signature> = waiting.iter().map(|w| w.0).collect();
            let found = rt.block_on(gw.query(move |n: &mut Node| sigs.iter().map(|s| n.index.status(s).map(|x| x.0)).collect::<Vec<_>>())).unwrap();
            let mut back = vec![];
            for (w, f) in waiting.into_iter().zip(found) {
                match f {
                    Some(slot) => {
                        lat.push(w.1.elapsed().as_secs_f64() * 1e3);
                        seen.push((w.0, slot, node::now_ms()));
                    }
                    None => back.push(w),
                }
            }
            pending.lock().unwrap().extend(back);
        }
        let rss = rss_mb();
        snap_rss.retain(|&(sh, before, due)| {
            if Instant::now() < due {
                return true;
            }
            say!("snapshot all'altezza {sh}: RSS subito prima {before} MB, 1 minuto dopo {rss} MB ({:+} MB)", rss as i64 - before as i64);
            snap_lines.push((sh, before, rss));
            false
        });
        rss_last = rss;
        if t0.elapsed() < Duration::from_secs(minute * 60) {
            continue;
        }
        // Finestra degli stati: una tx di 280-295 blocchi fa c'è, una di oltre 310 no.
        let pick =
            |lo: u64, hi: u64| samples.lock().unwrap().iter().rev().find(|(_, sh)| h.saturating_sub(*sh) >= lo && h.saturating_sub(*sh) <= hi).map(|x| x.0);
        if let Some(s) = pick(282, 292) {
            let v = call(&url, "getSignatureStatuses", json!([[s.to_string()]])).unwrap();
            if v["value"][0]["slot"].is_u64() { st_ok += 1 } else { st_bad += 1 }
        }
        if let Some(s) = pick(320, 400) {
            let v = call(&url, "getSignatureStatuses", json!([[s.to_string()]])).unwrap();
            if v["value"][0].is_null() { exp_ok += 1 } else { exp_bad += 1 }
        }
        let every: u64 = std::env::var("WALLET_EVERY").ok().and_then(|v| v.parse().ok()).unwrap_or(5);
        if minute % every == 0 {
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
            if good {
                wallet_ok += 1
            } else {
                wallet_bad += 1
            }
            wallet_line = if good { format!("ok ({line})") } else { format!("ERRORE: {text}") };
        }
        let (s, ac) = (sent.load(Ordering::Relaxed), accepted.load(Ordering::Relaxed));
        let cpu = cpu_ticks();
        // saturating: un thread finito (es. quello dello snapshot) esce dal conteggio.
        let d: Vec<u64> = (0..4).map(|i| cpu[i].saturating_sub(cpu_last[i])).collect();
        (0..4).for_each(|i| cpu_total[i] += d[i]);
        cpu_last = cpu;
        let secs = 60.0; // tick a 100 Hz: % di un core = tick / secondi
        say!(
            "| {minute} | {} | {} | {} | {:.0} | {:.1} / {:.1} / {:.1} | {:.0} / {:.0} / {:.0} | {:.0} / {:.0} / {:.0} / {:.0} | {} | {:.0} | {st_ok} / {exp_ok} (errori {st_bad} / {exp_bad}) | {wallet_line} |",
            s - last_sent,
            ac - last_acc,
            inc - last_inc,
            (inc - last_inc) as f64 / 60.0,
            pct(&mut ticks, 0.5),
            pct(&mut ticks, 0.95),
            pct(&mut ticks, 0.99),
            pct(&mut lat, 0.5),
            pct(&mut lat, 0.95),
            pct(&mut lat, 0.99),
            d[0] as f64 / secs,
            d[1] as f64 / secs,
            d[2] as f64 / secs,
            d[3] as f64 / secs,
            rss_mb(),
            mb(&dir.join("journal.bin")),
        );
        (last_sent, last_acc, last_inc) = (s, ac, inc);
        ticks.clear();
        all_lat.append(&mut lat);
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
    let (h, lh, sh, inc, stl, skipped) = rt
        .block_on(gw.query(|n: &mut Node| {
            (
                n.chain.height,
                n.chain.last_hash,
                n.chain.state_hash(),
                n.index.count,
                n.index.statuses_len(),
                n.shadow_stats().map_or((0, (0, 0)), |s| {
                    (s.skipped.load(Ordering::Relaxed), (s.last_write_ms.load(Ordering::Relaxed), s.max_queue.load(Ordering::Relaxed)))
                }),
            )
        }))
        .unwrap();
    let (s, ac) = (sent.load(Ordering::Relaxed), accepted.load(Ordering::Relaxed));
    let hours = t0.elapsed().as_secs_f64() / 3600.0;
    let j = mb(&dir.join("journal.bin"));
    say!("totale: inviate {s}, accettate {ac}, rifiutate all'ingresso {}, incluse {inc}, non incluse {}", s - ac, ac.saturating_sub(inc));
    say!("TPS effettivi medi {:.0} (richiesti {tps}); altezza {h}", inc as f64 / (hours * 3600.0));
    say!(
        "blocco (tutta la prova): p50 {:.1} / p95 {:.1} / p99 {:.1} / max {:.1} ms",
        pct(&mut all_ticks, 0.5),
        pct(&mut all_ticks, 0.95),
        pct(&mut all_ticks, 0.99),
        pct(&mut all_ticks, 1.0)
    );
    say!("RAM: all'inizio {ram0} MB, prima del nodo; alla fine {} MB; stati in RAM {stl}", rss_mb());
    // Scomposizione della latenza dei campioni: invio -> eseguito -> durevole -> pubblicato -> visto.
    let times: std::collections::HashMap<u64, (u64, u64, u64)> =
        rt.block_on(gw.query(|n: &mut Node| n.block_log.iter().map(|b| (b.height, (b.executed_ms, b.durable_ms, b.published_ms))).collect())).unwrap();
    let sm = sent_ms.lock().unwrap().clone();
    let (mut a, mut b, mut c, mut d) = (vec![], vec![], vec![], vec![]);
    for (s, slot, seen_ms) in &seen {
        if let (Some(t0), Some((ex, du, pu))) = (sm.get(s), times.get(slot)) {
            a.push(ex.saturating_sub(*t0) as f64);
            b.push(du.saturating_sub(*ex) as f64);
            c.push(pu.saturating_sub(*du) as f64);
            d.push(seen_ms.saturating_sub(*pu) as f64);
        }
    }
    say!(
        "latenza scomposta ({} campioni), p50 / p99 ms: invio -> eseguito {:.0} / {:.0}; eseguito -> durevole {:.0} / {:.0}; durevole -> pubblicato {:.0} / {:.0}; pubblicato -> visto dal soak {:.0} / {:.0}",
        a.len(),
        pct(&mut a, 0.5),
        pct(&mut a, 0.99),
        pct(&mut b, 0.5),
        pct(&mut b, 0.99),
        pct(&mut c, 0.5),
        pct(&mut c, 0.99),
        pct(&mut d, 0.5),
        pct(&mut d, 0.99)
    );
    // Istogramma invio -> eseguito a passi di 10 ms; fase dell'invio rispetto all'ultimo
    // blocco eseguito; blocchi eseguiti tra l'invio e quello che include la tx.
    {
        let mut execs: Vec<u64> = times.values().map(|t| t.0).collect();
        execs.sort_unstable();
        let (mut hist, mut phase, mut skip) = ([0u32; 31], [0u32; 31], [0u32; 6]);
        for (s, slot, _) in &seen {
            if let (Some(t0), Some((ex, _, _))) = (sm.get(s), times.get(slot)) {
                hist[(ex.saturating_sub(*t0) / 10).min(30) as usize] += 1;
                let (i, j) = (execs.partition_point(|e| e <= t0), execs.partition_point(|e| e < ex));
                if i > 0 {
                    phase[((t0 - execs[i - 1]) / 10).min(30) as usize] += 1;
                }
                skip[j.saturating_sub(i).min(5)] += 1;
            }
        }
        let fmt = |h: &[u32]| h.iter().enumerate().map(|(k, c)| format!("{}:{c}", k * 10)).collect::<Vec<_>>().join(" ");
        say!("istogramma invio -> eseguito (inizio del passo da 10 ms: campioni; 300 = oltre): {}", fmt(&hist));
        say!("fase dell'invio dopo l'ultimo blocco eseguito (passi da 10 ms): {}", fmt(&phase));
        say!("blocchi eseguiti tra l'invio e il blocco della tx (0 = il primo utile): {:?}", skip);
    }
    let mut dl: Vec<f64> = rt.block_on(gw.query(|n: &mut Node| n.block_log.iter().map(|b| b.queued_us as f64 / 1e3).collect::<Vec<_>>())).unwrap();
    say!(
        "dall'accodamento al journal alla durabilità: p50 {:.1} / p99 {:.1} / max {:.1} ms (il blocco sopra è il tempo sul thread del nodo, attesa per coda piena compresa)",
        pct(&mut dl, 0.5),
        pct(&mut dl, 0.99),
        pct(&mut dl, 1.0)
    );
    // sync_data del journal: nella finestra reale di ogni snapshot (dall'inizio al rename),
    // nella finestra fissa di 600 blocchi dall'altezza dello snapshot, e nel resto.
    let (sync_log, windows) = rt
        .block_on(gw.query(|n: &mut Node| {
            (
                n.block_log.iter().map(|b| (b.height, b.durable_ms, b.sync_us)).collect::<Vec<_>>(),
                n.shadow_stats().map_or(vec![], |s| s.windows.lock().unwrap().clone()),
            )
        }))
        .unwrap();
    let dirty = dirty_samples.lock().unwrap().clone();
    let every_s = cfg.node.snapshot_every;
    say!("");
    say!("| snapshot (altezza) | durata s | picco Dirty MB | blocchi nella finestra | sync_data p99 / max ms |");
    say!("|---|---|---|---|---|");
    let (mut real, mut fixed, mut rest): (Vec<f64>, Vec<f64>, Vec<f64>) = (vec![], vec![], vec![]);
    for (h, t0, t1) in &windows {
        let mut w: Vec<f64> = sync_log.iter().filter(|(_, t, _)| t >= t0 && t <= t1).map(|x| x.2 as f64 / 1e3).collect();
        let peak = dirty.iter().filter(|(t, _)| t >= t0 && t <= t1).map(|x| x.1).max().unwrap_or(0);
        say!("| {h} | {:.1} | {} | {} | {:.1} / {:.1} |", (t1 - t0) as f64 / 1e3, peak / 1024, w.len(), pct(&mut w, 0.99), pct(&mut w, 1.0));
        real.append(&mut w);
    }
    for (h, t, us) in &sync_log {
        let ms = *us as f64 / 1e3;
        if windows.iter().any(|(_, t0, t1)| t >= t0 && t <= t1) {
            continue;
        }
        if every_s > 0 && *h >= every_s && h % every_s < 600 { fixed.push(ms) } else { rest.push(ms) }
    }
    let mut fixed_all = fixed.clone();
    fixed_all.extend(real.iter().copied());
    say!("");
    say!(
        "sync_data del journal: finestra reale degli snapshot ({} blocchi) p99 {:.1} / max {:.1} ms; finestra fissa di 600 blocchi ({} blocchi) p99 {:.1} / max {:.1} ms; resto ({} blocchi) p99 {:.1} / max {:.1} ms; picco Dirty in tutta la prova {} MB",
        real.len(),
        pct(&mut real, 0.99),
        pct(&mut real, 1.0),
        fixed_all.len(),
        pct(&mut fixed_all, 0.99),
        pct(&mut fixed_all, 1.0),
        rest.len(),
        pct(&mut rest, 0.99),
        pct(&mut rest, 1.0),
        dirty.iter().map(|x| x.1).max().unwrap_or(0) / 1024
    );
    say!(
        "invio→inclusione ({} campioni, 1 ogni ~1000 tx, risoluzione ~40 ms): p50 {:.0} / p95 {:.0} / p99 {:.0} / max {:.0} ms",
        all_lat.len(),
        pct(&mut all_lat, 0.5),
        pct(&mut all_lat, 0.95),
        pct(&mut all_lat, 0.99),
        pct(&mut all_lat, 1.0)
    );
    let secs = (minute - 1).max(1) as f64 * 60.0;
    say!(
        "CPU media (% di un core): nodo {:.0}, rete {:.0}, client {:.0}, altro {:.0}",
        cpu_total[0] as f64 / secs,
        cpu_total[1] as f64 / secs,
        cpu_total[2] as f64 / secs,
        cpu_total[3] as f64 / secs
    );
    // Il salto di uno snapshot rientra se il RSS prima dello snapshot successivo torna
    // vicino a quello prima del precedente; si accumula se riparte dal valore "dopo".
    say!(
        "RSS intorno agli snapshot (altezza: prima -> 1 min dopo): {}",
        snap_lines.iter().map(|(h, b, a)| format!("{h}: {b} -> {a}")).collect::<Vec<_>>().join(", ")
    );
    say!(
        "journal {j:.0} MB = {:.0} byte/tx, {:.0} MB/ora; snapshot {:.0} MB; status.bin {:.1} MB",
        j * 1e6 / inc.max(1) as f64,
        j / hours,
        mb(&dir.join("snapshot.bin")),
        mb(&dir.join("status.bin"))
    );
    say!("stati: {st_ok} trovati e {exp_ok} scaduti come atteso (errori {st_bad} / {exp_bad}); wallet sotto carico: {wallet_ok} ok, {wallet_bad} errori");
    say!(
        "snapshot durante il carico: {} (pausa sul thread del nodo: codifica degli stati e invio allo stato ombra; copia e scrittura sono in background: {}), massimo {:.0} ms",
        snap_pauses.len(),
        snap_pauses.iter().map(|p| format!("{p:.0} ms")).collect::<Vec<_>>().join(", "),
        snap_pauses.iter().cloned().fold(0.0, f64::max)
    );
    let (skipped, (write_ms, max_queue)) = skipped;
    let slow = rt.block_on(gw.query(|n: &mut Node| n.slow_blocks.clone())).unwrap();
    let ms = |d: std::time::Duration| d.as_secs_f64() * 1e3;
    say!(
        "blocchi oltre 100 ms: {} {}",
        slow.len(),
        slow.iter()
            .map(|(h, us, p)| format!(
                "[{h}: {:.0} ms = coda del journal piena {:.0} + attesa CPU {:.0} + esecuzione {:.0} + hash {:.0} + firma {:.0} + altro {:.0}]",
                *us as f64 / 1e3,
                ms(p.queue),
                ms(p.cpu_wait),
                ms(p.exec),
                ms(p.hash),
                ms(p.sign),
                ms(p.other)
            ))
            .collect::<Vec<_>>()
            .join(" ")
    );
    say!(
        "snapshot saltati (scrittura in corso, coda piena o ombra persa): {skipped}; ultima scrittura {write_ms} ms; coda massima durante una scrittura {max_queue} blocchi"
    );
    std::fs::write(dir.join("soak.meta"), format!("{n} {ts} {h} {lh} {sh} {load_from}")).unwrap();
    std::process::exit(0); // il nodo gira sul suo thread: si chiude con il processo
}

/// Riavvio da snapshot + journal; poi riesecuzione completa blocco per blocco dallo
/// stato iniziale, con il tempo separato per blocchi di preparazione e di carico, e
/// confronto dell'hash finale con il nodo vivo.
fn reopen(dir: &Path) {
    use std::os::unix::fs::FileExt;
    let meta = std::fs::read_to_string(dir.join("soak.meta")).unwrap();
    let m: Vec<&str> = meta.split(' ').collect();
    let (ts, h, load_from): (u64, u64, u64) = (m[1].parse().unwrap(), m[2].parse().unwrap(), m[5].parse().unwrap());
    let (lh, sh): (Hash, Hash) = (m[3].parse().unwrap(), m[4].parse().unwrap());
    let cfg = config(dir, ts);
    let t = Instant::now();
    let node = Node::open(cfg.clone(), key(u64::MAX)).unwrap();
    let restart = t.elapsed().as_secs_f64();
    let same = (node.chain.height, node.chain.last_hash, node.chain.state_hash()) == (h, lh, sh);
    let snap_h = centrifuga::ledger::read_snapshot(dir).unwrap().map_or(0, |s| s.height);
    drop(node);
    say!("riavvio da snapshot (altezza {snap_h}) + journal: {restart:.1} s, altezza {h}, hash {}", if same { "uguali" } else { "DIVERSI" });
    // Riesecuzione completa dallo stato iniziale (snapshot0), un blocco alla volta.
    let rd = dir.join("replay");
    let _ = std::fs::remove_dir_all(&rd);
    std::fs::create_dir_all(&rd).unwrap();
    std::fs::copy(dir.join("snapshot0.bin"), rd.join("snapshot.bin")).unwrap();
    let t = Instant::now();
    let (mut chain, _) = node::chain_from_snapshot(&cfg, &rd).unwrap();
    let load_state = t.elapsed().as_secs_f64();
    let f = std::fs::File::open(dir.join("journal.bin")).unwrap();
    let len = f.metadata().unwrap().len();
    let (mut off, mut prep, mut load, mut tail, mut failed) = (0u64, vec![], vec![], vec![], 0usize);
    let all = Instant::now();
    while off + 4 <= len {
        let mut n = [0u8; 4];
        f.read_exact_at(&mut n, off).unwrap();
        let n = u32::from_le_bytes(n) as u64;
        if off + 4 + n > len {
            break;
        }
        let mut b = vec![0u8; n as usize];
        f.read_exact_at(&mut b, off + 4).unwrap();
        off += 4 + n;
        let blk = centrifuga::ledger::Block::decode(&b).unwrap();
        let (bh, txs) = (blk.header.height, blk.txs.len());
        let t = Instant::now();
        failed += chain.apply(&blk).unwrap().results.iter().filter(|r| r.is_err()).count();
        let ms = t.elapsed().as_secs_f64() * 1e3;
        match () {
            _ if bh <= load_from => prep.push((ms, txs)),
            _ if txs > 0 => load.push((ms, txs)),
            _ => tail.push((ms, txs)),
        }
    }
    let total = all.elapsed().as_secs_f64();
    let same = (chain.height, chain.last_hash, chain.state_hash()) == (h, lh, sh) && chain.state_hash() == chain.full_state_hash();
    let sum = |v: &[(f64, usize)]| {
        let (ms, tx): (f64, usize) = v.iter().fold((0.0, 0), |a, x| (a.0 + x.0, a.1 + x.1));
        format!("{} blocchi, {tx} tx, {:.1} s, {:.1} ms/blocco", v.len(), ms / 1e3, ms / v.len().max(1) as f64)
    };
    say!("riesecuzione completa: stato iniziale {load_state:.1} s, journal {total:.1} s (altezza {})", chain.height);
    say!("  preparazione (fino all'altezza {load_from}): {}", sum(&prep));
    say!("  carico: {}", sum(&load));
    say!("  tx incluse ma fallite (eseguite con errore, contate rieseguendo il journal): {failed}");
    say!("  blocchi vuoti dopo l'inizio del carico (battito, coda): {}", sum(&tail));
    say!("  hash finale (blocco, stato incrementale, stato ricalcolato) {}", if same { "uguali al nodo vivo" } else { "DIVERSI" });
}
