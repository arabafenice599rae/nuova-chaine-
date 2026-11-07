//! Indice delle transazioni nel nodo: solo in RAM, a dimensione limitata.
//!   - stati (slot, errore) delle tx degli ultimi `blockhash_window` blocchi: servono a
//!     getSignatureStatuses e coprono la scadenza dei blockhash; si salvano accanto
//!     allo snapshot (status.bin, limitato alla finestra), così un riavvio non li perde;
//!   - record completi delle ultime `rpc_recent_txs` tx, per getTransaction.
//!
//! Lo storico completo non sta nel nodo: lo serve l'indexer (`centrifuga indexer`).

use std::collections::{HashMap, VecDeque};
use std::io;
use std::path::Path;

use solana_signature::Signature;
use solana_transaction::TransactionError;

pub use crate::archive::{HistoryEntry, TxRecord};
use crate::archive::{R, W, records_of};
use crate::exec::Executed;

pub struct TxIndex {
    statuses: HashMap<Signature, (u64, Option<TransactionError>)>,
    by_height: VecDeque<(u64, Vec<Signature>)>,
    blocks: u64,
    records: HashMap<Signature, TxRecord>,
    order: VecDeque<Signature>,
    max_records: usize,
    /// Tx eseguite da quando esiste la chain (getEpochInfo.transactionCount).
    pub count: u64,
}

impl TxIndex {
    /// `blocks`: finestra degli stati; `capacity`: tx attese in quella finestra.
    pub fn new(blocks: u64, max_records: usize, capacity: usize) -> TxIndex {
        TxIndex {
            statuses: HashMap::with_capacity(capacity),
            by_height: VecDeque::new(),
            blocks,
            records: HashMap::with_capacity(max_records.min(1 << 20)),
            order: VecDeque::new(),
            max_records,
            count: 0,
        }
    }

    pub fn record(&mut self, ex: &Executed, node_us: impl Fn(&Signature) -> Option<u64>) {
        let h = ex.block.header.height;
        let mut statuses = Vec::with_capacity(ex.block.txs.len());
        for rec in records_of(ex, node_us) {
            let s = rec.tx.signatures[0];
            statuses.push((s, rec.err.clone()));
            if self.max_records > 0 {
                self.order.push_back(s);
                self.records.insert(s, rec);
            }
        }
        self.record_statuses(h, statuses);
        while self.order.len() > self.max_records {
            let s = self.order.pop_front().unwrap();
            self.records.remove(&s);
        }
    }

    /// Solo gli stati di un blocco (firma, errore): la finestra di `status.bin`. Lo usa
    /// anche lo stato ombra dello snapshot, che ne tiene una copia propria.
    pub fn record_statuses(&mut self, h: u64, statuses: Vec<(Signature, Option<TransactionError>)>) {
        let mut sigs = Vec::with_capacity(statuses.len());
        for (s, err) in statuses {
            sigs.push(s);
            self.statuses.insert(s, (h, err));
        }
        self.count += sigs.len() as u64;
        self.by_height.push_back((h, sigs));
        while self.by_height.front().is_some_and(|f| f.0 + self.blocks < h) {
            for s in self.by_height.pop_front().unwrap().1 {
                self.statuses.remove(&s);
            }
        }
    }

    /// Stati di un blocco già eseguito (per lo stato ombra).
    pub fn statuses_of(ex: &Executed) -> Vec<(Signature, Option<TransactionError>)> {
        ex.block.txs.iter().zip(&ex.results).map(|(t, r)| (t.signatures[0], r.as_ref().err().map(|f| f.err.clone()))).collect()
    }

    pub fn status(&self, sig: &Signature) -> Option<&(u64, Option<TransactionError>)> {
        self.statuses.get(sig)
    }

    pub fn get(&self, sig: &Signature) -> Option<&TxRecord> {
        self.records.get(sig)
    }

    pub fn recent_len(&self) -> usize {
        self.records.len()
    }

    pub fn statuses_len(&self) -> usize {
        self.statuses.len()
    }

    /// Contenuto di `status.bin` per lo snapshot all'altezza `height`.
    pub fn encode(&self, height: u64) -> Vec<u8> {
        let mut w = W(Vec::new());
        w.u64(height);
        w.u64(self.count);
        w.u64(self.by_height.len() as u64);
        for (h, sigs) in &self.by_height {
            w.u64(*h);
            w.u64(sigs.len() as u64);
            for s in sigs {
                let (slot, err) = &self.statuses[s];
                w.0.extend_from_slice(s.as_ref());
                w.u64(*slot);
                w.bytes(&err.as_ref().map(|e| serde_json::to_vec(e).unwrap()).unwrap_or_default());
            }
        }
        w.0
    }

    /// Stati salvati con lo snapshot all'altezza `height` (i blocchi successivi arrivano
    /// dal journal). Un file di un altro snapshot si ignora.
    pub fn load(&mut self, dir: &Path, height: u64) -> io::Result<()> {
        let Ok(b) = std::fs::read(dir.join("status.bin")) else { return Ok(()) };
        self.decode(&b, height)
    }

    /// Stati da un contenuto di `status.bin` (ignorato se di un'altra altezza).
    pub fn decode(&mut self, b: &[u8], height: u64) -> io::Result<()> {
        let mut r = R(b);
        if r.u64()? != height {
            return Ok(());
        }
        self.count = r.u64()?;
        for _ in 0..r.u64()? {
            let h = r.u64()?;
            let mut sigs = vec![];
            for _ in 0..r.u64()? {
                let s = Signature::try_from(r.take(64)?).map_err(crate::archive::bad)?;
                let slot = r.u64()?;
                let e = r.bytes()?;
                let err = (!e.is_empty()).then(|| serde_json::from_slice(e)).transpose().map_err(crate::archive::bad)?;
                self.statuses.insert(s, (slot, err));
                sigs.push(s);
            }
            self.by_height.push_back((h, sigs));
        }
        Ok(())
    }
}
