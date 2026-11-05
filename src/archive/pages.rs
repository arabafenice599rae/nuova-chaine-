//! Archivio scritto a mano (primo prototipo, non definitivo):
//!   txlog.bin  record per blocco, in coda (lunghezza + checksum);
//!   txsig.idx  firma -> record, hash lineare a pagine (cresce di una pagina alla
//!              volta, split con copia su scrittura); txsig.tab prima pagina per secchio;
//!   txheads    ultimo record per indirizzo, salvato a ogni checkpoint; ogni record
//!              punta al precedente di ciascun suo indirizzo.
//! Ogni lettura verifica la firma intera. Arresto del processo: ultimo blocco troncato,
//! voci reinserite (idempotente). Interruzione di corrente: una pagina scritta a metà
//! può perdere voci; si ricostruisce cancellando txsig.idx. Nessuna conservazione
//! limitata (prune non supportato).

use std::collections::HashMap;
use std::fs::{File, OpenOptions};
use std::io::{self, Write};
use std::os::unix::fs::FileExt;
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};
use solana_address::Address;
use solana_signature::Signature;

use super::{Archive, HistoryEntry, R, TxRecord, decode, encode, keys};

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


pub struct Pages {
    heads: HashMap<Address, u64>,
    log: File,
    log_len: u64,
    sigs: SigIndex,
    height: u64,
    count: u64,
    dir: PathBuf,
}

const GROUP: &[u8; 4] = b"TXG1";

fn checksum(b: &[u8]) -> [u8; 8] {
    Sha256::digest(b)[..8].try_into().unwrap()
}

type Group = (u64, Vec<(u64, TxRecord, Vec<(Address, u64)>)>, u64);

impl Pages {
    pub fn open(dir: &Path) -> io::Result<Pages> {
        std::fs::create_dir_all(dir)?;
        let log = OpenOptions::new().read(true).append(true).create(true).open(dir.join("txlog.bin"))?;
        let (sigs, fresh) = SigIndex::open(dir)?;
        let (mut heads, mut from, mut height, mut count) = (HashMap::new(), 0, 0, 0);
        if let Ok(b) = std::fs::read(dir.join("txheads")) {
            if b.len() >= 32 && b.len() % 40 == 32 && !fresh {
                let g = |i: usize| u64::from_le_bytes(b[i * 8..i * 8 + 8].try_into().unwrap());
                (from, height, count) = (g(0), g(1), g(2));
                heads = b[32..].chunks(40).map(|c| (Address::new_from_array(c[..32].try_into().unwrap()), u64::from_le_bytes(c[32..].try_into().unwrap()))).collect();
            }
        }
        let mut a = Pages { heads, log_len: 0, log, sigs, height, count, dir: dir.into() };
        let len = a.log.metadata()?.len();
        let mut off = from;
        while let Some((h, recs, next)) = a.read_group(off, len)? {
            for (o, r, links) in recs {
                a.sigs.insert(&r.tx.signatures[0], o)?;
                for (k, _) in links {
                    a.heads.insert(k, o);
                }
                a.count += 1;
            }
            a.height = h;
            off = next;
        }
        if off < len {
            a.log.set_len(off)?; // blocco scritto a metà
        }
        a.log_len = off;
        a.sigs.save()?;
        Ok(a)
    }

    fn read_group(&self, off: u64, len: u64) -> io::Result<Option<Group>> {
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

    fn at(&self, off: u64) -> io::Result<(TxRecord, Vec<(Address, u64)>)> {
        let mut n = [0u8; 4];
        self.log.read_exact_at(&mut n, off)?;
        let mut b = vec![0u8; u32::from_le_bytes(n) as usize];
        self.log.read_exact_at(&mut b, off + 4)?;
        decode(&b)
    }
}

impl Archive for Pages {
    fn append(&mut self, height: u64, recs: Vec<TxRecord>) -> io::Result<()> {
        if height <= self.height && self.count > 0 {
            return Ok(());
        }
        let (mut body, mut placed) = (Vec::new(), vec![]);
        for rec in recs {
            let at = self.log_len + 16 + body.len() as u64;
            let ks = keys(&rec);
            let links: Vec<(Address, u64)> = ks.iter().map(|k| (*k, self.heads.get(k).copied().unwrap_or(u64::MAX))).collect();
            let b = encode(&rec, &links);
            body.extend_from_slice(&(b.len() as u32).to_le_bytes());
            body.extend_from_slice(&b);
            for k in ks {
                self.heads.insert(k, at);
            }
            placed.push((at, rec.tx.signatures[0]));
        }
        self.height = height;
        if placed.is_empty() {
            return Ok(());
        }
        let mut g = GROUP.to_vec();
        g.extend_from_slice(&height.to_le_bytes());
        g.extend_from_slice(&(body.len() as u32).to_le_bytes());
        g.extend_from_slice(&body);
        let sum = checksum(&g);
        g.extend_from_slice(&sum);
        self.log.write_all(&g)?;
        self.log_len += g.len() as u64;
        for (at, sig) in placed {
            self.sigs.insert(&sig, at)?;
            self.count += 1;
        }
        self.sigs.save()
    }

    fn get(&self, sig: &Signature) -> Option<TxRecord> {
        let offs = self.sigs.candidates(sig).ok()?;
        offs.into_iter().filter(|o| *o < self.log_len).filter_map(|o| self.at(o).ok()).map(|x| x.0).find(|r| r.tx.signatures[0] == *sig)
    }

    fn history(&self, addr: &Address, before: Option<Signature>, until: Option<Signature>, limit: usize) -> Vec<HistoryEntry> {
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

    fn checkpoint(&mut self) -> io::Result<u64> {
        self.log.sync_all()?;
        self.sigs.file.sync_all()?;
        self.sigs.tab.sync_all()?;
        let mut b = Vec::with_capacity(32 + self.heads.len() * 40);
        for v in [self.log_len, self.height, self.count, 0] {
            b.extend_from_slice(&v.to_le_bytes());
        }
        for (k, o) in &self.heads {
            b.extend_from_slice(k.as_ref());
            b.extend_from_slice(&o.to_le_bytes());
        }
        let tmp = self.dir.join("txheads.tmp");
        let mut f = File::create(&tmp)?;
        f.write_all(&b)?;
        f.sync_all()?;
        std::fs::rename(&tmp, self.dir.join("txheads"))?;
        Ok(self.height)
    }

    fn height(&self) -> u64 {
        self.height
    }

    fn prune(&mut self, _height: u64) -> io::Result<()> {
        Err(io::Error::other("archivio a pagine: conservazione limitata non supportata"))
    }

    fn disk_bytes(&self) -> u64 {
        ["txlog.bin", "txsig.idx", "txsig.tab", "txheads"].iter().filter_map(|n| std::fs::metadata(self.dir.join(n)).ok()).map(|m| m.len()).sum()
    }
}
