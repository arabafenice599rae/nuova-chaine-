//! Indice delle transazioni eseguite (getTransaction, getSignatureStatuses,
//! getSignaturesForAddress). In RAM solo una finestra delle tx recenti; tutto lo
//! storico su disco, senza dipendenze:
//!   txlog.bin  record delle tx, aggiunti in coda per blocco (lunghezza + checksum);
//!   txsig.idx  firma -> record, hash lineare a pagine (cresce di una pagina alla
//!              volta: nessuna pausa di ridimensionamento);
//!   txheads    ultimo record per indirizzo (salvato a ogni snapshot); ogni record
//!              punta al record precedente di ciascun suo indirizzo.
//! Ogni lettura dal disco verifica la firma intera: un indice danneggiato non dà mai
//! un risultato sbagliato. Dopo un arresto del processo il registro si tronca
//! all'ultimo blocco completo e le voci dell'indice si reinseriscono (idempotente).
//! Dopo un'interruzione di corrente una pagina dell'indice scritta a metà può perdere
//! voci: si ricostruisce cancellando txsig.idx (rilettura di txlog.bin all'avvio).

use std::collections::{HashMap, VecDeque};
use std::fs::{File, OpenOptions};
use std::io::{self, Write};
use std::os::unix::fs::FileExt;
use std::path::Path;

use sha2::{Digest, Sha256};
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

struct W(Vec<u8>);
impl W {
    fn u64(&mut self, v: u64) {
        self.0.extend_from_slice(&v.to_le_bytes());
    }
    fn bytes(&mut self, b: &[u8]) {
        self.0.extend_from_slice(&(b.len() as u32).to_le_bytes());
        self.0.extend_from_slice(b);
    }
}

struct R<'a>(&'a [u8]);
impl<'a> R<'a> {
    fn take(&mut self, n: usize) -> io::Result<&'a [u8]> {
        if self.0.len() < n {
            return Err(io::Error::other("record troncato"));
        }
        let (a, b) = self.0.split_at(n);
        self.0 = b;
        Ok(a)
    }
    fn u64(&mut self) -> io::Result<u64> {
        Ok(u64::from_le_bytes(self.take(8)?.try_into().unwrap()))
    }
    fn u32(&mut self) -> io::Result<u32> {
        Ok(u32::from_le_bytes(self.take(4)?.try_into().unwrap()))
    }
    fn bytes(&mut self) -> io::Result<&'a [u8]> {
        let n = self.u32()? as usize;
        self.take(n)
    }
}

fn bad<E: std::fmt::Display>(e: E) -> io::Error {
    io::Error::other(e.to_string())
}

/// Record: firma, campi di getTransaction, poi (indirizzo, record precedente) per ogni
/// account statico della tx.
fn encode(r: &TxRecord, links: &[(Address, u64)]) -> Vec<u8> {
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

fn decode(b: &[u8]) -> io::Result<(TxRecord, Vec<(Address, u64)>)> {
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

// ---- indice firma -> record: hash lineare a pagine ----

const PAGE: usize = 4096;
const PER_PAGE: usize = (PAGE - 16) / 16; // voci (hash u64 della firma, offset u64)
const N0: u64 = 256;
/// txsig.idx: pagina 0 intestazione (livello, split, voci, pagine), poi pagine di
/// secchi primari e di overflow mescolate. txsig.tab: prima pagina di ogni secchio
/// (8 byte per secchio; si riscrivono solo le voci cambiate da uno split).
struct SigIndex {
    file: File,
    tab: File,
    level: u32,
    split: u64,
    entries: u64,
    pages: u64,
    table: Vec<u64>,
    dirty: Vec<usize>,
}

/// 64 bit della firma (casuale): sceglie il secchio e fa da impronta.
fn key(sig: &Signature) -> u64 {
    u64::from_le_bytes(sig.as_ref()[..8].try_into().unwrap())
}

impl SigIndex {
    fn open(dir: &Path) -> io::Result<(SigIndex, bool)> {
        let rw = |p: &str| OpenOptions::new().read(true).write(true).create(true).truncate(false).open(dir.join(p));
        let (file, tab) = (rw("txsig.idx")?, rw("txsig.tab")?);
        let mut hdr = vec![0u8; 48];
        let fresh = file.metadata()?.len() < PAGE as u64 || {
            file.read_exact_at(&mut hdr, 0)?;
            &hdr[..8] != b"TXSIG002"
        };
        if fresh {
            file.set_len(0)?;
            tab.set_len(0)?;
            let mut ix = SigIndex { file, tab, level: 0, split: 0, entries: 0, pages: 1, table: vec![], dirty: vec![] };
            for i in 0..N0 as usize {
                let p = ix.new_page()?;
                ix.table.push(p);
                ix.dirty.push(i);
            }
            ix.save()?;
            return Ok((ix, true));
        }
        let g = |i: usize| u64::from_le_bytes(hdr[8 + i * 8..16 + i * 8].try_into().unwrap());
        let (level, split, entries, pages) = (g(0) as u32, g(1), g(2), g(3));
        let n = ((N0 << level) + split) as usize;
        let mut raw = vec![0u8; n * 8];
        tab.read_exact_at(&mut raw, 0)?;
        let table = raw.chunks(8).map(|c| u64::from_le_bytes(c.try_into().unwrap())).collect();
        Ok((SigIndex { file, tab, level, split, entries, pages, table, dirty: vec![] }, false))
    }

    /// Voci della tabella cambiate, poi l'intestazione (dopo ogni blocco).
    fn save(&mut self) -> io::Result<()> {
        for i in std::mem::take(&mut self.dirty) {
            self.tab.write_all_at(&self.table[i].to_le_bytes(), i as u64 * 8)?;
        }
        let mut hdr = b"TXSIG002".to_vec();
        for v in [self.level as u64, self.split, self.entries, self.pages] {
            hdr.extend_from_slice(&v.to_le_bytes());
        }
        self.file.write_all_at(&hdr, 0)
    }

    fn new_page(&mut self) -> io::Result<u64> {
        let p = self.pages;
        self.pages += 1;
        self.file.write_all_at(&[0u8; PAGE], p * PAGE as u64)?;
        Ok(p)
    }

    fn bucket(&self, h: u64) -> usize {
        let b = h % (N0 << self.level);
        (if b < self.split { h % (N0 << (self.level + 1)) } else { b }) as usize
    }

    fn read(&self, p: u64) -> io::Result<(Vec<(u64, u64)>, u64)> {
        let mut b = vec![0u8; PAGE];
        self.file.read_exact_at(&mut b, p * PAGE as u64)?;
        let n = u64::from_le_bytes(b[..8].try_into().unwrap()) as usize;
        let next = u64::from_le_bytes(b[8..16].try_into().unwrap());
        let e = (0..n.min(PER_PAGE)).map(|i| {
            let o = 16 + i * 16;
            (u64::from_le_bytes(b[o..o + 8].try_into().unwrap()), u64::from_le_bytes(b[o + 8..o + 16].try_into().unwrap()))
        });
        Ok((e.collect(), next))
    }

    fn write(&self, p: u64, e: &[(u64, u64)], next: u64) -> io::Result<()> {
        let mut b = Vec::with_capacity(PAGE);
        b.extend_from_slice(&(e.len() as u64).to_le_bytes());
        b.extend_from_slice(&next.to_le_bytes());
        for (f, o) in e {
            b.extend_from_slice(&f.to_le_bytes());
            b.extend_from_slice(&o.to_le_bytes());
        }
        self.file.write_all_at(&b, p * PAGE as u64)
    }

    /// Offset dei record con questa impronta (il chiamante verifica la firma).
    fn candidates(&self, sig: &Signature) -> io::Result<Vec<u64>> {
        let h = key(sig);
        let (mut p, mut out) = (self.table[self.bucket(h)], vec![]);
        while p != 0 {
            let (e, next) = self.read(p)?;
            out.extend(e.iter().filter(|x| x.0 == h).map(|x| x.1));
            p = next;
        }
        Ok(out)
    }

    /// Inserisce (hash, offset) se non c'è già; poi al più uno split.
    fn insert(&mut self, sig: &Signature, offset: u64) -> io::Result<()> {
        let h = key(sig);
        let fp = h;
        let mut p = self.table[self.bucket(h)];
        loop {
            let (mut e, next) = self.read(p)?;
            if e.contains(&(fp, offset)) {
                return Ok(());
            }
            if next == 0 {
                if e.len() < PER_PAGE {
                    e.push((fp, offset));
                    self.write(p, &e, 0)?;
                } else {
                    let q = self.new_page()?;
                    self.write(q, &[(fp, offset)], 0)?;
                    self.write(p, &e, q)?;
                }
                break;
            }
            p = next;
        }
        self.entries += 1;
        if self.entries * 4 > self.table.len() as u64 * PER_PAGE as u64 * 3 {
            self.split_one()?;
        }
        Ok(())
    }

    /// Divide il secchio `split` tra sé e il nuovo secchio (le pagine si riusano).
    fn split_one(&mut self) -> io::Result<()> {
        let s = self.split as usize;
        let (mut pages, mut all, mut p) = (vec![], vec![], self.table[s]);
        while p != 0 {
            let (e, next) = self.read(p)?;
            pages.push(p);
            all.extend(e);
            p = next;
        }
        let m = N0 << (self.level + 1);
        let (lo, hi): (Vec<_>, Vec<_>) = all.into_iter().partition(|(h, _)| (h % m) as usize == s);
        // Copia su scrittura: le catene nuove vanno in pagine nuove e la tabella cambia solo
        // in save(); se il processo si ferma a metà, la catena vecchia resta valida.
        // Le pagine vecchie restano allocate (circa metà del file).
        let _ = pages;
        let mut free = std::iter::empty();
        let lo_head = self.chain(&lo, &mut free)?;
        let hi_head = self.chain(&hi, &mut free)?;
        self.table[s] = lo_head;
        self.table.push(hi_head);
        self.dirty.extend([s, self.table.len() - 1]);
        self.split += 1;
        if self.split == N0 << self.level {
            self.level += 1;
            self.split = 0;
        }
        Ok(())
    }

    fn chain(&mut self, e: &[(u64, u64)], free: &mut impl Iterator<Item = u64>) -> io::Result<u64> {
        let parts: Vec<&[(u64, u64)]> = if e.is_empty() { vec![&[]] } else { e.chunks(PER_PAGE).collect() };
        let mut ids = vec![];
        for _ in 0..parts.len() {
            ids.push(match free.next() {
                Some(p) => p,
                None => self.new_page()?,
            });
        }
        for (i, part) in parts.iter().enumerate() {
            self.write(ids[i], part, ids.get(i + 1).copied().unwrap_or(0))?;
        }
        Ok(ids[0])
    }
}

// ---- registro e indice ----

pub struct TxIndex {
    /// Finestra in RAM: le ultime `window` tx (stati e getTransaction senza disco).
    recent: HashMap<Signature, TxRecord>,
    /// (firma, offset del suo blocco nel registro), dalla più vecchia.
    order: VecDeque<(Signature, u64)>,
    window: usize,
    /// Ultimo record di ogni indirizzo (offset in txlog.bin).
    heads: HashMap<Address, u64>,
    log: File,
    log_len: u64,
    sigs: SigIndex,
    /// Altezza dell'ultimo blocco registrato (i blocchi fino a qui non si ripetono).
    pub height: u64,
    pub count: u64,
    dir: std::path::PathBuf,
}

const GROUP: &[u8; 4] = b"TXG1";

fn checksum(b: &[u8]) -> [u8; 8] {
    Sha256::digest(b)[..8].try_into().unwrap()
}

impl TxIndex {
    /// Apre (o crea) lo storico in `dir`: tronca l'ultimo blocco incompleto, reinserisce
    /// le voci dei blocchi dopo l'ultimo salvataggio delle teste e ricostruisce l'indice
    /// se manca.
    pub fn open(dir: &Path, window: usize) -> Result<TxIndex, String> {
        let e = |x: io::Error| format!("indice tx: {x}");
        let log = OpenOptions::new().read(true).append(true).create(true).open(dir.join("txlog.bin")).map_err(e)?;
        let (sigs, fresh) = SigIndex::open(dir).map_err(e)?;
        let (mut heads, mut from, mut height, mut count, mut window_from) = (HashMap::new(), 0, 0, 0, 0);
        if let Ok(b) = std::fs::read(dir.join("txheads")) {
            if b.len() >= 32 && b.len() % 40 == 32 && !fresh {
                let g = |i: usize| u64::from_le_bytes(b[i * 8..i * 8 + 8].try_into().unwrap());
                (from, height, count, window_from) = (g(0), g(1), g(2), g(3).min(g(0)));
                heads = b[32..].chunks(40).map(|c| (Address::new_from_array(c[..32].try_into().unwrap()), u64::from_le_bytes(c[32..].try_into().unwrap()))).collect();
            }
        }
        let mut ix = TxIndex { recent: HashMap::new(), order: VecDeque::new(), window, heads, log_len: 0, log, sigs, height, count, dir: dir.into() };
        let len = ix.log.metadata().map_err(e)?.len();
        // Rilegge il registro dall'inizio della finestra in RAM: prima di `from` riempie
        // solo la finestra; da `from` anche teste e voci dell'indice (ultimo blocco completo).
        let mut off = window_from;
        while let Some((h, recs, next)) = ix.read_group(off, len).map_err(e)? {
            for (o, r, links) in recs {
                let sig = r.tx.signatures[0];
                if off >= from {
                    ix.sigs.insert(&sig, o).map_err(e)?;
                    for (k, _) in links {
                        ix.heads.insert(k, o);
                    }
                    ix.count += 1;
                    ix.height = h;
                }
                ix.remember(sig, off, r);
            }
            off = next;
        }
        if off < len {
            ix.log.set_len(off).map_err(e)?; // blocco scritto a metà
        }
        ix.log_len = off;
        ix.sigs.save().map_err(e)?;
        Ok(ix)
    }

    /// Un blocco del registro a partire da `off`: (altezza, record, offset successivo).
    fn read_group(&self, off: u64, len: u64) -> io::Result<Option<(u64, Vec<(u64, TxRecord, Vec<(Address, u64)>)>, u64)>> {
        if off + 24 > len {
            return Ok(None);
        }
        let mut h = [0u8; 16];
        self.log.read_exact_at(&mut h, off)?;
        let body_len = u32::from_le_bytes(h[12..16].try_into().unwrap()) as u64;
        if &h[..4] != GROUP || off + 16 + body_len + 8 > len {
            return Ok(None);
        }
        let mut body = vec![0u8; body_len as usize + 8];
        self.log.read_exact_at(&mut body, off + 16)?;
        let (body, sum) = body.split_at(body_len as usize);
        let mut all = h.to_vec();
        all.extend_from_slice(body);
        if checksum(&all) != sum {
            return Ok(None);
        }
        let height = u64::from_le_bytes(h[4..12].try_into().unwrap());
        let (mut r, mut recs) = (R(body), vec![]);
        while !r.0.is_empty() {
            let at = off + 16 + (body_len - r.0.len() as u64);
            let (rec, links) = decode(r.bytes()?)?;
            recs.push((at, rec, links));
        }
        Ok(Some((height, recs, off + 16 + body_len + 8)))
    }

    /// Registra un blocco eseguito (una sola volta per altezza). `node_us`: latenza nel nodo.
    pub fn record(&mut self, ex: &Executed, node_us: impl Fn(&Signature) -> Option<u64>) -> Result<(), String> {
        let h = &ex.block.header;
        if h.height <= self.height && self.count > 0 {
            return Ok(());
        }
        let e = |x: io::Error| format!("indice tx: {x}");
        let mut body = Vec::new();
        let mut placed = vec![];
        for ((tx, result), (pre, post)) in ex.block.txs.iter().zip(&ex.results).zip(&ex.balances) {
            let meta = match result {
                Ok(m) => m,
                Err(f) => &f.meta,
            };
            let rd = &meta.return_data;
            let rec = TxRecord {
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
            };
            let at = self.log_len + 16 + body.len() as u64;
            let mut keys: Vec<Address> = tx.message.static_account_keys().to_vec();
            keys.dedup();
            let links: Vec<(Address, u64)> = keys.iter().map(|k| (*k, self.heads.get(k).copied().unwrap_or(u64::MAX))).collect();
            let b = encode(&rec, &links);
            body.extend_from_slice(&(b.len() as u32).to_le_bytes());
            body.extend_from_slice(&b);
            for k in keys {
                self.heads.insert(k, at);
            }
            placed.push((at, rec));
        }
        if placed.is_empty() {
            self.height = h.height;
            return Ok(());
        }
        let mut g = GROUP.to_vec();
        g.extend_from_slice(&h.height.to_le_bytes());
        g.extend_from_slice(&(body.len() as u32).to_le_bytes());
        g.extend_from_slice(&body);
        let sum = checksum(&g);
        g.extend_from_slice(&sum);
        self.log.write_all(&g).map_err(e)?;
        self.log_len += g.len() as u64;
        let group = self.log_len - g.len() as u64;
        for (at, rec) in placed {
            let sig = rec.tx.signatures[0];
            self.sigs.insert(&sig, at).map_err(e)?;
            self.count += 1;
            self.remember(sig, group, rec);
        }
        self.sigs.save().map_err(e)?;
        self.height = h.height;
        Ok(())
    }

    fn remember(&mut self, sig: Signature, group: u64, rec: TxRecord) {
        self.order.push_back((sig, group));
        self.recent.insert(sig, rec);
        while self.order.len() > self.window {
            let (s, _) = self.order.pop_front().unwrap();
            self.recent.remove(&s);
        }
    }

    /// A ogni snapshot: dati su disco (fsync) e teste per indirizzo, in modo atomico.
    pub fn checkpoint(&mut self) -> Result<(), String> {
        let e = |x: io::Error| format!("indice tx: {x}");
        self.log.sync_all().map_err(e)?;
        self.sigs.file.sync_all().map_err(e)?;
        self.sigs.tab.sync_all().map_err(e)?;
        let mut b = Vec::with_capacity(32 + self.heads.len() * 40);
        let window_from = self.order.front().map_or(self.log_len, |x| x.1);
        for v in [self.log_len, self.height, self.count, window_from] {
            b.extend_from_slice(&v.to_le_bytes());
        }
        for (k, o) in &self.heads {
            b.extend_from_slice(k.as_ref());
            b.extend_from_slice(&o.to_le_bytes());
        }
        let tmp = self.dir.join("txheads.tmp");
        let mut f = File::create(&tmp).map_err(e)?;
        f.write_all(&b).map_err(e)?;
        f.sync_all().map_err(e)?;
        std::fs::rename(&tmp, self.dir.join("txheads")).map_err(e)
    }

    fn at(&self, off: u64) -> io::Result<(TxRecord, Vec<(Address, u64)>)> {
        let mut n = [0u8; 4];
        self.log.read_exact_at(&mut n, off)?;
        let mut b = vec![0u8; u32::from_le_bytes(n) as usize];
        self.log.read_exact_at(&mut b, off + 4)?;
        decode(&b)
    }

    /// Tx recente (finestra in RAM).
    pub fn recent(&self, sig: &Signature) -> Option<&TxRecord> {
        self.recent.get(sig)
    }

    pub fn recent_mut(&mut self, sig: &Signature) -> Option<&mut TxRecord> {
        self.recent.get_mut(sig)
    }

    /// Qualsiasi tx: finestra in RAM, altrimenti storico su disco (firma verificata).
    pub fn get(&self, sig: &Signature) -> Option<TxRecord> {
        if let Some(r) = self.recent.get(sig) {
            return Some(r.clone());
        }
        let offs = self.sigs.candidates(sig).ok()?;
        offs.into_iter().filter(|o| *o < self.log_len).filter_map(|o| self.at(o).ok()).map(|x| x.0).find(|r| r.tx.signatures[0] == *sig)
    }

    /// Tx che toccano `addr`, dalla più recente: segue i collegamenti tra record.
    pub fn history(&self, addr: &Address, before: Option<Signature>, until: Option<Signature>, limit: usize) -> Vec<HistoryEntry> {
        let (mut out, mut next) = (vec![], self.heads.get(addr).copied());
        let mut started = before.is_none();
        while let Some(off) = next.filter(|o| *o < self.log_len) {
            let Ok((r, links)) = self.at(off) else { break };
            next = links.iter().find(|(k, _)| k == addr).map(|x| x.1).filter(|o| *o != u64::MAX);
            let sig = r.tx.signatures[0];
            if !started {
                started = Some(sig) == before;
                continue;
            }
            if Some(sig) == until || out.len() == limit {
                break;
            }
            out.push(HistoryEntry { sig, slot: r.slot, time_ms: r.time_ms, err: r.err });
        }
        out
    }

    pub fn recent_len(&self) -> usize {
        self.recent.len()
    }
}
