//! Stato ombra per lo snapshot in background. A ogni blocco il nodo manda il delta
//! dello stream (gli account toccati, `None` per quelli chiusi) su un canale limitato;
//! questo thread lo applica a una `stream::Replica`, che verifica l'LtHash a ogni blocco.
//! All'altezza dello snapshot copia la propria mappa (qui, non sul thread del nodo) e la
//! passa allo scrittore, che ricalcola l'LtHash completo della copia: se non coincide con
//! quello del nodo non scrive e segnala l'errore. Il nodo non aspetta mai: se il canale è
//! pieno l'ombra è persa, gli snapshot successivi si saltano (contati) fino al riavvio.

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
}

#[derive(Default)]
pub struct Stats {
    /// Snapshot saltati: scrittore occupato, ombra persa o in errore.
    pub skipped: AtomicU64,
    pub written: AtomicU64,
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
                run(dir, rep, index, rx, st)
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

fn run(dir: PathBuf, rep: Result<Replica, String>, mut index: TxIndex, rx: Receiver<Msg>, st: Arc<Stats>) {
    let mut rep = match rep {
        Ok(r) => Some(r),
        Err(e) => {
            st.fail(format!("shadow state: {e}"));
            None
        }
    };
    let mut writer: Option<JoinHandle<Result<(), String>>> = None;
    let finish = |w: &mut Option<JoinHandle<Result<(), String>>>, st: &Stats| {
        if let Some(h) = w.take() {
            match h.join() {
                Ok(Ok(())) => {
                    st.written.fetch_add(1, Ordering::Relaxed);
                }
                Ok(Err(e)) => st.fail(e),
                Err(_) => st.fail("snapshot writer panicked".into()),
            }
        }
    };
    for m in rx {
        match m {
            Msg::Block { height, state_hash, delta, statuses } => {
                index.record_statuses(height, statuses);
                if let Some(r) = rep.as_mut() {
                    if let Err(e) = r.apply_delta(height, state_hash, &delta) {
                        st.fail(format!("shadow state diverged at {height}: {e}"));
                        rep = None;
                    }
                }
            }
            Msg::Snapshot { height, genesis_hash, state_hash } => {
                let Some(r) = rep.as_ref() else {
                    st.skipped.fetch_add(1, Ordering::Relaxed);
                    continue;
                };
                if writer.as_ref().is_some_and(|w| !w.is_finished()) {
                    st.skipped.fetch_add(1, Ordering::Relaxed);
                    eprintln!("snapshot {height} skipped: the previous one is still being written");
                    continue;
                }
                finish(&mut writer, &st);
                if r.height != height {
                    st.fail(format!("shadow state at {} instead of {height}", r.height));
                    rep = None;
                    continue;
                }
                let copy = r.accounts().clone();
                let status = index.encode(height);
                let dir = dir.clone();
                let spawned = std::thread::Builder::new().name("centrifuga-snapshot".into()).spawn(move || {
                    // Prima di scrivere: LtHash completo della copia = hash di stato del nodo.
                    let full = crate::exec::state_hash::LtState::full(&copy).hash();
                    if full != state_hash {
                        return Err(format!("snapshot {height} not written: shadow state hash {full} differs from the node's {state_hash}"));
                    }
                    let accounts: Vec<_> = copy.into_iter().collect();
                    crate::node::write_snapshot_and_status(&dir, height, genesis_hash, state_hash, accounts, &status)
                });
                match spawned {
                    Ok(h) => writer = Some(h),
                    Err(e) => st.fail(format!("snapshot thread: {e}")),
                }
            }
            Msg::Barrier(ack) => {
                finish(&mut writer, &st);
                let _ = ack.send(());
            }
            Msg::Sleep(d) => std::thread::sleep(d),
            Msg::Tamper(k) => {
                if let Some(r) = rep.as_mut() {
                    r.tamper(&k);
                }
            }
        }
    }
    finish(&mut writer, &st);
}
