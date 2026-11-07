//! Persistenza: journal append-only di blocchi firmati + snapshot atomico.

pub mod block;

use std::fs::{self, File, OpenOptions};
use std::io::{self, BufReader, BufWriter, Read, Write};
use std::path::{Path, PathBuf};

use solana_account::{AccountSharedData, ReadableAccount, WritableAccount};
use solana_address::Address;
use solana_hash::Hash;

pub use block::{Block, Header};

/// Journal: sequenza di record `[len u32][blocco]`. Mai riscritto, solo esteso.
pub struct Journal {
    file: File,
    /// Offset di fine file: dove inizierà il prossimo record.
    end: u64,
}

impl Journal {
    /// Apre (o crea) il journal e passa ogni blocco a `f`, in ordine, con l'offset
    /// del suo record. Un record finale incompleto (crash a metà scrittura) viene troncato.
    pub fn open(path: &Path, mut f: impl FnMut(Block, u64) -> Result<(), String>) -> Result<Journal, String> {
        let mut file = OpenOptions::new().read(true).append(true).create(true).open(path).map_err(|e| e.to_string())?;
        let total = file.metadata().map_err(|e| e.to_string())?.len();
        let mut r = BufReader::new(&mut file);
        let mut good = 0u64;
        loop {
            let mut len = [0u8; 4];
            if r.read_exact(&mut len).is_err() {
                break;
            }
            let len = u32::from_le_bytes(len) as u64;
            if good + 4 + len > total {
                break;
            }
            let mut buf = vec![0u8; len as usize];
            r.read_exact(&mut buf).map_err(|e| e.to_string())?;
            f(Block::decode(&buf)?, good)?;
            good += 4 + len;
        }
        drop(r);
        if good < total {
            eprintln!("journal: truncating {} incomplete trailing bytes", total - good);
            file.set_len(good).map_err(|e| e.to_string())?;
        }
        Ok(Journal { file, end: good })
    }
}

/// Legge il blocco che inizia all'offset dato (da un altro handle, in sola lettura).
pub fn read_block_at(file: &mut File, offset: u64) -> Result<Block, String> {
    use std::io::{Seek, SeekFrom};
    file.seek(SeekFrom::Start(offset)).map_err(|e| e.to_string())?;
    let mut len = [0u8; 4];
    file.read_exact(&mut len).map_err(|e| e.to_string())?;
    let mut buf = vec![0u8; u32::from_le_bytes(len) as usize];
    file.read_exact(&mut buf).map_err(|e| e.to_string())?;
    Block::decode(&buf)
}

/// v2: l'header porta anche l'hash di stato (LtHash) dell'altezza dello snapshot.
const SNAP_MAGIC: &[u8; 8] = b"CNFASNP2";

pub struct Snapshot {
    pub height: u64,
    pub genesis_hash: Hash,
    /// Hash di stato all'altezza dello snapshot: chi lo carica lo ricalcola e lo confronta.
    pub state_hash: Hash,
    pub accounts: Vec<(Address, AccountSharedData)>,
}

pub fn snapshot_path(dir: &Path) -> PathBuf {
    dir.join("snapshot.bin")
}

/// Scrive su file temporaneo, fsync, poi rename atomico.
pub fn write_snapshot<'a>(
    dir: &Path,
    height: u64,
    genesis_hash: Hash,
    state_hash: Hash,
    accounts: impl Iterator<Item = (&'a Address, &'a AccountSharedData)>,
) -> io::Result<()> {
    let mut sorted: Vec<_> = accounts.collect();
    sorted.sort_by_key(|(k, _)| **k);
    let tmp = dir.join("snapshot.tmp");
    write_snapshot_file(&tmp, height, genesis_hash, state_hash, sorted.into_iter())?;
    fs::rename(&tmp, snapshot_path(dir))?;
    File::open(dir)?.sync_all()
}

/// Scrive (con fsync) uno snapshot già ordinato per indirizzo in `path`.
pub fn write_snapshot_file<'a>(
    path: &Path,
    height: u64,
    genesis_hash: Hash,
    state_hash: Hash,
    sorted: impl ExactSizeIterator<Item = (&'a Address, &'a AccountSharedData)>,
) -> io::Result<()> {
    let mut w = SnapshotWriter::create(path, height, genesis_hash, state_hash, sorted.len() as u64)?;
    for (k, a) in sorted {
        w.push(k, a)?;
    }
    w.finish()
}

/// Blocchi di scrittura dello snapshot: mai più di ~2 blocchi in sospeso nella cache del
/// disco, così il sync_data del journal non aspetta GB di scritture dello snapshot.
const SNAP_CHUNK: u64 = 8 << 20;

/// Scrittura di uno snapshot un account alla volta, in ordine di indirizzo (lo stato
/// ombra la interrompe per raccogliere i delta che arrivano nel frattempo). Ogni 8 MB:
/// sync_file_range del blocco appena scritto (avvio) e attesa del precedente; con
/// `with_rate`, una pausa per non superare i MB/s indicati.
pub struct SnapshotWriter {
    w: BufWriter<File>,
    pos: u64,
    synced: u64,
    mb_s: u64,
    started: std::time::Instant,
}

impl SnapshotWriter {
    pub fn create(path: &Path, height: u64, genesis_hash: Hash, state_hash: Hash, count: u64) -> io::Result<SnapshotWriter> {
        let mut w = BufWriter::with_capacity(1 << 20, File::create(path)?);
        w.write_all(SNAP_MAGIC)?;
        w.write_all(&height.to_le_bytes())?;
        w.write_all(genesis_hash.as_ref())?;
        w.write_all(state_hash.as_ref())?;
        w.write_all(&count.to_le_bytes())?;
        Ok(SnapshotWriter { w, pos: 88, synced: 0, mb_s: 0, started: std::time::Instant::now() })
    }

    /// Limite di banda della scrittura (0 = nessuno).
    pub fn with_rate(mut self, mb_s: u64) -> SnapshotWriter {
        self.mb_s = mb_s;
        self
    }

    pub fn push(&mut self, k: &Address, a: &AccountSharedData) -> io::Result<()> {
        let w = &mut self.w;
        w.write_all(k.as_ref())?;
        w.write_all(&a.lamports().to_le_bytes())?;
        w.write_all(a.owner().as_ref())?;
        w.write_all(&[a.executable() as u8])?;
        w.write_all(&a.rent_epoch().to_le_bytes())?;
        w.write_all(&(a.data().len() as u64).to_le_bytes())?;
        w.write_all(a.data())?;
        self.pos += 89 + a.data().len() as u64;
        if self.pos - self.synced >= SNAP_CHUNK {
            self.chunk()?;
        }
        Ok(())
    }

    fn chunk(&mut self) -> io::Result<()> {
        use std::os::fd::AsRawFd;
        self.w.flush()?;
        let fd = self.w.get_ref().as_raw_fd();
        let (start, len) = (self.synced as i64, (self.pos - self.synced) as i64);
        // SAFETY: solo hint di scrittura sul file aperto; nessuna memoria coinvolta.
        unsafe {
            libc::sync_file_range(fd, start, len, libc::SYNC_FILE_RANGE_WRITE);
            if start > 0 {
                // Tutto ciò che precede il blocco appena avviato: già quasi tutto su disco.
                let flags = libc::SYNC_FILE_RANGE_WAIT_BEFORE | libc::SYNC_FILE_RANGE_WRITE | libc::SYNC_FILE_RANGE_WAIT_AFTER;
                libc::sync_file_range(fd, 0, start, flags);
            }
        }
        self.synced = self.pos;
        if self.mb_s > 0 {
            let due = std::time::Duration::from_secs_f64(self.pos as f64 / (self.mb_s as f64 * 1e6));
            if let Some(wait) = due.checked_sub(self.started.elapsed()) {
                std::thread::sleep(wait);
            }
        }
        Ok(())
    }

    /// Flush e fsync.
    pub fn finish(self) -> io::Result<()> {
        self.w.into_inner().map_err(|e| e.into_error())?.sync_all()
    }
}

pub fn read_snapshot(dir: &Path) -> io::Result<Option<Snapshot>> {
    let path = snapshot_path(dir);
    if !path.exists() {
        return Ok(None);
    }
    let mut r = BufReader::new(File::open(path)?);
    let mut b8 = [0u8; 8];
    let mut b32 = [0u8; 32];
    r.read_exact(&mut b8)?;
    if &b8 == b"CNFASNP1" {
        return Err(io::Error::other("snapshot: format v1 (without state hash): delete it and restart from the journal"));
    }
    if &b8 != SNAP_MAGIC {
        return Err(io::Error::other("snapshot: bad magic"));
    }
    let mut u64_ = |r: &mut BufReader<File>| -> io::Result<u64> {
        r.read_exact(&mut b8)?;
        Ok(u64::from_le_bytes(b8))
    };
    let height = u64_(&mut r)?;
    r.read_exact(&mut b32)?;
    let genesis_hash = Hash::new_from_array(b32);
    r.read_exact(&mut b32)?;
    let state_hash = Hash::new_from_array(b32);
    let n = u64_(&mut r)?;
    let mut accounts = Vec::with_capacity(n as usize);
    for _ in 0..n {
        r.read_exact(&mut b32)?;
        let key = Address::new_from_array(b32);
        let lamports = u64_(&mut r)?;
        r.read_exact(&mut b32)?;
        let owner = Address::new_from_array(b32);
        let mut ex = [0u8; 1];
        r.read_exact(&mut ex)?;
        let rent_epoch = u64_(&mut r)?;
        let len = u64_(&mut r)? as usize;
        let mut a = AccountSharedData::new(lamports, len, &owner);
        r.read_exact(a.data_as_mut_slice())?;
        a.set_executable(ex[0] == 1);
        a.set_rent_epoch(rent_epoch);
        accounts.push((key, a));
    }
    Ok(Some(Snapshot { height, genesis_hash, state_hash, accounts }))
}

/// Un blocco reso durevole dal thread del journal.
#[derive(Clone, Copy, Debug)]
pub struct Durable {
    pub height: u64,
    pub offset: u64,
    /// Write e sync_data del gruppo che lo contiene (uguali per tutti i blocchi del gruppo).
    pub write: std::time::Duration,
    pub sync: std::time::Duration,
    /// Fine del sync_data (ms unix): solo misura.
    pub at_ms: u64,
}

/// Thread del journal: riceve i blocchi già eseguiti (coda limitata), li scrive e fa
/// sync_data; i blocchi arrivati nel frattempo vanno insieme, con una sola write e un
/// solo sync (group commit). Ogni blocco durevole torna al nodo con il suo offset.
pub struct JournalThread {
    tx: Option<std::sync::mpsc::SyncSender<(u64, Vec<u8>)>>,
    pub done: std::sync::mpsc::Receiver<Result<Durable, String>>,
    handle: Option<std::thread::JoinHandle<()>>,
    /// Solo test: trattiene le scritture (come un disco fermo) o le scarta (crash).
    pub hold: std::sync::Arc<std::sync::atomic::AtomicBool>,
    pub discard: std::sync::Arc<std::sync::atomic::AtomicBool>,
}

impl JournalThread {
    pub fn start(mut journal: Journal, queue: usize) -> Result<JournalThread, String> {
        use std::sync::atomic::Ordering::Relaxed;
        let (tx, rx) = std::sync::mpsc::sync_channel::<(u64, Vec<u8>)>(queue);
        let (dtx, done) = std::sync::mpsc::channel();
        let (hold, discard) = (std::sync::Arc::<std::sync::atomic::AtomicBool>::default(), std::sync::Arc::<std::sync::atomic::AtomicBool>::default());
        let (h, d) = (hold.clone(), discard.clone());
        let handle = std::thread::Builder::new()
            .name("centrifuga-journal".into())
            .spawn(move || {
                while let Ok(first) = rx.recv() {
                    while h.load(Relaxed) && !d.load(Relaxed) {
                        std::thread::sleep(std::time::Duration::from_millis(1));
                    }
                    let mut batch = vec![first];
                    batch.extend(std::iter::from_fn(|| rx.try_recv().ok()));
                    if d.load(Relaxed) {
                        return; // crash simulato: niente arriva su disco
                    }
                    let mut buf = Vec::new();
                    let mut at = Vec::with_capacity(batch.len());
                    for (height, bytes) in &batch {
                        at.push((*height, journal.end + buf.len() as u64));
                        buf.extend_from_slice(&(bytes.len() as u32).to_le_bytes());
                        buf.extend_from_slice(bytes);
                    }
                    let t = std::time::Instant::now();
                    let res = journal.file.write_all(&buf).map(|_| t.elapsed()).and_then(|w| {
                        let t = std::time::Instant::now();
                        journal.file.sync_data().map(|_| (w, t.elapsed()))
                    });
                    match res {
                        Ok((write, sync)) => {
                            journal.end += buf.len() as u64;
                            let at_ms = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_millis() as u64);
                            for (height, offset) in at {
                                let _ = dtx.send(Ok(Durable { height, offset, write, sync, at_ms }));
                            }
                        }
                        Err(e) => {
                            let _ = dtx.send(Err(format!("journal: {e}")));
                            return;
                        }
                    }
                }
            })
            .map_err(|e| format!("journal thread: {e}"))?;
        Ok(JournalThread { tx: Some(tx), done, handle: Some(handle), hold, discard })
    }

    /// Senza thread (dopo un crash simulato nei test).
    pub fn dead() -> JournalThread {
        let (_, done) = std::sync::mpsc::channel();
        JournalThread { tx: None, done, handle: None, hold: Default::default(), discard: Default::default() }
    }

    /// Accoda un blocco; con la coda piena aspetta (backpressure, nessun blocco scartato).
    pub fn send(&self, height: u64, bytes: Vec<u8>) -> Result<(), String> {
        self.tx.as_ref().ok_or("journal closed")?.send((height, bytes)).map_err(|_| "journal thread stopped".to_string())
    }
}

impl Drop for JournalThread {
    /// Chiude la coda: il thread scrive ciò che c'è (salvo crash simulato) e finisce.
    fn drop(&mut self) {
        self.tx = None;
        if let Some(h) = self.handle.take() {
            let _ = h.join();
        }
    }
}
