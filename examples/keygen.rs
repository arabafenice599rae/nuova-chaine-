//! Chiavi nel formato solana-keygen (array JSON di 64 byte).
//!   keygen > chiave.json      genera: la chiave va su stdout, la pubkey su stderr
//!   keygen chiave.json        stampa solo la pubkey di una chiave esistente
use solana_signer::Signer;
fn main() {
    if let Some(path) = std::env::args().nth(1) {
        match centrifuga::node::read_keypair(std::path::Path::new(&path)) {
            Ok(kp) => println!("{}", kp.pubkey()),
            Err(e) => {
                eprintln!("errore: {e}");
                std::process::exit(1);
            }
        }
        return;
    }
    let kp = solana_keypair::Keypair::new();
    let b: Vec<String> = kp.to_bytes().iter().map(|x| x.to_string()).collect();
    println!("[{}]", b.join(","));
    eprintln!("pubkey: {}", kp.pubkey());
}
