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
}

impl Journal {
    /// Apre (o crea) il journal e passa ogni blocco a `f`, in ordine.
    /// Un record finale incompleto (crash a metà scrittura) viene troncato.
    pub fn open(path: &Path, mut f: impl FnMut(Block) -> Result<(), String>) -> Result<Journal, String> {
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
            f(Block::decode(&buf)?)?;
            good += 4 + len;
        }
        drop(r);
        if good < total {
            eprintln!("journal: tronco {} byte finali incompleti", total - good);
            file.set_len(good).map_err(|e| e.to_string())?;
        }
        Ok(Journal { file })
    }

    pub fn append(&mut self, b: &Block) -> io::Result<()> {
        let bytes = b.encode();
        let mut rec = Vec::with_capacity(4 + bytes.len());
        rec.extend_from_slice(&(bytes.len() as u32).to_le_bytes());
        rec.extend_from_slice(&bytes);
        self.file.write_all(&rec)?;
        self.file.sync_data()
    }
}

const SNAP_MAGIC: &[u8; 8] = b"CNFASNP1";

pub struct Snapshot {
    pub height: u64,
    pub genesis_hash: Hash,
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
    accounts: impl Iterator<Item = (&'a Address, &'a AccountSharedData)>,
) -> io::Result<()> {
    let mut sorted: Vec<_> = accounts.collect();
    sorted.sort_by_key(|(k, _)| **k);
    let tmp = dir.join("snapshot.tmp");
    let mut w = BufWriter::new(File::create(&tmp)?);
    w.write_all(SNAP_MAGIC)?;
    w.write_all(&height.to_le_bytes())?;
    w.write_all(genesis_hash.as_ref())?;
    w.write_all(&(sorted.len() as u64).to_le_bytes())?;
    for (k, a) in sorted {
        w.write_all(k.as_ref())?;
        w.write_all(&a.lamports().to_le_bytes())?;
        w.write_all(a.owner().as_ref())?;
        w.write_all(&[a.executable() as u8])?;
        w.write_all(&a.rent_epoch().to_le_bytes())?;
        w.write_all(&(a.data().len() as u64).to_le_bytes())?;
        w.write_all(a.data())?;
    }
    w.into_inner().map_err(|e| e.into_error())?.sync_all()?;
    fs::rename(&tmp, snapshot_path(dir))?;
    File::open(dir)?.sync_all()
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
    if &b8 != SNAP_MAGIC {
        return Err(io::Error::other("snapshot: magic errato"));
    }
    let mut u64_ = |r: &mut BufReader<File>| -> io::Result<u64> {
        r.read_exact(&mut b8)?;
        Ok(u64::from_le_bytes(b8))
    };
    let height = u64_(&mut r)?;
    r.read_exact(&mut b32)?;
    let genesis_hash = Hash::new_from_array(b32);
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
    Ok(Some(Snapshot { height, genesis_hash, accounts }))
}
