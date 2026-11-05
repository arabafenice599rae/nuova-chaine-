//! Confronto dei moduli di archiviazione dello storico, sugli stessi record.
//!   cargo run --release --features redb-archive --example archive_bench -- [tx] [tx_per_block]
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
        "pages" => Box::new(centrifuga::archive::pages::Pages::open(dir).unwrap()),
        #[cfg(feature = "redb-archive")]
        "redb" => Box::new(centrifuga::archive::redb::Redb::open(dir).unwrap()),
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
    let mut kinds = vec!["segments", "pages"];
    if cfg!(feature = "redb-archive") {
        kinds.push("redb");
    }
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
