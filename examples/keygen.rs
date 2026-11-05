//! Genera una chiave nel formato solana-keygen: `cargo run --example keygen > operator.json`.
use solana_signer::Signer;
fn main() {
    let kp = solana_keypair::Keypair::new();
    let b: Vec<String> = kp.to_bytes().iter().map(|x| x.to_string()).collect();
    println!("[{}]", b.join(","));
    eprintln!("pubkey: {}", kp.pubkey());
}
