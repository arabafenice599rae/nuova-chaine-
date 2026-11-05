//! Hash dello stato: LtHash omomorfo come in Agave (`solana-lattice-hash`).
//! Lo stato è la somma (mod 2^16, 1024 elementi) degli LtHash di ogni account;
//! a ogni blocco si toglie il vecchio contributo degli account cambiati e si
//! aggiunge il nuovo, quindi il costo dipende dagli account cambiati, non dal
//! totale. L'hash nell'header è il checksum (blake3, 32 byte) dell'LtHash.

use std::collections::HashMap;

use solana_account::{AccountSharedData, ReadableAccount};
use solana_address::Address;
use solana_hash::Hash;
use solana_lattice_hash::lt_hash::LtHash;

/// LtHash di un account, con gli stessi campi e lo stesso ordine di Agave
/// (`rent_epoch` escluso). Un account a zero lamport non contribuisce.
pub fn account_lt_hash(k: &Address, a: &AccountSharedData) -> LtHash {
    if a.lamports() == 0 {
        return LtHash::identity();
    }
    let mut h = blake3::Hasher::new();
    h.update(&a.lamports().to_le_bytes());
    h.update(a.data());
    h.update(&[a.executable() as u8]);
    h.update(a.owner().as_ref());
    h.update(k.as_ref());
    LtHash::with(&h)
}

/// Stato corrente dell'accumulatore.
#[derive(Clone)]
pub struct LtState(LtHash);

impl LtState {
    /// Ricalcolo completo: O(account totali). All'avvio e come riferimento nei test.
    pub fn full(accounts: &HashMap<Address, AccountSharedData>) -> LtState {
        let mut lt = LtHash::identity();
        for (k, a) in accounts {
            lt.mix_in(&account_lt_hash(k, a));
        }
        LtState(lt)
    }

    /// Stato vuoto.
    pub fn empty() -> LtState {
        LtState(LtHash::identity())
    }

    /// Un account passa da `old` a `new` (`None` = non esiste).
    pub fn replace(&mut self, k: &Address, old: Option<&AccountSharedData>, new: Option<&AccountSharedData>) {
        if let Some(old) = old {
            self.0.mix_out(&account_lt_hash(k, old));
        }
        if let Some(new) = new {
            self.0.mix_in(&account_lt_hash(k, new));
        }
    }

    /// Aggiornamento incrementale: `before` ha il valore di ogni account cambiato
    /// prima del blocco (`None` = non esisteva). O(account cambiati).
    pub fn update(&mut self, before: &HashMap<Address, Option<AccountSharedData>>, accounts: &HashMap<Address, AccountSharedData>) {
        for (k, old) in before {
            self.replace(k, old.as_ref(), accounts.get(k));
        }
    }

    pub fn hash(&self) -> Hash {
        Hash::new_from_array(self.0.checksum().0)
    }
}
