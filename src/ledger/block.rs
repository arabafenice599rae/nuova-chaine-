//! Blocco: header firmato dall'operatore + transazioni nell'ordine eseguito.

use sha2::{Digest, Sha256};
use solana_address::Address;
use solana_hash::Hash;
use solana_keypair::Keypair;
use solana_signature::Signature;
use solana_signer::Signer;
use solana_transaction::versioned::VersionedTransaction;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Header {
    pub height: u64,
    pub prev_hash: Hash,
    pub timestamp_ms: u64,
    pub state_hash: Hash,
    pub signature: Signature,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Block {
    pub header: Header,
    pub txs: Vec<VersionedTransaction>,
}

const HEADER_LEN: usize = 8 + 32 + 8 + 32 + 64;

pub fn tx_bytes(tx: &VersionedTransaction) -> Vec<u8> {
    wincode::serialize(tx).expect("serialize tx")
}

impl Block {
    /// Hash del blocco = blockhash usato dalle transazioni.
    /// sha256(height || prev_hash || timestamp || state_hash || sha256(corpo)).
    /// La firma copre questo hash, quindi anche il corpo.
    pub fn hash(&self) -> Hash {
        let mut body = Sha256::new();
        for tx in &self.txs {
            let b = tx_bytes(tx);
            body.update((b.len() as u32).to_le_bytes());
            body.update(&b);
        }
        let h = &self.header;
        let mut d = Sha256::new();
        d.update(h.height.to_le_bytes());
        d.update(h.prev_hash.as_ref());
        d.update(h.timestamp_ms.to_le_bytes());
        d.update(h.state_hash.as_ref());
        d.update(body.finalize());
        Hash::new_from_array(d.finalize().into())
    }

    pub fn sign(&mut self, kp: &Keypair) {
        self.header.signature = kp.sign_message(self.hash().as_ref());
    }

    pub fn verify_signature(&self, operator: &Address) -> bool {
        self.header.signature.verify(operator.as_ref(), self.hash().as_ref())
    }

    pub fn encode(&self) -> Vec<u8> {
        let h = &self.header;
        let mut out = Vec::with_capacity(HEADER_LEN + 4 + self.txs.len() * 256);
        out.extend_from_slice(&h.height.to_le_bytes());
        out.extend_from_slice(h.prev_hash.as_ref());
        out.extend_from_slice(&h.timestamp_ms.to_le_bytes());
        out.extend_from_slice(h.state_hash.as_ref());
        out.extend_from_slice(h.signature.as_ref());
        out.extend_from_slice(&(self.txs.len() as u32).to_le_bytes());
        for tx in &self.txs {
            let b = tx_bytes(tx);
            out.extend_from_slice(&(b.len() as u32).to_le_bytes());
            out.extend_from_slice(&b);
        }
        out
    }

    pub fn decode(mut b: &[u8]) -> Result<Block, String> {
        let mut take = |n: usize| -> Result<&[u8], String> {
            if b.len() < n {
                return Err("blocco troncato".into());
            }
            let (x, rest) = b.split_at(n);
            b = rest;
            Ok(x)
        };
        let u64_ = |x: &[u8]| u64::from_le_bytes(x.try_into().unwrap());
        let hash = |x: &[u8]| Hash::new_from_array(x.try_into().unwrap());
        let height = u64_(take(8)?);
        let prev_hash = hash(take(32)?);
        let timestamp_ms = u64_(take(8)?);
        let state_hash = hash(take(32)?);
        let signature = Signature::from(<[u8; 64]>::try_from(take(64)?).unwrap());
        let n = u32::from_le_bytes(take(4)?.try_into().unwrap());
        let mut txs = Vec::with_capacity(n.min(65_536) as usize);
        for _ in 0..n {
            let len = u32::from_le_bytes(take(4)?.try_into().unwrap()) as usize;
            let tx = wincode::deserialize(take(len)?).map_err(|e| format!("tx: {e}"))?;
            txs.push(tx);
        }
        Ok(Block { header: Header { height, prev_hash, timestamp_ms, state_hash, signature }, txs })
    }
}
