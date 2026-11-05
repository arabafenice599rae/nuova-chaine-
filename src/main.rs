use std::path::PathBuf;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use centrifuga::genesis::{self, Config};
use centrifuga::node::{Node, fmt_amount, read_keypair};

const USAGE: &str = "uso: centrifuga <genesis|run> [config.toml]";

fn now_ms() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_millis() as u64
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let cfg_path = PathBuf::from(args.get(2).map_or("centrifuga.toml", String::as_str));
    let res = match args.get(1).map(String::as_str) {
        Some("genesis") => cmd_genesis(&cfg_path),
        Some("run") => cmd_run(&cfg_path),
        _ => Err(USAGE.to_string()),
    };
    if let Err(e) = res {
        eprintln!("errore: {e}");
        std::process::exit(1);
    }
}

fn cmd_genesis(path: &PathBuf) -> Result<(), String> {
    let cfg = Config::load(path)?;
    let sym = &cfg.node.token_symbol;
    let (chain, s) = genesis::build(&cfg.genesis)?;
    println!("{} ({sym}), supply {}", cfg.node.token_name, fmt_amount(genesis::SUPPLY_LAMPORTS, sym));
    println!("account di sistema: {}", fmt_amount(s.system_lamports, sym));
    for name in ["users", "operator", "reserve"] {
        let k = genesis::vault_address(name);
        println!("vault {name:<8} {k} {}", fmt_amount(chain.svm.lamports(&k), sym));
    }
    println!("riserva netta: {}", fmt_amount(s.reserve_lamports, sym));
    println!("conservazione: OK (somma = {})", fmt_amount(chain.total_lamports() as u64, sym));
    let h = genesis::check_determinism(&cfg.genesis, 20, cfg.node.slot_ms)?;
    println!("determinismo: OK (20 blocchi di test, hash finale {h})");
    println!("genesis hash: {}", chain.genesis_hash);
    Ok(())
}

fn cmd_run(path: &PathBuf) -> Result<(), String> {
    let cfg = Config::load(path)?;
    let kp = read_keypair(&cfg.node.operator_keypair)?;
    let slot = Duration::from_millis(cfg.node.slot_ms);
    let sym = cfg.node.token_symbol.clone();
    let mut node = Node::open(cfg, kp)?;
    println!("{}: altezza {}, hash {}", node.cfg.node.token_name, node.chain.height, node.chain.last_hash);
    let mut next = Instant::now();
    loop {
        next += slot;
        let ex = node.tick(now_ms())?;
        if !ex.block.txs.is_empty() {
            let fees: u64 = ex.results.iter().map(|r| r.as_ref().map_or_else(|e| e.meta.fee, |m| m.fee)).sum();
            println!("blocco {}: {} tx, fee {}", ex.block.header.height, ex.block.txs.len(), fmt_amount(fees, &sym));
        }
        std::thread::sleep(next.saturating_duration_since(Instant::now()));
    }
}
