//! Albero di Merkle sparso, SOLO per le misure: crate `sparse-merkle-tree` 0.6.1
//! (Nervos, usato da CKB/Godwoken), variante `trie` con percorsi compressi.
//! Posizione della foglia = SHA-256 della pubkey (256 bit, non dipende dalla storia);
//! valore = hash dei campi dell'account, zero = account assente (prova di assenza).
//! Hash dei nodi: SHA-256 tramite il trait `Hasher` del crate (adattatore, nessuna
//! crittografia nuova).

use std::collections::HashMap;

use sha2::{Digest, Sha256};
use solana_account::{AccountSharedData, ReadableAccount};
use solana_address::Address;
use sparse_merkle_tree::default_store::DefaultStore;
use sparse_merkle_tree::traits::Hasher;
use sparse_merkle_tree::{CompiledMerkleProof, H256, SparseMerkleTree};

#[derive(Default)]
pub struct Sha(Sha256);

impl Hasher for Sha {
    fn write_h256(&mut self, h: &H256) {
        self.0.update(h.as_slice());
    }
    fn write_byte(&mut self, b: u8) {
        self.0.update([b]);
    }
    fn finish(self) -> H256 {
        let a: [u8; 32] = self.0.finalize().into();
        a.into()
    }
}

type Tree = SparseMerkleTree<Sha, H256, DefaultStore<H256>>;

pub fn key(k: &Address) -> H256 {
    let a: [u8; 32] = Sha256::digest(k.as_ref()).into();
    a.into()
}

pub fn value(k: &Address, a: Option<&AccountSharedData>) -> H256 {
    match a {
        Some(a) if a.lamports() > 0 => super::merkle::leaf_hash(k, a).into(),
        _ => H256::zero(),
    }
}

pub struct Sparse(Tree);

impl Sparse {
    pub fn new() -> Sparse {
        Sparse(Tree::default())
    }

    pub fn build(accounts: &HashMap<Address, AccountSharedData>) -> Sparse {
        let mut t = Sparse::new();
        let leaves: Vec<(H256, H256)> = accounts.iter().map(|(k, a)| (key(k), value(k, Some(a)))).filter(|(_, v)| !v.is_zero()).collect();
        for chunk in leaves.chunks(100_000) {
            t.0.update_all(chunk.to_vec()).unwrap();
        }
        t
    }

    /// Account cambiati (None o zero lamport = rimosso).
    pub fn update(&mut self, changed: &[(Address, Option<&AccountSharedData>)]) {
        let leaves = changed.iter().map(|(k, a)| (key(k), value(k, *a))).collect();
        self.0.update_all(leaves).unwrap();
    }

    pub fn root(&self) -> [u8; 32] {
        let mut r = [0u8; 32];
        r.copy_from_slice(self.0.root().as_slice());
        r
    }

    /// Prova compilata per un account (presente o assente).
    pub fn proof(&self, k: &Address) -> Vec<u8> {
        self.0.merkle_proof(vec![key(k)]).unwrap().compile(vec![key(k)]).unwrap().0
    }
}

/// Verifica: con `a` = None si prova l'assenza dell'account.
pub fn verify(root: &[u8; 32], k: &Address, a: Option<&AccountSharedData>, proof: &[u8]) -> bool {
    let root: H256 = (*root).into();
    CompiledMerkleProof(proof.to_vec()).verify::<Sha>(&root, vec![(key(k), value(k, a))]).unwrap_or(false)
}
