//! Indice in memoria delle transazioni eseguite (per getTransaction,
//! getSignatureStatuses, getSignaturesForAddress). Si ricostruisce rieseguendo
//! il journal; le tx precedenti all'ultimo snapshot non sono indicizzate.

use std::collections::HashMap;

use solana_address::Address;
use solana_signature::Signature;
use solana_transaction::TransactionError;
use solana_transaction::versioned::VersionedTransaction;

use crate::exec::{Executed, TransactionResult};

pub struct TxRecord {
    pub slot: u64,
    pub time_ms: u64,
    pub tx: VersionedTransaction,
    pub result: TransactionResult,
    pub pre: Vec<u64>,
    pub post: Vec<u64>,
    /// Tempo nel nodo: arrivo della richiesta HTTP -> blocco firmato e scritto (µs).
    pub node_us: Option<u64>,
}

impl TxRecord {
    pub fn err(&self) -> Option<&TransactionError> {
        self.result.as_ref().err().map(|f| &f.err)
    }
}

#[derive(Default)]
pub struct TxIndex {
    pub by_sig: HashMap<Signature, TxRecord>,
    /// Firme per indirizzo, dalla più vecchia alla più recente.
    pub by_addr: HashMap<Address, Vec<Signature>>,
}

impl TxIndex {
    pub fn record(&mut self, ex: &Executed) {
        let h = &ex.block.header;
        for ((tx, result), (pre, post)) in ex.block.txs.iter().zip(&ex.results).zip(&ex.balances) {
            let sig = tx.signatures[0];
            for k in tx.message.static_account_keys() {
                let v = self.by_addr.entry(*k).or_default();
                if v.last() != Some(&sig) {
                    v.push(sig);
                }
            }
            let rec = TxRecord {
                slot: h.height,
                time_ms: h.timestamp_ms,
                tx: tx.clone(),
                result: result.clone(),
                pre: pre.clone(),
                post: post.clone(),
                node_us: None,
            };
            self.by_sig.insert(sig, rec);
        }
    }
}
