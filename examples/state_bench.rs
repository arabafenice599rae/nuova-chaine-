//! Confronto tra hash di stato: LtHash (Agave) e albero di Merkle incrementale con
//! prove per singolo account. Stessi account cambiati per blocco del test di carico
//! (5.000 trasferimenti: 2.000 mittenti fissi più destinatari casuali tra gli N).
//!   cargo run --release --example state_bench -- <accounts> [blocks]
//! Prototipo SOLO per la misura: il Merkle non è nel nodo.

use std::collections::HashMap;
use std::time::Instant;

use centrifuga::exec::state_hash::{LtState, account_lt_hash};
use sha2::{Digest, Sha256};
use solana_account::{AccountSharedData, ReadableAccount, WritableAccount};
use solana_address::Address;

fn rss_mb() -> u64 {
    let s = std::fs::read_to_string("/proc/self/status").unwrap_or_default();
    s.lines().find(|l| l.starts_with("VmRSS:")).and_then(|l| l.split_whitespace().nth(1)?.parse::<u64>().ok()).unwrap_or(0) / 1024
}

fn addr(i: u64) -> Address {
    let mut b = [0u8; 32];
    b[..8].copy_from_slice(&i.to_le_bytes());
    b[8] = 0xCA;
    Address::new_from_array(b)
}

type H = [u8; 32];

fn leaf_hash(k: &Address, a: &AccountSharedData) -> H {
    let mut h = Sha256::new();
    h.update([0u8]);
    h.update(k.as_ref());
    h.update(a.lamports().to_le_bytes());
    h.update(a.owner().as_ref());
    h.update([a.executable() as u8]);
    h.update((a.data().len() as u64).to_le_bytes());
    h.update(a.data());
    h.finalize().into()
}

fn node_hash(l: &H, r: &H) -> H {
    let mut h = Sha256::new();
    h.update([1u8]);
    h.update(l);
    h.update(r);
    h.finalize().into()
}

/// Albero binario denso: ogni account ha un indice di foglia fisso (ordine di
/// creazione, deterministico); livelli completi fino alla potenza di 2.
struct Merkle {
    index: HashMap<Address, u32>,
    levels: Vec<Vec<H>>, // levels[0] = foglie
}

impl Merkle {
    fn build(accounts: &HashMap<Address, AccountSharedData>) -> Merkle {
        let mut keys: Vec<&Address> = accounts.keys().collect();
        keys.sort_unstable();
        let cap = keys.len().next_power_of_two();
        let mut leaves = vec![[0u8; 32]; cap];
        let mut index = HashMap::with_capacity(keys.len());
        for (i, k) in keys.iter().enumerate() {
            leaves[i] = leaf_hash(k, &accounts[*k]);
            index.insert(**k, i as u32);
        }
        let mut levels = vec![leaves];
        while levels.last().unwrap().len() > 1 {
            let prev = levels.last().unwrap();
            levels.push(prev.chunks(2).map(|p| node_hash(&p[0], &p[1])).collect());
        }
        Merkle { index, levels }
    }

    fn root(&self) -> H {
        self.levels.last().unwrap()[0]
    }

    /// Aggiorna le foglie cambiate e ricalcola i soli percorsi interessati.
    fn update(&mut self, changed: &[(Address, &AccountSharedData)]) {
        let mut idx: Vec<usize> = changed
            .iter()
            .map(|(k, a)| {
                let i = self.index[k] as usize;
                self.levels[0][i] = leaf_hash(k, a);
                i
            })
            .collect();
        for l in 1..self.levels.len() {
            idx.iter_mut().for_each(|i| *i /= 2);
            idx.sort_unstable();
            idx.dedup();
            for &i in &idx {
                let (a, b) = (self.levels[l - 1][2 * i], self.levels[l - 1][2 * i + 1]);
                self.levels[l][i] = node_hash(&a, &b);
            }
        }
    }

    /// Prova: indice e fratelli lungo il percorso (32 byte per livello).
    fn proof(&self, k: &Address) -> (usize, Vec<H>) {
        let mut i = self.index[k] as usize;
        let mut sib = Vec::with_capacity(self.levels.len() - 1);
        for l in 0..self.levels.len() - 1 {
            sib.push(self.levels[l][i ^ 1]);
            i /= 2;
        }
        (self.index[k] as usize, sib)
    }
}

fn verify(root: &H, k: &Address, a: &AccountSharedData, index: usize, sib: &[H]) -> bool {
    let (mut h, mut i) = (leaf_hash(k, a), index);
    for s in sib {
        h = if i % 2 == 0 { node_hash(&h, s) } else { node_hash(s, &h) };
        i /= 2;
    }
    &h == root
}

fn main() {
    let args: Vec<u64> = std::env::args().skip(1).map(|a| a.replace('_', "").parse().expect("number")).collect();
    let (n, blocks) = (args[0], *args.get(1).unwrap_or(&20));
    let system = solana_sdk_ids::system_program::id();
    let mut accounts: HashMap<Address, AccountSharedData> = HashMap::with_capacity(n as usize + 2_000);
    for i in 0..n + 2_000 {
        accounts.insert(addr(i), AccountSharedData::new(890_880 + i % 1000, 0, &system));
    }
    let base = rss_mb();

    let t = Instant::now();
    let mut lt = LtState::full(&accounts);
    let lt_build = t.elapsed().as_secs_f64();
    let lt_ram = rss_mb().saturating_sub(base);
    let r0 = rss_mb();
    let t = Instant::now();
    let mut mk = Merkle::build(&accounts);
    let mk_build = t.elapsed().as_secs_f64();
    let mk_ram = rss_mb().saturating_sub(r0);

    // Stessi blocchi del test di carico: 5.000 tx, mittente a rotazione tra 2.000, destinatario casuale.
    let mut seed = 0x9E37_79B9_7F4A_7C15u64;
    let mut rnd = || {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        seed
    };
    let (mut lt_t, mut mk_t, mut changed_total) = (0f64, 0f64, 0usize);
    for _ in 0..blocks {
        let mut ch: Vec<Address> = (0..2_000).map(|s| addr(n + s)).collect();
        ch.extend((0..5_000).map(|_| addr(rnd() % n)));
        ch.sort_unstable();
        ch.dedup();
        changed_total += ch.len();
        let before: HashMap<Address, Option<AccountSharedData>> = ch.iter().map(|k| (*k, accounts.get(k).cloned())).collect();
        for k in &ch {
            let a = accounts.get_mut(k).unwrap();
            a.set_lamports(a.lamports() + 1);
        }
        let t = Instant::now();
        lt.update(&before, &accounts);
        let _ = lt.hash();
        lt_t += t.elapsed().as_secs_f64();
        let list: Vec<(Address, &AccountSharedData)> = ch.iter().map(|k| (*k, &accounts[k])).collect();
        let t = Instant::now();
        mk.update(&list);
        let _ = mk.root();
        mk_t += t.elapsed().as_secs_f64();
    }
    assert_eq!(lt.hash(), LtState::full(&accounts).hash(), "LtHash incrementale");
    assert_eq!(mk.root(), Merkle::build(&accounts).root(), "Merkle incrementale");

    // Prove: 10.000 account casuali.
    let root = mk.root();
    let ks: Vec<Address> = (0..10_000).map(|_| addr(rnd() % n)).collect();
    let proofs: Vec<_> = ks.iter().map(|k| mk.proof(k)).collect();
    let t = Instant::now();
    for (k, (i, sib)) in ks.iter().zip(&proofs) {
        assert!(verify(&root, k, &accounts[k], *i, sib));
    }
    let verify_us = t.elapsed().as_secs_f64() * 1e6 / ks.len() as f64;
    let proof_bytes = 8 + 32 * proofs[0].1.len();
    let _ = account_lt_hash; // stesso hash per account del nodo

    println!(
        "| {n} | {:.0} | {:.1} | {:.1} | {lt_ram} | {mk_ram} | {lt_build:.1} | {mk_build:.1} | {proof_bytes} + account | {verify_us:.1} |",
        changed_total as f64 / blocks as f64,
        lt_t / blocks as f64 * 1e3,
        mk_t / blocks as f64 * 1e3,
    );
}
