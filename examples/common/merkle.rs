//! Prototipo di albero di Merkle incrementale (SHA-256, foglie e nodi con prefissi
//! di dominio diversi), SOLO per le misure: non è nel nodo. Albero denso: ogni
//! account ha un indice di foglia fisso (ordine di pubkey alla costruzione, poi di
//! creazione); non dà prove di non appartenenza.

use std::collections::HashMap;

use sha2::{Digest, Sha256};
use solana_account::{AccountSharedData, ReadableAccount};
use solana_address::Address;

pub type H = [u8; 32];

pub fn leaf_hash(k: &Address, a: &AccountSharedData) -> H {
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
pub struct Merkle {
    pub index: HashMap<Address, u32>,
    pub levels: Vec<Vec<H>>, // levels[0] = foglie
}

impl Merkle {
    pub fn build(accounts: &HashMap<Address, AccountSharedData>) -> Merkle {
        let mut keys: Vec<&Address> = accounts.keys().collect();
        keys.sort_unstable();
        let cap = (keys.len() + keys.len() / 8 + 1).next_power_of_two(); // spazio per nuovi account
        let mut leaves = vec![[0u8; 32]; cap];
        let mut index = HashMap::with_capacity(keys.len());
        for (i, k) in keys.iter().enumerate() {
            let a = &accounts[*k];
            leaves[i] = if a.lamports() > 0 { leaf_hash(k, a) } else { [0u8; 32] };
            index.insert(**k, i as u32);
        }
        let mut levels = vec![leaves];
        while levels.last().unwrap().len() > 1 {
            let prev = levels.last().unwrap();
            levels.push(prev.chunks(2).map(|p| node_hash(&p[0], &p[1])).collect());
        }
        Merkle { index, levels }
    }

    /// Ricalcolo completo con gli stessi indici di foglia: deve dare la stessa radice.
    pub fn full_root(&self, accounts: &HashMap<Address, AccountSharedData>) -> H {
        let mut level = vec![[0u8; 32]; self.levels[0].len()];
        for (k, i) in &self.index {
            if let Some(a) = accounts.get(k).filter(|a| a.lamports() > 0) {
                level[*i as usize] = leaf_hash(k, a);
            }
        }
        while level.len() > 1 {
            level = level.chunks(2).map(|p| node_hash(&p[0], &p[1])).collect();
        }
        level[0]
    }

    pub fn root(&self) -> H {
        self.levels.last().unwrap()[0]
    }

    /// Aggiorna le foglie cambiate (nuovi account in coda, account chiusi = foglia
    /// vuota) e ricalcola i soli percorsi interessati.
    pub fn update(&mut self, changed: &[(Address, Option<&AccountSharedData>)]) {
        let mut idx = Vec::with_capacity(changed.len());
        for (k, a) in changed {
            let i = match self.index.get(k) {
                Some(i) => *i as usize,
                None => {
                    let i = self.index.len();
                    if i == self.levels[0].len() {
                        self.grow();
                    }
                    self.index.insert(*k, i as u32);
                    i
                }
            };
            self.levels[0][i] = match a {
                Some(a) if a.lamports() > 0 => leaf_hash(k, a),
                _ => [0u8; 32],
            };
            idx.push(i);
        }
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

    /// Albero pieno: raddoppia le foglie e ricalcola tutti i livelli (O(n), un picco).
    fn grow(&mut self) {
        let mut level = std::mem::take(&mut self.levels[0]);
        level.resize(level.len() * 2, [0u8; 32]);
        let mut levels = vec![level];
        while levels.last().unwrap().len() > 1 {
            let prev = levels.last().unwrap();
            levels.push(prev.chunks(2).map(|p| node_hash(&p[0], &p[1])).collect());
        }
        self.levels = levels;
    }

    /// Prova: indice e fratelli lungo il percorso (32 byte per livello).
    pub fn proof(&self, k: &Address) -> (usize, Vec<H>) {
        let mut i = self.index[k] as usize;
        let mut sib = Vec::with_capacity(self.levels.len() - 1);
        for l in 0..self.levels.len() - 1 {
            sib.push(self.levels[l][i ^ 1]);
            i /= 2;
        }
        (self.index[k] as usize, sib)
    }
}

pub fn verify(root: &H, k: &Address, a: &AccountSharedData, index: usize, sib: &[H]) -> bool {
    let (mut h, mut i) = (leaf_hash(k, a), index);
    for s in sib {
        h = if i % 2 == 0 { node_hash(&h, s) } else { node_hash(s, &h) };
        i /= 2;
    }
    &h == root
}
