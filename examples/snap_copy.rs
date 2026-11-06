//! Misura (solo misure) delle parti della pausa dello snapshot sul thread del nodo:
//! copia della mappa degli account in un Vec, per N account.
//! Uso: `snap_copy <account>`

use std::time::Instant;

use centrifuga::genesis::{self, Config};
use solana_account::{Account, AccountSharedData};
use solana_address::Address;

const EXAMPLE: &str = include_str!("../centrifuga.example.toml");

fn main() {
    let n: u64 = std::env::args().nth(1).and_then(|v| v.replace('_', "").parse().ok()).unwrap_or(1_000_000);
    let mut cfg: Config = toml::from_str(EXAMPLE).unwrap();
    cfg.genesis.operator = "11111111111111111111111111111112".into();
    cfg.genesis.network = "devnet".into();
    cfg.genesis.chain_id = "centrifuga:devnet".into();
    let mut chain = genesis::build(&cfg.genesis).unwrap().0;
    let rent = chain.svm.rent_exempt(0);
    for i in 0..n {
        let mut b = [0u8; 32];
        b[..8].copy_from_slice(&i.to_le_bytes());
        b[8] = 0xCA;
        chain.svm.set(Address::new_from_array(b), Account { lamports: rent, owner: solana_sdk_ids::system_program::id(), ..Account::default() });
    }
    let accounts = chain.svm.accounts();
    println!("{} account", accounts.len());
    for round in 0..3 {
        let t = Instant::now();
        let v: Vec<(Address, AccountSharedData)> = accounts.iter().map(|(k, a)| (*k, a.clone())).collect();
        let copy = t.elapsed();
        let t = Instant::now();
        let mut w: Vec<(Address, AccountSharedData)> = Vec::with_capacity(accounts.len());
        w.extend(accounts.iter().map(|(k, a)| (*k, a.clone())));
        let copy_cap = t.elapsed();
        let t = Instant::now();
        let refs: Vec<(&Address, &AccountSharedData)> = accounts.iter().collect();
        let refs_t = t.elapsed();
        let t = Instant::now();
        drop(v);
        drop(w);
        let free = t.elapsed();
        println!(
            "giro {round}: copia {:.1} ms (con capacità {:.1} ms), solo riferimenti {:.1} ms, rilascio delle due copie {:.1} ms; {} B per voce",
            copy.as_secs_f64() * 1e3,
            copy_cap.as_secs_f64() * 1e3,
            refs_t.as_secs_f64() * 1e3,
            free.as_secs_f64() * 1e3,
            std::mem::size_of::<(Address, AccountSharedData)>()
        );
        drop(refs);
    }
}
