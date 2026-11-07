//! Storico delle transazioni (fuori dal nodo): record, formato binario e interfaccia
//! dell'archivio dell'indexer. Archivio: file di segmenti append-only (`segments`),
//! scelto dopo il confronto con un archivio a pagine scritto a mano e con redb
//! (docs/history.md).

pub mod segments;

use std::io;

use solana_address::Address;
use solana_signature::Signature;
use solana_transaction::TransactionError;
use solana_transaction::versioned::VersionedTransaction;

use crate::exec::Executed;

/// Quello che serve a getTransaction, in RAM e su disco.
#[derive(Clone)]
pub struct TxRecord {
    pub slot: u64,
    pub time_ms: u64,
    pub tx: VersionedTransaction,
    pub err: Option<TransactionError>,
    pub fee: u64,
    pub logs: Vec<String>,
    pub compute_units: u64,
    pub return_data: Option<(Address, Vec<u8>)>,
    pub pre: Vec<u64>,
    pub post: Vec<u64>,
    /// Tempo nel nodo: arrivo della richiesta HTTP -> blocco firmato e scritto (µs).
    pub node_us: Option<u64>,
}

impl TxRecord {
    pub fn err(&self) -> Option<&TransactionError> {
        self.err.as_ref()
    }
}

/// Riga di getSignaturesForAddress.
pub struct HistoryEntry {
    pub sig: Signature,
    pub slot: u64,
    pub time_ms: u64,
    pub err: Option<TransactionError>,
}

// ---- formato dei record ----

pub struct W(pub Vec<u8>);
impl W {
    pub fn u64(&mut self, v: u64) {
        self.0.extend_from_slice(&v.to_le_bytes());
    }
    pub fn bytes(&mut self, b: &[u8]) {
        self.0.extend_from_slice(&(b.len() as u32).to_le_bytes());
        self.0.extend_from_slice(b);
    }
}

pub struct R<'a>(pub &'a [u8]);
impl<'a> R<'a> {
    pub fn take(&mut self, n: usize) -> io::Result<&'a [u8]> {
        if self.0.len() < n {
            return Err(io::Error::other("record troncato"));
        }
        let (a, b) = self.0.split_at(n);
        self.0 = b;
        Ok(a)
    }
    pub fn u64(&mut self) -> io::Result<u64> {
        Ok(u64::from_le_bytes(self.take(8)?.try_into().unwrap()))
    }
    pub fn u32(&mut self) -> io::Result<u32> {
        Ok(u32::from_le_bytes(self.take(4)?.try_into().unwrap()))
    }
    pub fn bytes(&mut self) -> io::Result<&'a [u8]> {
        let n = self.u32()? as usize;
        self.take(n)
    }
}

pub fn bad<E: std::fmt::Display>(e: E) -> io::Error {
    io::Error::other(e.to_string())
}

/// Record: firma, campi di getTransaction, poi (indirizzo, record precedente) per ogni
/// account statico della tx.
pub fn encode(r: &TxRecord, links: &[(Address, u64)]) -> Vec<u8> {
    let mut w = W(Vec::with_capacity(512));
    w.0.extend_from_slice(r.tx.signatures[0].as_ref());
    for v in [r.slot, r.time_ms, r.node_us.unwrap_or(u64::MAX), r.fee, r.compute_units] {
        w.u64(v);
    }
    w.bytes(&r.err.as_ref().map(|e| serde_json::to_vec(e).unwrap()).unwrap_or_default());
    w.u64(r.logs.len() as u64);
    for l in &r.logs {
        w.bytes(l.as_bytes());
    }
    match &r.return_data {
        Some((p, d)) => {
            w.0.extend_from_slice(p.as_ref());
            w.bytes(d);
        }
        None => w.0.extend_from_slice(&[0u8; 32 + 4]),
    }
    w.u64(r.pre.len() as u64);
    r.pre.iter().chain(&r.post).for_each(|v| w.u64(*v));
    w.u64(links.len() as u64);
    for (k, prev) in links {
        w.0.extend_from_slice(k.as_ref());
        w.u64(*prev);
    }
    w.bytes(&wincode::serialize(&r.tx).unwrap());
    w.0
}

pub fn decode(b: &[u8]) -> io::Result<(TxRecord, Vec<(Address, u64)>)> {
    let mut r = R(b);
    r.take(64)?;
    let (slot, time_ms, node_us, fee, compute_units) = (r.u64()?, r.u64()?, r.u64()?, r.u64()?, r.u64()?);
    let e = r.bytes()?;
    let err = (!e.is_empty()).then(|| serde_json::from_slice(e)).transpose().map_err(bad)?;
    let logs = (0..r.u64()?).map(|_| r.bytes().map(|l| String::from_utf8_lossy(l).into_owned())).collect::<io::Result<_>>()?;
    let program = Address::new_from_array(r.take(32)?.try_into().unwrap());
    let data = r.bytes()?.to_vec();
    let return_data = (!data.is_empty()).then_some((program, data));
    let n = r.u64()? as usize;
    let pre = (0..n).map(|_| r.u64()).collect::<io::Result<_>>()?;
    let post = (0..n).map(|_| r.u64()).collect::<io::Result<_>>()?;
    let links = (0..r.u64()?).map(|_| Ok((Address::new_from_array(r.take(32)?.try_into().unwrap()), r.u64()?))).collect::<io::Result<_>>()?;
    let tx = wincode::deserialize(r.bytes()?).map_err(bad)?;
    let node_us = (node_us != u64::MAX).then_some(node_us);
    Ok((TxRecord { slot, time_ms, tx, err, fee, logs, compute_units, return_data, pre, post, node_us }, links))
}

/// Record di un blocco eseguito, con i collegamenti per indirizzo ancora da riempire.
pub fn records_of(ex: &Executed, node_us: impl Fn(&Signature) -> Option<u64>) -> Vec<TxRecord> {
    let h = &ex.block.header;
    ex.block
        .txs
        .iter()
        .zip(&ex.results)
        .zip(&ex.balances)
        .map(|((tx, result), (pre, post))| {
            let meta = match result {
                Ok(m) => m,
                Err(f) => &f.meta,
            };
            let rd = &meta.return_data;
            TxRecord {
                slot: h.height,
                time_ms: h.timestamp_ms,
                tx: tx.clone(),
                err: result.as_ref().err().map(|f| f.err.clone()),
                fee: meta.fee,
                logs: meta.logs.clone(),
                compute_units: meta.compute_units_consumed,
                return_data: (!rd.data.is_empty()).then(|| (rd.program_id, rd.data.clone())),
                pre: pre.clone(),
                post: post.clone(),
                node_us: node_us(&tx.signatures[0]),
            }
        })
        .collect()
}

/// Account statici distinti di una tx (chiavi della cronologia per indirizzo).
pub fn keys(r: &TxRecord) -> Vec<Address> {
    let mut k: Vec<Address> = r.tx.message.static_account_keys().to_vec();
    k.dedup();
    k
}

/// Modulo di archiviazione dello storico.
pub trait Archive: Send {
    /// Aggiunge i record di un blocco (una volta per altezza, in ordine).
    fn append(&mut self, height: u64, recs: Vec<TxRecord>) -> io::Result<()>;
    fn get(&self, sig: &Signature) -> Option<TxRecord>;
    /// Tx che toccano `addr`, dalla più recente.
    fn history(&self, addr: &Address, before: Option<Signature>, until: Option<Signature>, limit: usize) -> Vec<HistoryEntry>;
    /// Dati su disco in modo durevole; restituisce l'altezza garantita dopo un riavvio.
    fn checkpoint(&mut self) -> io::Result<u64>;
    /// Ultima altezza registrata.
    fn height(&self) -> u64;
    /// Conservazione: elimina i blocchi sotto `height` (se il modulo lo permette).
    fn prune(&mut self, height: u64) -> io::Result<()>;
    /// Byte occupati su disco.
    fn disk_bytes(&self) -> u64;
}
