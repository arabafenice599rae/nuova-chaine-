use std::path::PathBuf;
use std::sync::{Arc, mpsc};

use centrifuga::genesis::{self, Config};
use centrifuga::node::{Node, fmt_amount, read_keypair};
use centrifuga::rpc::{self, BlockEvent, Gateway};

const USAGE: &str = "uso: centrifuga genesis|run [config.toml]\n     centrifuga eligible <config.toml> <lista_airdrop.txt> <output.txt>";

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let cfg_path = PathBuf::from(args.get(2).map_or("centrifuga.toml", String::as_str));
    let res = match args.get(1).map(String::as_str) {
        Some("genesis") => cmd_genesis(&cfg_path),
        Some("run") => cmd_run(&cfg_path),
        Some("eligible") if args.len() == 5 => cmd_eligible(&cfg_path, &args[3], &args[4]),
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
    for name in genesis::VAULT_NAMES {
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

/// Aventi diritto al residuo del fee mining, rieseguendo il journal da zero.
fn cmd_eligible(path: &PathBuf, list: &str, out: &str) -> Result<(), String> {
    use centrifuga::claims;
    let cfg = Config::load(path)?;
    let text = std::fs::read_to_string(list).map_err(|e| format!("{list}: {e}"))?;
    let airdrop = claims::parse_list(&text)?;
    let e = claims::compute(&cfg.genesis, &cfg.node.data_dir.join("journal.bin"), &airdrop)?;
    let (file, hash) = e.render();
    std::fs::write(out, &file).map_err(|e| format!("{out}: {e}"))?;
    let sym = &cfg.node.token_symbol;
    println!("fine fee mining al blocco {}, residuo {}", e.end_height, fmt_amount(e.residual, sym));
    println!("aventi diritto: {} su {} in lista, quota {}", e.eligible.len(), airdrop.len(), fmt_amount(e.quota, sym));
    println!("non assegnato (a reserve): {}", fmt_amount(e.unassigned, sym));
    println!("{out} sha256 {hash}");
    Ok(())
}

fn cmd_run(path: &PathBuf) -> Result<(), String> {
    let cfg = Config::load(path)?;
    let kp = read_keypair(&cfg.node.operator_keypair)?;
    let addr = cfg.node.rpc_addr.parse().map_err(|e| format!("rpc_addr: {e}"))?;
    let (qtx, qrx) = mpsc::channel();
    let (etx, _) = tokio::sync::broadcast::channel(1024);
    let gw = Gateway { queries: qtx, events: etx.clone(), trace: cfg.node.rpc_trace };
    let (ready_tx, ready_rx) = mpsc::channel();
    // Il nodo (litesvm) vive su un thread dedicato; l'RPC gli manda le query.
    let node_thread = std::thread::spawn(move || {
        let sym = cfg.node.token_symbol.clone();
        let node = match Node::open(cfg, kp) {
            Ok(n) => n,
            Err(e) => return ready_tx.send(Err(e)).unwrap(),
        };
        println!("{}: altezza {}, hash {}", node.cfg.node.token_name, node.chain.height, node.chain.last_hash);
        ready_tx.send(Ok(())).unwrap();
        let res = node.serve(qrx, |_, ex| {
            let _ = etx.send(Arc::new(BlockEvent::new(ex)));
            if !ex.block.txs.is_empty() {
                let s = &ex.settlement;
                println!(
                    "blocco {}: {} tx, fee base {}, operatore {}, riserva {}, premio {}, rimborso priority {}",
                    ex.block.header.height, ex.block.txs.len(), fmt_amount(s.base, &sym), fmt_amount(s.operator, &sym),
                    fmt_amount(s.reserve, &sym), fmt_amount(s.reward, &sym), fmt_amount(s.priority_refund, &sym),
                );
            }
        });
        if let Err(e) = res {
            eprintln!("errore nodo: {e}");
            std::process::exit(1);
        }
    });
    ready_rx.recv().map_err(|e| e.to_string())??;
    let rt = tokio::runtime::Runtime::new().map_err(|e| e.to_string())?;
    rt.block_on(rpc::serve(gw, addr)).map_err(|e| format!("rpc: {e}"))?;
    node_thread.join().ok();
    Ok(())
}
