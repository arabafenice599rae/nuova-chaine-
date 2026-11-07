//! `centrifuga indexer <config>`: storico delle transazioni fuori dal nodo.
//! Legge il journal del nodo in sola lettura (anche mentre cresce), riesegue i blocchi
//! su uno stato suo (i risultati, i log e le fee non sono nel journal), verifica firme
//! e hash di stato come una replica, e scrive l'archivio. Serve via JSON-RPC
//! getTransaction, getSignaturesForAddress e getSignatureStatuses (storico).
//! Il nodo non lo aspetta mai: se l'indexer è spento o in ritardo, recupera da solo.

use std::fs::File;
use std::io;
use std::os::unix::fs::FileExt;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde_json::{Value, json};

use crate::archive::{Archive, records_of};
use crate::exec::Chain;
use crate::genesis::{Config, IndexerConfig};
use crate::ledger::{self, Block};
use crate::rpc::methods;

pub type Shared = Arc<Mutex<Box<dyn Archive>>>;

pub fn open_archive(c: &IndexerConfig) -> Result<Box<dyn Archive>, String> {
    let dir = c.data_dir.join("archive");
    let e = |x: io::Error| format!("archivio: {x}");
    Ok(Box::new(crate::segments::Segments::open(&dir, c.segment_txs).map_err(e)?))
}

/// Blocco del journal a `off` (None se incompleto: il nodo lo sta ancora scrivendo).
fn block_at(f: &File, off: u64) -> io::Result<Option<(Block, u64)>> {
    let len = f.metadata()?.len();
    let mut n = [0u8; 4];
    if off + 4 > len {
        return Ok(None);
    }
    f.read_exact_at(&mut n, off)?;
    let n = u32::from_le_bytes(n) as u64;
    if off + 4 + n > len {
        return Ok(None);
    }
    let mut b = vec![0u8; n as usize];
    f.read_exact_at(&mut b, off + 4)?;
    Ok(Some((Block::decode(&b).map_err(io::Error::other)?, off + 4 + n)))
}

pub struct Indexer {
    pub chain: Chain,
    pub archive: Shared,
    journal: File,
    offset: u64,
    /// Altezza dello snapshot dello stato: i blocchi fino a qui si annotano soltanto.
    snap_height: u64,
    cfg: IndexerConfig,
    state_dir: PathBuf,
}

impl Indexer {
    /// Stato dall'ultimo snapshot dell'indexer, poi il journal fino in fondo.
    pub fn open(cfg: &Config, journal: &Path) -> Result<Indexer, String> {
        let c = cfg.indexer.clone().ok_or("manca la sezione [indexer]")?;
        let state_dir = c.data_dir.join("state");
        std::fs::create_dir_all(&state_dir).map_err(|e| e.to_string())?;
        // Archivio illeggibile o indietro rispetto allo stato: si mette da parte e si
        // ricostruisce dal journal, dal genesis (più lento, ma corretto e automatico).
        let archive = match open_archive(&c) {
            Ok(a) if a.height() >= ledger::read_snapshot(&state_dir).ok().flatten().map_or(0, |s| s.height) => a,
            other => {
                let why = other.err().unwrap_or_else(|| "indietro rispetto allo stato dell'indexer".into());
                let aside = c.data_dir.join(format!("archive.broken-{}", crate::node::now_ms()));
                eprintln!("indexer: archivio non utilizzabile ({why}): spostato in {}, lo ricostruisco dal journal", aside.display());
                std::fs::rename(c.data_dir.join("archive"), &aside).map_err(|e| e.to_string())?;
                let _ = std::fs::remove_file(ledger::snapshot_path(&state_dir));
                open_archive(&c)?
            }
        };
        let (chain, snap_height) = crate::node::chain_from_snapshot(cfg, &state_dir)?;
        let archive: Shared = Arc::new(Mutex::new(archive));
        let journal = File::open(journal).map_err(|e| format!("{}: {e}", journal.display()))?;
        let mut ix = Indexer { chain, archive, journal, offset: 0, snap_height, cfg: c, state_dir };
        while ix.step()? {}
        Ok(ix)
    }

    /// Un blocco del journal, se c'è. Prima dell'altezza dello stato: solo finestra dei
    /// blockhash; dopo: esecuzione e archivio.
    pub fn step(&mut self) -> Result<bool, String> {
        let Some((b, next)) = block_at(&self.journal, self.offset).map_err(|e| e.to_string())? else { return Ok(false) };
        let h = b.header.height;
        if h <= self.snap_height {
            if h == self.snap_height && self.chain.state_hash() != b.header.state_hash {
                return Err(format!("indexer: snapshot {h} diverso dal journal (journal di un'altra chain?)"));
            }
            self.chain.note_block(&b)?;
        } else {
            let ex = self.chain.apply(&b).map_err(|e| format!("indexer, blocco {h}: {e} (journal di un'altra chain o reset del nodo?)"))?;
            let mut a = self.archive.lock().unwrap();
            if h > a.height() {
                a.append(h, records_of(&ex, |_| None)).map_err(|e| format!("archivio: {e}"))?;
                let keep = self.cfg.retention_blocks;
                if keep > 0 && h % 1_000 == 0 && h > keep {
                    a.prune(h - keep).map_err(|e| format!("archivio: {e}"))?;
                }
            }
            drop(a);
            if self.cfg.snapshot_every > 0 && h % self.cfg.snapshot_every == 0 {
                self.checkpoint()?;
            }
        }
        self.offset = next;
        Ok(true)
    }

    /// Archivio durevole, poi snapshot dello stato dell'indexer alla stessa altezza.
    pub fn checkpoint(&mut self) -> Result<(), String> {
        self.archive.lock().unwrap().checkpoint().map_err(|e| format!("archivio: {e}"))?;
        let c = &self.chain;
        ledger::write_snapshot(&self.state_dir, c.height, c.genesis_hash, c.state_hash(), c.svm.accounts().iter()).map_err(|e| format!("snapshot indexer: {e}"))
    }

    /// Segue il journal: un blocco appena arriva, altrimenti attende.
    pub fn follow(mut self) -> Result<(), String> {
        loop {
            if !self.step()? {
                std::thread::sleep(Duration::from_millis(20));
            }
        }
    }
}

/// Risposta JSON-RPC dall'archivio.
pub fn call(a: &dyn Archive, req: &Value) -> Value {
    let id = req.get("id").cloned().unwrap_or(Value::Null);
    let p = req.get("params").cloned().unwrap_or(json!([]));
    let sig = |v: &Value| v.as_str().and_then(|s| s.parse::<solana_signature::Signature>().ok());
    let result: Result<Value, String> = match req["method"].as_str().unwrap_or("") {
        "getTransaction" => Ok(sig(&p[0]).and_then(|s| a.get(&s)).map_or(Value::Null, |r| methods::transaction(&r, &p[1]))),
        "getSignaturesForAddress" => (|| {
            let addr = p[0].as_str().and_then(|s| s.parse().ok()).ok_or("indirizzo non valido")?;
            let (limit, before, until) = methods::history_args(&p).map_err(|e| e.message)?;
            Ok(methods::history_json(a.history(&addr, before, until, limit)))
        })(),
        "getSignatureStatuses" => {
            let list = p[0].as_array().cloned().unwrap_or_default();
            let v: Vec<Value> = list.iter().map(|s| sig(s).and_then(|s| a.get(&s)).map_or(Value::Null, |r| methods::status(r.slot, r.err()))).collect();
            Ok(json!({"context": {"slot": a.height()}, "value": v}))
        }
        "getHealth" => Ok(json!("ok")),
        "centrifugaIndexerStatus" => Ok(json!({"height": a.height(), "diskBytes": a.disk_bytes()})),
        _ => return json!({"jsonrpc": "2.0", "id": id, "error": {"code": -32601, "message": "Method not found"}}),
    };
    match result {
        Ok(v) => json!({"jsonrpc": "2.0", "id": id, "result": v}),
        Err(e) => json!({"jsonrpc": "2.0", "id": id, "error": {"code": -32602, "message": e}}),
    }
}

/// Server JSON-RPC dell'indexer (stesso trasporto HTTP del nodo, archivio sotto mutex).
pub async fn serve(archive: Shared, addr: std::net::SocketAddr) -> io::Result<()> {
    use http_body_util::{BodyExt, Full};
    use hyper::body::Bytes;
    let l = tokio::net::TcpListener::bind(addr).await?;
    eprintln!("indexer rpc: http://{addr}");
    loop {
        let (s, _) = l.accept().await?;
        let archive = archive.clone();
        tokio::spawn(async move {
            let svc = hyper::service::service_fn(move |r: hyper::Request<hyper::body::Incoming>| {
                let archive = archive.clone();
                async move {
                    let body = r.into_body().collect().await.map(|b| b.to_bytes()).unwrap_or_default();
                    let out = match serde_json::from_slice::<Value>(&body) {
                        Ok(req) => {
                            let a = archive.clone();
                            tokio::task::spawn_blocking(move || call(a.lock().unwrap().as_ref(), &req)).await.unwrap_or(Value::Null)
                        }
                        Err(_) => json!({"jsonrpc": "2.0", "id": null, "error": {"code": -32700, "message": "Parse error"}}),
                    };
                    Ok::<_, std::convert::Infallible>(hyper::Response::new(Full::new(Bytes::from(out.to_string()))))
                }
            });
            let _ = hyper::server::conn::http1::Builder::new().serve_connection(hyper_util::rt::TokioIo::new(s), svc).await;
        });
    }
}

/// `centrifuga indexer`: segue il journal del nodo e serve lo storico.
pub fn run(cfg: Config) -> Result<(), String> {
    let c = cfg.indexer.clone().ok_or("manca la sezione [indexer]")?;
    let journal = cfg.node.data_dir.join("journal.bin");
    let ix = Indexer::open(&cfg, &journal)?;
    eprintln!("indexer: altezza {}, archivio a segmenti in {}", ix.chain.height, c.data_dir.display());
    let archive = ix.archive.clone();
    let addr = c.rpc_addr.parse().map_err(|e| format!("rpc_addr: {e}"))?;
    std::thread::spawn(move || {
        let rt = tokio::runtime::Runtime::new().expect("tokio");
        if let Err(e) = rt.block_on(serve(archive, addr)) {
            eprintln!("indexer rpc: {e}");
        }
    });
    ix.follow()
}
