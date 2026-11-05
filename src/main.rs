use std::path::{Path, PathBuf};

use centrifuga::genesis::{self, Config};
use centrifuga::node::{fmt_amount, read_keypair};

const USAGE: &str = "usage: centrifuga genesis|run [config.toml]
     centrifuga balance <pubkey> [rpc_url]          (default http://127.0.0.1:8899)
     centrifuga wallet <new|address|balance|send|token-send|history> ...
     centrifuga eligible <config.toml> <airdrop_list.txt> <output.txt>";

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let arg = |i: usize, default: &str| args.get(i).map_or(default.to_string(), String::clone);
    let res = match args.get(1).map(String::as_str) {
        Some("genesis") => cmd_genesis(Path::new(&arg(2, "centrifuga.toml"))),
        Some("run") => cmd_run(Path::new(&arg(2, "centrifuga.toml"))),
        Some("balance") if args.len() >= 3 => cmd_balance(&args[2], &arg(3, "http://127.0.0.1:8899")),
        Some("wallet") => centrifuga::wallet::main(&args[2..]),
        Some("eligible") if args.len() == 5 => cmd_eligible(&PathBuf::from(&args[2]), &args[3], &args[4]),
        _ => Err(USAGE.to_string()),
    };
    if let Err(e) = res {
        eprintln!("error: {e}");
        std::process::exit(1);
    }
}

fn cmd_genesis(path: &Path) -> Result<(), String> {
    let cfg = Config::load(path)?;
    let sym = &cfg.genesis.symbol;
    let (chain, s) = genesis::build(&cfg.genesis)?;
    println!("{} ({sym}, {}), supply {}", cfg.genesis.chain_name, cfg.genesis.chain_id, fmt_amount(genesis::SUPPLY_LAMPORTS, sym));
    println!("system accounts: {}", fmt_amount(s.system_lamports, sym));
    for name in genesis::VAULT_NAMES {
        let k = genesis::vault_address(name);
        println!("vault {name:<8} {k} {}", fmt_amount(chain.svm.lamports(&k), sym));
    }
    println!("net reserve: {}", fmt_amount(s.reserve_lamports, sym));
    println!("conservation: OK (sum = {})", fmt_amount(chain.total_lamports() as u64, sym));
    let h = genesis::check_determinism(&cfg.genesis, 20)?;
    println!("determinism: OK (20 test blocks, final hash {h})");
    println!("genesis hash: {}", chain.genesis_hash);
    Ok(())
}

/// Saldo via JSON-RPC; il simbolo arriva da getVersion del nodo interrogato.
fn cmd_balance(pubkey: &str, url: &str) -> Result<(), String> {
    genesis::parse_address(pubkey)?;
    let version = centrifuga::rpc::client::call(url, "getVersion", serde_json::json!([]))?;
    let symbol = version["token"]["symbol"].as_str().unwrap_or("CnFA").to_string();
    let r = centrifuga::rpc::client::call(url, "getBalance", serde_json::json!([pubkey]))?;
    let lamports = r["value"].as_u64().ok_or("invalid getBalance response")?;
    println!("{}", fmt_amount(lamports, &symbol));
    Ok(())
}

/// Aventi diritto al residuo del fee mining, rieseguendo il journal da zero.
fn cmd_eligible(path: &Path, list: &str, out: &str) -> Result<(), String> {
    use centrifuga::claims;
    let cfg = Config::load(path)?;
    let text = std::fs::read_to_string(list).map_err(|e| format!("{list}: {e}"))?;
    let airdrop = claims::parse_list(&text)?;
    let e = claims::compute(&cfg.genesis, &cfg.node.data_dir.join("journal.bin"), &airdrop)?;
    let (file, hash) = e.render();
    std::fs::write(out, &file).map_err(|e| format!("{out}: {e}"))?;
    let sym = &cfg.genesis.symbol;
    println!("fee mining ends at block {}, residual {}", e.end_height, fmt_amount(e.residual, sym));
    println!("eligible: {} of {} listed, share {}", e.eligible.len(), airdrop.len(), fmt_amount(e.quota, sym));
    println!("unassigned (to reserve): {}", fmt_amount(e.unassigned, sym));
    println!("{out} sha256 {hash}");
    Ok(())
}

fn cmd_run(path: &Path) -> Result<(), String> {
    let cfg = Config::load(path)?;
    let kp = read_keypair(&cfg.node.operator_keypair)?;
    let running = centrifuga::node::start(cfg, kp)?;
    let rt = tokio::runtime::Runtime::new().map_err(|e| e.to_string())?;
    rt.block_on(running.serve_network())
}
