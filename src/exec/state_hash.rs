//! Hash dello stato. Interfaccia `StateHasher`: oggi SHA-256 completo su tutti
//! gli account ordinati; un hasher incrementale userà `changed`.

use std::collections::{BTreeSet, HashMap};

use sha2::{Digest, Sha256};
use solana_account::{AccountSharedData, ReadableAccount};
use solana_address::Address;
use solana_hash::Hash;

pub trait StateHasher: Send {
    /// `changed` contiene (un sovrainsieme de)gli account modificati nel blocco.
    fn hash(&mut self, accounts: &HashMap<Address, AccountSharedData>, changed: &BTreeSet<Address>) -> Hash;
}

/// SHA-256 su (pubkey, lamports, owner, executable, len, data) di ogni account,
/// in ordine di pubkey. `rent_epoch` escluso, come in Agave.
pub struct FullSha256;

impl StateHasher for FullSha256 {
    fn hash(&mut self, accounts: &HashMap<Address, AccountSharedData>, _: &BTreeSet<Address>) -> Hash {
        let mut keys: Vec<&Address> = accounts.keys().collect();
        keys.sort_unstable();
        let mut d = Sha256::new();
        for k in keys {
            let a = &accounts[k];
            d.update(k.as_ref());
            d.update(a.lamports().to_le_bytes());
            d.update(a.owner().as_ref());
            d.update([a.executable() as u8]);
            d.update((a.data().len() as u64).to_le_bytes());
            d.update(a.data());
        }
        Hash::new_from_array(d.finalize().into())
    }
}
