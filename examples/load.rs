//! Load test: N accounts, blocks of real signed transfers.
//!   cargo run --release --example load -- <accounts> [blocks] [txs_per_block] [--rpc]
//! With --rpc it also measures the end-to-end path: the real node (`node::start`)
//! with concurrent HTTP clients sending `sendTransaction`.
//! Measures: setup, snapshot size and write time, restart time (snapshot + full
//! LtHash), RAM (RSS), per-block time (ingress verification and execution
//! separately) and sustained TPS. Prints one summary line in Markdown at the end.

use std::path::PathBuf;
use std::time::Instant;

use centrifuga::genesis::{self, Config, LAMPORTS_PER_CNFA, Prefund, transfer_tx};
use centrifuga::node::Node;
use solana_account::Account;
use solana_address::Address;
use solana_keypair::Keypair;
use solana_signer::Signer;

const EXAMPLE: &str = include_str!("../centrifuga.example.toml");
const SENDERS: usize = 2_000;

fn rss_mb() -> (u64, u64) {
    let s = std::fs::read_to_string("/proc/self/status").unwrap_or_default();
    let kb = |k: &str| s.lines().find(|l| l.starts_with(k)).and_then(|l| l.split_whitespace().nth(1)?.parse::<u64>().ok()).unwrap_or(0);
    (kb("VmRSS:") / 1024, kb("VmHWM:") / 1024)
}

fn addr(i: u64) -> Address {
    let mut b = [0u8; 32];
    b[..8].copy_from_slice(&i.to_le_bytes());
    b[8] = 0xCA; // fuori dagli indirizzi di sistema
    Address::new_from_array(b)
}

fn main() {
    let rpc_mode = std::env::args().any(|a| a == "--rpc");
    let args: Vec<u64> = std::env::args().skip(1).filter(|a| a != "--rpc").map(|a| a.replace('_', "").parse().expect("number")).collect();
    let (n, blocks, per_block) = (args[0], *args.get(1).unwrap_or(&20), *args.get(2).unwrap_or(&5_000));
    let op = Keypair::new();
    let senders: Vec<Keypair> = (0..SENDERS).map(|_| Keypair::new()).collect();
    let mut cfg: Config = toml::from_str(EXAMPLE).unwrap();
    cfg.genesis.operator = op.pubkey().to_string();
    cfg.genesis.network = "devnet".into();
    cfg.genesis.chain_id = "centrifuga:devnet".into();
    cfg.genesis.timestamp_ms = centrifuga::node::now_ms() - 86_400_000;
    cfg.genesis.prefund = senders.iter().map(|k| Prefund { pubkey: k.pubkey().to_string(), lamports: 100 * LAMPORTS_PER_CNFA }).collect();
    cfg.node.max_block_txs = per_block as usize;
    cfg.node.snapshot_every = 0; // snapshot misurato a parte
    cfg.node.ingress_pending_per_payer = u32::MAX;
    let dir = std::env::var("LOAD_DIR").map(PathBuf::from).unwrap_or_else(|_| std::env::temp_dir().join("centrifuga-load"));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    cfg.node.data_dir = dir.clone();

    // Stato con N account (oltre a genesis e mittenti), salvato come snapshot all'altezza 0.
    let t = Instant::now();
    let mut chain = genesis::build(&cfg.genesis).unwrap().0;
    let rent = chain.svm.rent_exempt(0);
    let system = solana_sdk_ids::system_program::id();
    for i in 0..n {
        chain.svm.set(addr(i), Account { lamports: rent, owner: system, ..Account::default() });
    }
    chain.rehash();
    let setup_s = t.elapsed().as_secs_f64();
    let t = Instant::now();
    centrifuga::ledger::write_snapshot(&dir, 0, chain.genesis_hash, chain.full_state_hash(), chain.svm.accounts().iter()).unwrap();
    let snap_write_s = t.elapsed().as_secs_f64();
    let snap_mb = std::fs::metadata(centrifuga::ledger::snapshot_path(&dir)).unwrap().len() as f64 / 1e6;
    let total_accounts = chain.svm.accounts().len();
    drop(chain);

    // Riavvio: lettura snapshot, ricostruzione litesvm, LtHash completo.
    let t = Instant::now();
    let mut node = Node::open(cfg.clone(), Keypair::try_from(op.to_bytes().as_slice()).unwrap()).unwrap();
    let restart_s = t.elapsed().as_secs_f64();
    assert_eq!(node.chain.state_hash(), node.chain.full_state_hash());
    let (rss_open, _) = rss_mb();

    // Transazioni firmate in anticipo (lato client): mittente a rotazione, destinatario casuale tra gli N.
    let bh = node.chain.last_hash;
    let mut seed = 0x9E37_79B9_7F4A_7C15u64;
    let mut rnd = || {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        seed
    };
    let all: Vec<Vec<_>> = (0..blocks)
        .map(|b| (0..per_block).map(|j| transfer_tx(&senders[(j as usize) % SENDERS], &addr(rnd() % n.max(1)), 1 + b, bh)).collect())
        .collect();

    let (mut verify, mut exec, mut included) = (0f64, Vec::new(), 0usize);
    let mut now = centrifuga::node::now_ms();
    let wall = Instant::now();
    for txs in all {
        let t = Instant::now();
        let r = node.ingress.submit(None, txs);
        assert!(r.iter().all(|x| x.is_ok()), "ingress rejected a tx");
        verify += t.elapsed().as_secs_f64();
        now += 100;
        let t = Instant::now();
        let ex = node.tick(now).unwrap();
        exec.push(t.elapsed().as_secs_f64());
        included += ex.block.txs.len();
    }
    let wall_s = wall.elapsed().as_secs_f64();
    assert_eq!(node.chain.state_hash(), node.chain.full_state_hash(), "LtHash incrementale");
    let t = Instant::now();
    node.snapshot().unwrap();
    node.wait_snapshot().unwrap();
    let snap2_s = t.elapsed().as_secs_f64();
    let (rss_end, hwm) = rss_mb();
    drop(node);
    let t = Instant::now();
    let node = Node::open(cfg.clone(), Keypair::try_from(op.to_bytes().as_slice()).unwrap()).unwrap();
    let restart2_s = t.elapsed().as_secs_f64();
    let bh = node.chain.last_hash;
    drop(node);
    let rpc = rpc_mode.then(|| rpc_tps(cfg, &op, &senders, n, bh, blocks * per_block));

    exec.sort_by(f64::total_cmp);
    let mean = exec.iter().sum::<f64>() / exec.len() as f64;
    let p = |q: f64| exec[((exec.len() - 1) as f64 * q) as usize] * 1e3;
    let tps_exec = included as f64 / exec.iter().sum::<f64>();
    let tps_total = included as f64 / wall_s;
    println!("accounts total {total_accounts}; setup {setup_s:.1} s; RSS after open {rss_open} MB");
    println!("included {included} txs in {blocks} blocks of {per_block}");
    println!("block exec: mean {:.1} ms, p50 {:.1} ms, max {:.1} ms; ingress verify {:.1} ms/block", mean * 1e3, p(0.5), p(1.0), verify / blocks as f64 * 1e3);
    println!(
        "| {n} | {:.1} | {:.0} | {:.0} | {rss_end} (peak {hwm}) | {snap_mb:.0} | {snap_write_s:.1} / {snap2_s:.1} | {restart_s:.1} / {restart2_s:.1} |",
        mean * 1e3, tps_exec, tps_total
    );
    if let Some((sent_s, incl_s, count, preflight)) = rpc {
        println!("rpc: {count} txs sent in {sent_s:.1} s ({:.0}/s), all included after {incl_s:.1} s -> {:.0} TPS end-to-end; preflight sample: {preflight:.0}/s",
            count as f64 / sent_s, count as f64 / incl_s);
    }
}

/// CPU per thread (secondi): (tid, nome) -> utime + stime.
fn thread_cpu() -> std::collections::HashMap<(u64, String), f64> {
    let mut out = std::collections::HashMap::new();
    for e in std::fs::read_dir("/proc/self/task").into_iter().flatten().flatten() {
        let Ok(st) = std::fs::read_to_string(e.path().join("stat")) else { continue };
        let (Some(open), Some(close)) = (st.find('('), st.rfind(')')) else { continue };
        let tid = st[..open].trim().parse().unwrap_or(0);
        let f: Vec<&str> = st[close + 2..].split_whitespace().collect();
        let ticks = f[11].parse::<f64>().unwrap_or(0.0) + f[12].parse::<f64>().unwrap_or(0.0);
        out.insert((tid, st[open + 1..close].to_string()), ticks / 100.0);
    }
    out
}

/// Real node with RPC: CLIENTS threads send pre-signed txs (skipPreflight) over
/// HTTP; time until all are included. Then a short sample with preflight.
fn rpc_tps(mut cfg: Config, op: &Keypair, senders: &[Keypair], n: u64, bh: solana_hash::Hash, count: u64) -> (f64, f64, u64, f64) {
    use base64::Engine;
    use centrifuga::rpc::client::call;
    use serde_json::json;
    const CLIENTS: usize = 16;
    cfg.node.rpc_addr = "127.0.0.1:19300".into();
    cfg.node.stream_addr = "127.0.0.1:19302".into();
    cfg.node.ingress_tps_per_ip = u32::MAX;
    let running = centrifuga::node::start(cfg, Keypair::try_from(op.to_bytes().as_slice()).unwrap()).unwrap();
    let gw = running.gateway.clone();
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.spawn(running.serve_network());
    std::thread::sleep(std::time::Duration::from_millis(500));
    let enc = |t: &solana_transaction::versioned::VersionedTransaction| base64::engine::general_purpose::STANDARD.encode(wincode::serialize(t).unwrap());
    let raws: Vec<String> = (0..count).map(|i| enc(&transfer_tx(&senders[(i as usize) % SENDERS], &addr((i * 7919) % n.max(1)), 1_000 + i, bh))).collect();
    let indexed = || rt.block_on(gw.query(|nd: &mut Node| nd.index.count as usize)).unwrap();
    let base = indexed();
    let url = "http://127.0.0.1:19300";
    // Tid del thread del nodo: letto dal thread stesso.
    let node_tid: u64 = rt.block_on(gw.query(|_| std::fs::read_to_string("/proc/thread-self/stat").unwrap().split(' ').next().unwrap().parse().unwrap())).unwrap();
    let cpu0 = thread_cpu();
    let t = Instant::now();
    std::thread::scope(|s| {
        for chunk in raws.chunks(raws.len().div_ceil(CLIENTS)) {
            s.spawn(move || {
                for r in chunk {
                    call(url, "sendTransaction", json!([r, {"encoding": "base64", "skipPreflight": true}])).unwrap();
                }
            });
        }
    });
    let sent_s = t.elapsed().as_secs_f64();
    let cpu1 = thread_cpu();
    let (mut node_cpu, mut tokio_cpu, mut other_cpu) = (0.0, 0.0, 0.0);
    for (k, v) in &cpu1 {
        let d = v - cpu0.get(k).copied().unwrap_or(0.0);
        if k.0 == node_tid {
            node_cpu += d;
        } else if k.1.starts_with("tokio") {
            tokio_cpu += d;
        } else {
            other_cpu += d;
        }
    }
    println!(
        "rpc CPU durante l'invio ({sent_s:.1} s): thread del nodo {:.0}%, tokio (HTTP, JSON) {:.0}%, client e altro {:.0}% (su {} core)",
        node_cpu / sent_s * 100.0,
        tokio_cpu / sent_s * 100.0,
        other_cpu / sent_s * 100.0,
        std::thread::available_parallelism().map_or(0, |n| n.get())
    );
    while indexed() < base + count as usize {
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    let incl_s = t.elapsed().as_secs_f64();
    // Con preflight (default dei wallet): simulazione sul thread del nodo.
    let sample: Vec<String> = (0..2_000u64).map(|i| enc(&transfer_tx(&senders[(i as usize) % SENDERS], &addr(i + 1), 2_000_000 + i, bh))).collect();
    let t = Instant::now();
    std::thread::scope(|s| {
        for chunk in sample.chunks(sample.len().div_ceil(CLIENTS)) {
            s.spawn(move || {
                for r in chunk {
                    call(url, "sendTransaction", json!([r, {"encoding": "base64"}])).unwrap();
                }
            });
        }
    });
    let preflight = sample.len() as f64 / t.elapsed().as_secs_f64();
    (sent_s, incl_s, count, preflight)
}
