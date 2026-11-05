//! Wallet contro il nodo devnet installato dietro Caddy (workflow di deploy).
//! Ignorato in `cargo test`: lo esegue `deploy-test.yml` con
//! `cargo test --test wallet_caddy -- --ignored` e queste variabili:
//!   CENTRIFUGA_RPC           https://<dominio>
//!   CENTRIFUGA_CA            root della CA locale di Caddy (solo test, caricata qui)
//!   CENTRIFUGA_KEY           chiave prefinanziata (JSON solana-keygen)
//!   CENTRIFUGA_CHAIN_ID      dal genesis locale, non dal nodo
//!   CENTRIFUGA_GENESIS_HASH  dal genesis locale (`centrifuga genesis`), non dal nodo

use std::path::PathBuf;
use std::process::Command;
use std::time::Duration;

use centrifuga::rpc::client::{call_with, tls_with};
use rustls::pki_types::CertificateDer;
use rustls::pki_types::pem::PemObject;
use serde_json::json;
use solana_address::Address;

fn env(k: &str) -> String {
    std::env::var(k).unwrap_or_else(|_| panic!("manca {k}"))
}

#[test]
#[ignore]
fn wallet_against_caddy() {
    let (rpc, key) = (env("CENTRIFUGA_RPC"), env("CENTRIFUGA_KEY"));
    let mut roots = rustls::RootCertStore::empty();
    roots.add(CertificateDer::from_pem_file(env("CENTRIFUGA_CA")).unwrap()).unwrap();
    let tls = tls_with(roots);
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("wallet_caddy");
    std::fs::create_dir_all(&dir).unwrap();
    let toml = dir.join("wallet.toml");
    let conf = |hash: &str| {
        let s = format!("rpc = \"{rpc}\"\nkeypair = \"{key}\"\nchain_id = \"{}\"\ngenesis_hash = \"{hash}\"\n", env("CENTRIFUGA_CHAIN_ID"));
        std::fs::write(&toml, s).unwrap();
    };
    let cfg = toml.to_str().unwrap().to_string();
    let wallet = |a: &[&str]| {
        let args: Vec<String> = a.iter().chain(&["--config", cfg.as_str()]).map(|s| s.to_string()).collect();
        centrifuga::wallet::run_with(&args, tls.clone())
    };

    conf(&env("CENTRIFUGA_GENESIS_HASH"));
    wallet(&["address"]).unwrap();
    wallet(&["balance"]).unwrap();
    let to = Address::new_unique().to_string();
    wallet(&["send", &to, "0.25", "--yes"]).unwrap();
    let got = call_with(&tls, &rpc, "getBalance", json!([to])).unwrap();
    assert_eq!(got["value"], 250_000_000u64);
    wallet(&["history"]).unwrap();
    println!("wallet via Caddy: saldo, invio e cronologia OK");

    // Il binario usa solo le CA pubbliche: la CA locale di Caddy non è accettata.
    let o = Command::new(env!("CARGO_BIN_EXE_centrifuga")).args(["wallet", "balance", "--config", &cfg]).output().unwrap();
    let err = String::from_utf8_lossy(&o.stderr);
    assert!(!o.status.success() && err.contains("UnknownIssuer"), "{err}");
    // Genesis hash diverso da quello fissato: nodo rifiutato anche via HTTPS.
    conf("11111111111111111111111111111111");
    assert!(wallet(&["send", &to, "1", "--yes"]).unwrap_err().contains("node rejected"));
    std::thread::sleep(Duration::from_millis(500));
    assert_eq!(call_with(&tls, &rpc, "getBalance", json!([to])).unwrap()["value"], 250_000_000u64);
    println!("wallet via Caddy: CA non pubblica e genesis errato rifiutati");
}
