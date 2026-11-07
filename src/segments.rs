//! Archivio a segmenti append-only (la soluzione più semplice):
//!   seg-<altezza>.log  record per blocco, in coda (lunghezza + checksum);
//!   seg-<altezza>.sig  a segmento chiuso: (64 bit della firma, offset) ordinati;
//!   seg-<altezza>.adr  a segmento chiuso: (indirizzo, offset) ordinati.
//! Il segmento attivo ha gli indici in RAM (al più `segment_txs` tx). Ricerca: dal
//! segmento più recente al più vecchio, ricerca binaria sui file degli indici.
//! Conservazione: si cancellano interi segmenti. Recupero dopo un arresto: il
//! segmento attivo si rilegge e si tronca all'ultimo blocco completo; un segmento
//! chiuso senza indici (arresto durante la chiusura) si reindicizza.

use std::collections::HashMap;
use std::fs::{File, OpenOptions};
use std::io::{self, Write};
use std::os::unix::fs::FileExt;
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};
use solana_address::Address;
use solana_signature::Signature;

use crate::archive::{Archive, HistoryEntry, R, TxRecord, decode, encode, keys};

const GROUP: &[u8; 4] = b"SEG1";

fn checksum(b: &[u8]) -> [u8; 8] {
    Sha256::digest(b)[..8].try_into().unwrap()
}

fn sig_key(s: &Signature) -> u64 {
    u64::from_le_bytes(s.as_ref()[..8].try_into().unwrap())
}

struct Sealed {
    first: u64,
    log: File,
    sig: File,
    sig_n: u64,
    adr: File,
    adr_n: u64,
}

pub struct Segments {
    dir: PathBuf,
    segment_txs: u64,
    sealed: Vec<Sealed>, // dal più vecchio
    first: u64,
    log: File,
    len: u64,
    sigs: HashMap<u64, Vec<u64>>,
    addrs: HashMap<Address, Vec<u64>>,
    txs: u64,
    height: u64,
}

fn name(dir: &Path, first: u64, ext: &str) -> PathBuf {
    dir.join(format!("seg-{first:020}.{ext}"))
}

/// Blocchi completi di un file: (altezza, [(offset, record)]); si ferma al primo incompleto.
fn scan(f: &File, mut off: u64, len: u64, mut each: impl FnMut(u64, u64, TxRecord)) -> io::Result<u64> {
    while off + 24 <= len {
        let mut h = [0u8; 16];
        f.read_exact_at(&mut h, off)?;
        let body_len = u32::from_le_bytes(h[12..16].try_into().unwrap()) as u64;
        if &h[..4] != GROUP || off + 16 + body_len + 8 > len {
            break;
        }
        let mut body = vec![0u8; body_len as usize + 8];
        f.read_exact_at(&mut body, off + 16)?;
        let (b, sum) = body.split_at(body_len as usize);
        let mut all = h.to_vec();
        all.extend_from_slice(b);
        if checksum(&all) != sum {
            break;
        }
        let height = u64::from_le_bytes(h[4..12].try_into().unwrap());
        let mut r = R(b);
        while !r.0.is_empty() {
            let at = off + 16 + (body_len - r.0.len() as u64);
            each(height, at, decode(r.bytes()?)?.0);
        }
        off += 16 + body_len + 8;
    }
    Ok(off)
}

fn read_rec(f: &File, off: u64) -> io::Result<TxRecord> {
    let mut n = [0u8; 4];
    f.read_exact_at(&mut n, off)?;
    let mut b = vec![0u8; u32::from_le_bytes(n) as usize];
    f.read_exact_at(&mut b, off + 4)?;
    Ok(decode(&b)?.0)
}

/// Indice ordinato su file: voci a dimensione fissa `w`, chiave nei primi `k` byte.
fn write_index(path: &Path, mut entries: Vec<Vec<u8>>, trailer: Option<u64>) -> io::Result<()> {
    entries.sort();
    let tmp = path.with_extension("tmp");
    let mut f = std::io::BufWriter::new(File::create(&tmp)?);
    for e in &entries {
        f.write_all(e)?;
    }
    if let Some(t) = trailer {
        f.write_all(&t.to_le_bytes())?; // altezza dell'ultimo blocco del segmento
    }
    f.into_inner().map_err(|e| e.into_error())?.sync_all()?;
    std::fs::rename(&tmp, path)
}

/// Offset delle voci con chiave `key` (ricerca binaria, poi scansione delle uguali).
fn lookup(f: &File, n: u64, w: usize, key: &[u8]) -> Vec<u64> {
    let read = |i: u64| {
        let mut b = vec![0u8; w];
        f.read_exact_at(&mut b, i * w as u64).ok().map(|_| b)
    };
    let (mut lo, mut hi) = (0, n);
    while lo < hi {
        let mid = (lo + hi) / 2;
        match read(mid) {
            Some(b) if &b[..key.len()] < key => lo = mid + 1,
            _ => hi = mid,
        }
    }
    let mut out = vec![];
    while let Some(b) = (lo < n).then(|| read(lo)).flatten().filter(|b| &b[..key.len()] == key) {
        out.push(u64::from_le_bytes(b[key.len()..].try_into().unwrap()));
        lo += 1;
    }
    out
}

impl Segments {
    pub fn open(dir: &Path, segment_txs: u64) -> io::Result<Segments> {
        std::fs::create_dir_all(dir)?;
        let mut firsts: Vec<u64> =
            std::fs::read_dir(dir)?.flatten().filter_map(|e| e.file_name().to_str()?.strip_prefix("seg-")?.strip_suffix(".log")?.parse().ok()).collect();
        firsts.sort();
        let active = firsts.pop().unwrap_or(1);
        let mut s = Segments {
            dir: dir.into(),
            segment_txs,
            sealed: vec![],
            first: active,
            log: OpenOptions::new().read(true).append(true).create(true).open(name(dir, active, "log"))?,
            len: 0,
            sigs: HashMap::new(),
            addrs: HashMap::new(),
            txs: 0,
            height: 0,
        };
        for f in firsts {
            s.load_sealed(f)?;
        }
        let len = s.log.metadata()?.len();
        let (mut sigs, mut addrs, mut txs, mut height) = (HashMap::<u64, Vec<u64>>::new(), HashMap::<Address, Vec<u64>>::new(), 0, s.height);
        let end = scan(&s.log, 0, len, |h, at, r| {
            sigs.entry(sig_key(&r.tx.signatures[0])).or_default().push(at);
            for k in keys(&r) {
                addrs.entry(k).or_default().push(at);
            }
            txs += 1;
            height = h;
        })?;
        if end < len {
            s.log.set_len(end)?;
        }
        (s.sigs, s.addrs, s.txs, s.height, s.len) = (sigs, addrs, txs, height, end);
        Ok(s)
    }

    /// Segmento chiuso: apre gli indici, o li ricostruisce se mancano.
    fn load_sealed(&mut self, first: u64) -> io::Result<()> {
        let log = File::open(name(&self.dir, first, "log"))?;
        let (sp, ap) = (name(&self.dir, first, "sig"), name(&self.dir, first, "adr"));
        let mut last = 0;
        if !sp.exists() || !ap.exists() {
            let (mut se, mut ae) = (vec![], vec![]);
            scan(&log, 0, log.metadata()?.len(), |h, at, r| {
                se.push([sig_key(&r.tx.signatures[0]).to_be_bytes(), at.to_le_bytes()].concat());
                for k in keys(&r) {
                    ae.push([k.as_ref(), &at.to_le_bytes()].concat());
                }
                last = h;
            })?;
            write_index(&sp, se, Some(last))?;
            write_index(&ap, ae, None)?;
        }
        let (sig, adr) = (File::open(&sp)?, File::open(&ap)?);
        let (sig_n, adr_n) = ((sig.metadata()?.len().saturating_sub(8)) / 16, adr.metadata()?.len() / 40);
        let mut t = [0u8; 8];
        sig.read_exact_at(&mut t, sig_n * 16)?;
        last = last.max(u64::from_le_bytes(t));
        self.height = self.height.max(last);
        self.sealed.push(Sealed { first, log, sig, sig_n, adr, adr_n });
        Ok(())
    }

    /// Chiude il segmento attivo (indici su file) e ne apre uno nuovo.
    fn seal(&mut self, next_first: u64) -> io::Result<()> {
        self.log.sync_all()?;
        let se = self.sigs.iter().flat_map(|(k, v)| v.iter().map(move |o| [k.to_be_bytes(), o.to_le_bytes()].concat())).collect();
        let ae = self.addrs.iter().flat_map(|(k, v)| v.iter().map(move |o| [k.as_ref(), &o.to_le_bytes()].concat())).collect();
        write_index(&name(&self.dir, self.first, "sig"), se, Some(self.height))?;
        write_index(&name(&self.dir, self.first, "adr"), ae, None)?;
        let first = self.first;
        self.load_sealed(first)?;
        self.first = next_first;
        self.log = OpenOptions::new().read(true).append(true).create(true).open(name(&self.dir, next_first, "log"))?;
        (self.len, self.txs) = (0, 0);
        self.sigs.clear();
        self.addrs.clear();
        Ok(())
    }
}

impl Archive for Segments {
    fn append(&mut self, height: u64, recs: Vec<TxRecord>) -> io::Result<()> {
        if height <= self.height && self.height > 0 {
            return Ok(());
        }
        if self.txs >= self.segment_txs {
            self.seal(height)?;
        }
        self.height = height;
        if recs.is_empty() {
            return Ok(());
        }
        let mut body = Vec::new();
        for rec in &recs {
            let at = self.len + 16 + body.len() as u64;
            let b = encode(rec, &[]);
            body.extend_from_slice(&(b.len() as u32).to_le_bytes());
            body.extend_from_slice(&b);
            self.sigs.entry(sig_key(&rec.tx.signatures[0])).or_default().push(at);
            for k in keys(rec) {
                self.addrs.entry(k).or_default().push(at);
            }
        }
        let mut g = GROUP.to_vec();
        g.extend_from_slice(&height.to_le_bytes());
        g.extend_from_slice(&(body.len() as u32).to_le_bytes());
        g.extend_from_slice(&body);
        let sum = checksum(&g);
        g.extend_from_slice(&sum);
        self.log.write_all(&g)?;
        self.len += g.len() as u64;
        self.txs += recs.len() as u64;
        Ok(())
    }

    fn get(&self, sig: &Signature) -> Option<TxRecord> {
        let key = sig_key(sig);
        let hit = |f: &File, offs: Vec<u64>| offs.into_iter().filter_map(|o| read_rec(f, o).ok()).find(|r| r.tx.signatures[0] == *sig);
        if let Some(r) = self.sigs.get(&key).and_then(|v| hit(&self.log, v.clone())) {
            return Some(r);
        }
        self.sealed.iter().rev().find_map(|s| hit(&s.log, lookup(&s.sig, s.sig_n, 16, &key.to_be_bytes())))
    }

    fn history(&self, addr: &Address, before: Option<Signature>, until: Option<Signature>, limit: usize) -> Vec<HistoryEntry> {
        let mut out = vec![];
        let mut started = before.is_none();
        let active = self.addrs.get(addr).cloned().unwrap_or_default();
        let lists = std::iter::once((&self.log, active)).chain(self.sealed.iter().rev().map(|s| (&s.log, lookup(&s.adr, s.adr_n, 40, addr.as_ref()))));
        for (f, mut offs) in lists {
            offs.sort_unstable();
            for o in offs.into_iter().rev() {
                let Ok(r) = read_rec(f, o) else { continue };
                let sig = r.tx.signatures[0];
                if !started {
                    started = Some(sig) == before;
                    continue;
                }
                if Some(sig) == until || out.len() == limit {
                    return out;
                }
                out.push(HistoryEntry { sig, slot: r.slot, time_ms: r.time_ms, err: r.err });
            }
        }
        out
    }

    fn checkpoint(&mut self) -> io::Result<u64> {
        self.log.sync_all()?;
        Ok(self.height)
    }

    fn height(&self) -> u64 {
        self.height
    }

    fn prune(&mut self, height: u64) -> io::Result<()> {
        // Un segmento chiuso si elimina se il successivo comincia entro `height`.
        while !self.sealed.is_empty() {
            let next_first = self.sealed.get(1).map_or(self.first, |s| s.first);
            if next_first > height {
                break;
            }
            let s = self.sealed.remove(0);
            for ext in ["log", "sig", "adr"] {
                std::fs::remove_file(name(&self.dir, s.first, ext))?;
            }
        }
        Ok(())
    }

    fn disk_bytes(&self) -> u64 {
        std::fs::read_dir(&self.dir).into_iter().flatten().flatten().filter_map(|e| e.metadata().ok()).map(|m| m.len()).sum()
    }
}
