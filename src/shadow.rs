//! Stato ombra per lo snapshot in background. A ogni blocco il nodo manda il delta
//! dello stream (gli account toccati, `None` per quelli chiusi) e gli stati delle sue tx
//! su un canale limitato; questo thread (a priorità bassa) li applica a una
//! `stream::Replica` con la mappa ordinata, che verifica l'LtHash a ogni blocco.
//! All'altezza dello snapshot scrive la propria mappa in ordine, senza copiarla, e intanto
//! calcola l'LtHash di ciò che scrive: se non coincide con quello del nodo non rinomina il
//! file e segnala l'errore. Durante la scrittura i messaggi in arrivo vanno in una coda
//! locale limitata e si applicano in ordine alla fine; se la coda si riempie lo snapshot
//! si interrompe (saltato, contato) e l'ombra recupera. Il nodo non aspetta mai: se il
//! canale è pieno l'ombra è persa, gli snapshot si saltano (contati) fino al riavvio.

use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{Receiver, SyncSender, TrySendError};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::Duration;

use solana_hash::Hash;

use solana_signature::Signature;
use solana_transaction::TransactionError;

use crate::rpc::index::TxIndex;
use crate::stream::{Accounts, Replica};

/// Blocchi in coda al massimo verso il thread dell'ombra (~60 s a 100 ms per blocco).
pub const QUEUE_BLOCKS: usize = 600;

pub enum Msg {
    /// Delta del blocco e stati delle sue tx (firma, errore).
    Block { height: u64, state_hash: Hash, delta: Arc<Accounts>, statuses: Vec<(Signature, Option<TransactionError>)> },
    Snapshot { height: u64, genesis_hash: Hash, state_hash: Hash },
    /// Attende la fine della scrittura in corso e risponde.
    Barrier(SyncSender<()>),
    /// Solo test: rallenta il thread / simula una divergenza.
    #[doc(hidden)]
    Sleep(Duration),
    #[doc(hidden)]
    Tamper(solana_address::Address),
    /// Solo test: pausa dopo ogni account scritto, e limite della coda durante la scrittura.
    #[doc(hidden)]
    SlowWrite(Duration),
    #[doc(hidden)]
    WriteQueueLimit(usize),
}

#[derive(Default)]
pub struct Stats {
    /// Snapshot saltati: scrittore occupato, ombra persa o in errore.
    pub skipped: AtomicU64,
    pub written: AtomicU64,
    /// Durata dell'ultima scrittura (ms) e coda più lunga raccolta durante una scrittura.
    pub last_write_ms: AtomicU64,
    pub max_queue: AtomicU64,
    /// Snapshot scritti: (altezza, ms unix di inizio e del rename). Solo misura.
    pub windows: Mutex<Vec<(u64, u64, u64)>>,
    /// Primo errore (divergenza, scrittura): l'ombra non scrive più fino al riavvio.
    pub error: Mutex<Option<String>>,
}

impl Stats {
    fn fail(&self, e: String) {
        eprintln!("snapshot: {e}");
        self.error.lock().unwrap().get_or_insert(e);
    }
}

pub struct Shadow {
    tx: SyncSender<Msg>,
    /// Canale pieno almeno una volta: l'ombra ha perso un delta e non è più valida.
    lost: bool,
    pub stats: Arc<Stats>,
    handle: Option<JoinHandle<()>>,
}

impl Shadow {
    /// `accounts` e `status` (contenuto di `status.bin`): stato del nodo a `height`,
    /// copiato una volta, all'avvio. `window`: blocchi della finestra degli stati.
    #[allow(clippy::too_many_arguments)]
    pub fn start(
        dir: PathBuf,
        height: u64,
        state_hash: Hash,
        accounts: std::collections::HashMap<solana_address::Address, solana_account::AccountSharedData>,
        rules: crate::exec::Rules,
        status: Vec<u8>,
        window: u64,
        mb_s: u64,
    ) -> Result<Shadow, String> {
        let (tx, rx) = std::sync::mpsc::sync_channel(QUEUE_BLOCKS);
        let stats = Arc::new(Stats::default());
        let st = stats.clone();
        let handle = std::thread::Builder::new()
            .name("centrifuga-shadow".into())
            .spawn(move || {
                let mut index = TxIndex::new(window, 0, 0);
                let rep = index
                    .decode(&status, height)
                    .map_err(|e| format!("status: {e}"))
                    .and_then(|()| Replica::from_state(height, state_hash, accounts, rules));
                run(dir, rep, index, rx, st, mb_s)
            })
            .map_err(|e| format!("shadow thread: {e}"))?;
        Ok(Shadow { tx, lost: false, stats, handle: Some(handle) })
    }

    /// Mai bloccante: con il canale pieno l'ombra si considera persa.
    pub fn send(&mut self, m: Msg) {
        if self.lost {
            if matches!(m, Msg::Snapshot { .. }) {
                self.stats.skipped.fetch_add(1, Ordering::Relaxed);
            }
            return;
        }
        let snapshot = matches!(m, Msg::Snapshot { .. });
        match self.tx.try_send(m) {
            Ok(()) => {}
            Err(TrySendError::Full(_)) | Err(TrySendError::Disconnected(_)) => {
                self.lost = true;
                if snapshot {
                    self.stats.skipped.fetch_add(1, Ordering::Relaxed);
                }
                self.stats.fail(format!("shadow state lost: more than {QUEUE_BLOCKS} blocks behind; snapshots skipped until restart"));
            }
        }
    }

    pub fn lost(&self) -> bool {
        self.lost
    }

    /// Attende la scrittura in corso (bloccante: solo test e chiusura) e restituisce il
    /// primo errore, se c'è.
    pub fn wait(&mut self) -> Result<(), String> {
        let (atx, arx) = std::sync::mpsc::sync_channel(1);
        if self.tx.send(Msg::Barrier(atx)).is_ok() {
            let _ = arx.recv();
        }
        match self.stats.error.lock().unwrap().clone() {
            Some(e) => Err(e),
            None => Ok(()),
        }
    }

    /// Solo test: messaggi diretti al thread (bloccanti).
    #[doc(hidden)]
    pub fn send_blocking(&self, m: Msg) {
        let _ = self.tx.send(m);
    }
}

impl Drop for Shadow {
    fn drop(&mut self) {
        let _ = self.wait();
        let (tx, _) = std::sync::mpsc::sync_channel(1);
        drop(std::mem::replace(&mut self.tx, tx)); // chiude il canale: il thread finisce
        if let Some(h) = self.handle.take() {
            let _ = h.join();
        }
    }
}

/// Priorità bassa (nice 10) al thread corrente, senza dipendenze: `renice` sul tid.
fn lower_priority() {
    let tid = std::fs::read_link("/proc/thread-self").ok().and_then(|p| p.file_name().map(|f| f.to_string_lossy().into_owned()));
    if let Some(tid) = tid {
        let ok = std::process::Command::new("renice").args(["-n", "10", "-p", &tid]).stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null()).status();
        if !ok.is_ok_and(|s| s.success()) {
            eprintln!("shadow: could not lower the thread priority (renice)");
        }
    }
}

struct State {
    dir: PathBuf,
    rep: Option<Replica>,
    index: TxIndex,
    st: Arc<Stats>,
    slow: Duration,
    queue_limit: usize,
    /// Banda massima della scrittura (MB/s, 0 = nessun limite).
    mb_s: u64,
}

impl State {
    /// Un messaggio fuori dalla scrittura.
    fn handle(&mut self, m: Msg, rx: &Receiver<Msg>) {
        match m {
            Msg::Block { height, state_hash, delta, statuses } => {
                self.index.record_statuses(height, statuses);
                if let Some(r) = self.rep.as_mut() {
                    if let Err(e) = r.apply_delta(height, state_hash, &delta) {
                        self.st.fail(format!("shadow state diverged at {height}: {e}"));
                        self.rep = None;
                    }
                }
            }
            Msg::Snapshot { height, genesis_hash, state_hash } => self.snapshot(height, genesis_hash, state_hash, rx),
            Msg::Barrier(ack) => drop(ack.send(())),
            Msg::Sleep(d) => std::thread::sleep(d),
            Msg::Tamper(k) => {
                if let Some(r) = self.rep.as_mut() {
                    r.tamper(&k);
                }
            }
            Msg::SlowWrite(d) => self.slow = d,
            Msg::WriteQueueLimit(n) => self.queue_limit = n,
        }
    }

    /// Scrive lo snapshot dalla mappa ordinata, raccogliendo intanto i messaggi; poi li
    /// applica in ordine. Coda piena: snapshot interrotto (saltato), l'ombra recupera.
    fn snapshot(&mut self, height: u64, genesis_hash: Hash, state_hash: Hash, rx: &Receiver<Msg>) {
        let skip = |st: &Stats| st.skipped.fetch_add(1, Ordering::Relaxed);
        let Some(r) = self.rep.as_ref() else {
            skip(&self.st);
            return;
        };
        if r.height != height {
            self.st.fail(format!("shadow state at {} instead of {height}", r.height));
            self.rep = None;
            return;
        }
        let (started, start_ms) = (std::time::Instant::now(), crate::node::now_ms());
        let mut queue: std::collections::VecDeque<Msg> = Default::default();
        let mut blocks = 0usize;
        let status = self.index.encode(height);
        let tmp = self.dir.join("snapshot.tmp");
        let result = (|| -> Result<bool, String> {
            let e = |e: std::io::Error| format!("snapshot {height}: {e}");
            let mut w = crate::ledger::SnapshotWriter::create(&tmp, height, genesis_hash, state_hash, r.accounts().len() as u64).map_err(e)?.with_rate(self.mb_s);
            let mut lt = crate::exec::state_hash::LtState::empty();
            for (i, (k, a)) in r.accounts().iter().enumerate() {
                w.push(k, a).map_err(e)?;
                lt.replace(k, None, Some(a));
                if !self.slow.is_zero() {
                    std::thread::sleep(self.slow);
                }
                if i % 1024 == 0 || !self.slow.is_zero() {
                    while let Ok(m) = rx.try_recv() {
                        match m {
                            Msg::Snapshot { height: h, .. } => {
                                skip(&self.st);
                                eprintln!("snapshot {h} skipped: the previous one is still being written");
                            }
                            m => {
                                blocks += matches!(m, Msg::Block { .. }) as usize;
                                queue.push_back(m);
                            }
                        }
                    }
                    self.st.max_queue.fetch_max(blocks as u64, Ordering::Relaxed);
                    if blocks > self.queue_limit {
                        return Ok(false); // interrotto: troppi blocchi in coda
                    }
                }
            }
            w.finish().map_err(e)?;
            // Prima di rinominare: LtHash di ciò che è stato scritto = hash di stato del nodo.
            if lt.hash() != state_hash {
                return Err(format!("snapshot {height} not written: shadow state hash {} differs from the node's {state_hash}", lt.hash()));
            }
            crate::node::finish_snapshot(&self.dir, height, &status)?;
            Ok(true)
        })();
        match result {
            Ok(true) => {
                self.st.written.fetch_add(1, Ordering::Relaxed);
                self.st.last_write_ms.store(started.elapsed().as_millis() as u64, Ordering::Relaxed);
                self.st.windows.lock().unwrap().push((height, start_ms, crate::node::now_ms()));
            }
            Ok(false) => {
                let _ = std::fs::remove_file(&tmp);
                skip(&self.st);
                eprintln!("snapshot {height} interrupted: more than {} blocks queued during the write", self.queue_limit);
            }
            Err(e) => {
                let _ = std::fs::remove_file(&tmp);
                self.st.fail(e);
            }
        }
        for m in queue {
            self.handle(m, rx);
        }
    }
}

fn run(dir: PathBuf, rep: Result<Replica, String>, index: TxIndex, rx: Receiver<Msg>, st: Arc<Stats>, mb_s: u64) {
    lower_priority();
    let rep = match rep {
        Ok(r) => Some(r),
        Err(e) => {
            st.fail(format!("shadow state: {e}"));
            None
        }
    };
    let mut state = State { dir, rep, index, st, slow: Duration::ZERO, queue_limit: QUEUE_BLOCKS, mb_s };
    while let Ok(m) = rx.recv() {
        state.handle(m, &rx);
    }
}
