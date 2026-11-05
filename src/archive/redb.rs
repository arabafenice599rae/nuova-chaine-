//! Archivio su `redb` (database embedded, B-tree con copia su scrittura e
//! transazioni ACID). Una transazione per blocco con durabilità `None` (nessun
//! fsync), `Immediate` a ogni checkpoint: dopo un arresto il database torna
//! all'ultimo checkpoint e i blocchi successivi si riregistrano dal journal.
//!   recs   id -> record codificato
//!   sigs   64 bit della firma -> id (multimappa: verifica della firma intera)
//!   addrs  indirizzo ‖ id (big endian) -> ()
//!   blocks altezza -> (primo id, id successivo), per la conservazione

use std::io;
use std::path::{Path, PathBuf};

use redb::{Database, Durability, MultimapTableDefinition, ReadableDatabase, ReadableTable, TableDefinition};
use solana_address::Address;
use solana_signature::Signature;

use super::{Archive, HistoryEntry, TxRecord, decode, encode, keys};

const RECS: TableDefinition<u64, &[u8]> = TableDefinition::new("recs");
const SIGS: MultimapTableDefinition<u64, u64> = MultimapTableDefinition::new("sigs");
const ADDRS: TableDefinition<&[u8], ()> = TableDefinition::new("addrs");
const BLOCKS: TableDefinition<u64, (u64, u64)> = TableDefinition::new("blocks");

fn e<E: std::fmt::Display>(x: E) -> io::Error {
    io::Error::other(x.to_string())
}

fn sig_key(s: &Signature) -> u64 {
    u64::from_le_bytes(s.as_ref()[..8].try_into().unwrap())
}

pub struct Redb {
    db: Database,
    path: PathBuf,
    next_id: u64,
    height: u64,
}

impl Redb {
    pub fn open(dir: &Path) -> io::Result<Redb> {
        std::fs::create_dir_all(dir)?;
        let path = dir.join("history.redb");
        let db = Database::create(&path).map_err(e)?;
        let w = db.begin_write().map_err(e)?;
        let (mut next_id, mut height) = (0, 0);
        {
            w.open_table(RECS).map_err(e)?;
            w.open_multimap_table(SIGS).map_err(e)?;
            w.open_table(ADDRS).map_err(e)?;
            let b = w.open_table(BLOCKS).map_err(e)?;
            if let Some((h, v)) = b.last().map_err(e)? {
                (height, next_id) = (h.value(), v.value().1);
            }
        }
        w.commit().map_err(e)?;
        Ok(Redb { db, path, next_id, height })
    }

    fn read(&self, id: u64) -> Option<TxRecord> {
        let r = self.db.begin_read().ok()?;
        let t = r.open_table(RECS).ok()?;
        let v = t.get(id).ok()??;
        decode(v.value()).ok().map(|x| x.0)
    }
}

impl Archive for Redb {
    fn append(&mut self, height: u64, recs: Vec<TxRecord>) -> io::Result<()> {
        if height <= self.height && self.height > 0 {
            return Ok(());
        }
        let mut w = self.db.begin_write().map_err(e)?;
        w.set_durability(Durability::None).map_err(e)?;
        let first = self.next_id;
        {
            let (mut rt, mut st, mut at, mut bt) = (w.open_table(RECS).map_err(e)?, w.open_multimap_table(SIGS).map_err(e)?, w.open_table(ADDRS).map_err(e)?, w.open_table(BLOCKS).map_err(e)?);
            for rec in &recs {
                let id = self.next_id;
                self.next_id += 1;
                rt.insert(id, encode(rec, &[]).as_slice()).map_err(e)?;
                st.insert(sig_key(&rec.tx.signatures[0]), id).map_err(e)?;
                for k in keys(rec) {
                    let key = [k.as_ref(), &id.to_be_bytes()].concat();
                    at.insert(key.as_slice(), ()).map_err(e)?;
                }
            }
            bt.insert(height, (first, self.next_id)).map_err(e)?;
        }
        w.commit().map_err(e)?;
        self.height = height;
        Ok(())
    }

    fn get(&self, sig: &Signature) -> Option<TxRecord> {
        let r = self.db.begin_read().ok()?;
        let t = r.open_multimap_table(SIGS).ok()?;
        let ids: Vec<u64> = t.get(sig_key(sig)).ok()?.filter_map(|x| x.ok().map(|v| v.value())).collect();
        drop((t, r));
        ids.into_iter().filter_map(|id| self.read(id)).find(|x| x.tx.signatures[0] == *sig)
    }

    fn history(&self, addr: &Address, before: Option<Signature>, until: Option<Signature>, limit: usize) -> Vec<HistoryEntry> {
        let Ok(r) = self.db.begin_read() else { return vec![] };
        let (Ok(t), Ok(recs)) = (r.open_table(ADDRS), r.open_table(RECS)) else { return vec![] };
        let (lo, hi) = ([addr.as_ref(), &[0u8; 8]].concat(), [addr.as_ref(), &[0xffu8; 8]].concat());
        let Ok(range) = t.range(lo.as_slice()..=hi.as_slice()) else { return vec![] };
        let (mut out, mut started) = (vec![], before.is_none());
        for item in range.rev() {
            let Ok((k, _)) = item else { break };
            let id = u64::from_be_bytes(k.value()[32..40].try_into().unwrap());
            let Some(rec) = recs.get(id).ok().flatten().and_then(|v| decode(v.value()).ok()) else { continue };
            let sig = rec.0.tx.signatures[0];
            if !started {
                started = Some(sig) == before;
                continue;
            }
            if Some(sig) == until || out.len() == limit {
                break;
            }
            out.push(HistoryEntry { sig, slot: rec.0.slot, time_ms: rec.0.time_ms, err: rec.0.err });
        }
        out
    }

    fn checkpoint(&mut self) -> io::Result<u64> {
        let mut w = self.db.begin_write().map_err(e)?;
        w.set_durability(Durability::Immediate).map_err(e)?;
        w.commit().map_err(e)?;
        Ok(self.height)
    }

    fn height(&self) -> u64 {
        self.height
    }

    fn prune(&mut self, height: u64) -> io::Result<()> {
        let w = self.db.begin_write().map_err(e)?;
        {
            let (mut rt, mut st, mut at, mut bt) = (w.open_table(RECS).map_err(e)?, w.open_multimap_table(SIGS).map_err(e)?, w.open_table(ADDRS).map_err(e)?, w.open_table(BLOCKS).map_err(e)?);
            let old: Vec<(u64, (u64, u64))> = bt.range(..height).map_err(e)?.filter_map(|x| x.ok().map(|(k, v)| (k.value(), v.value()))).collect();
            for (h, (a, b)) in old {
                for id in a..b {
                    if let Some(v) = rt.remove(id).map_err(e)? {
                        let (rec, _) = decode(v.value())?;
                        st.remove(sig_key(&rec.tx.signatures[0]), id).map_err(e)?;
                        for k in keys(&rec) {
                            at.remove([k.as_ref(), &id.to_be_bytes()].concat().as_slice()).map_err(e)?;
                        }
                    }
                }
                bt.remove(h).map_err(e)?;
            }
        }
        w.commit().map_err(e)
    }

    fn disk_bytes(&self) -> u64 {
        std::fs::metadata(&self.path).map_or(0, |m| m.len())
    }
}
