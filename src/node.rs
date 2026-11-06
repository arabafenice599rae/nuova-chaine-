//! Nodo: apertura (snapshot + journal), produzione dei blocchi, snapshot periodici.

use std::collections::{HashMap, VecDeque};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::mpsc::{Receiver, RecvTimeoutError};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use solana_keypair::Keypair;
use solana_signer::Signer;

use crate::exec::litesvm_internal::Svm;
use crate::exec::{Chain, Executed};
use crate::genesis::{self, Config};
use crate::ingress::{Ingress, Limits};
use solana_address::Address;
use crate::ledger::{self, Journal};
use crate::rpc::index::TxIndex;

/// Richiesta da eseguire sul thread del nodo (RPC, WebSocket).
pub type Query = Box<dyn FnOnce(&mut Node) + Send>;

pub struct Node {
    pub chain: Chain,
    pub ingress: Arc<Ingress>,
    pub cfg: Config,
    pub index: TxIndex,
    /// Ultimi blocchi per il recupero dello stream: (altezza, offset nel journal, account cambiati).
    /// Limitati da `stream_catchup_blocks` e `stream_catchup_bytes`: vale il primo raggiunto.
    pub recent: VecDeque<(u64, u64, Vec<Address>)>,
    /// Byte stimati di `recent` (indirizzi più l'intestazione di ogni voce).
    pub recent_bytes: usize,
    /// Istante di arrivo delle richieste sendTransaction, per misurare il tempo nel nodo.
    pub arrivals: HashMap<solana_signature::Signature, Instant>,
    /// Ora dell'ultimo blocco prodotto da questo processo (None dopo l'avvio).
    last_tick_ms: Option<u64>,
    /// Offset nel journal dell'ultimo blocco (None all'altezza 0).
    pub last_offset: Option<u64>,
    /// Durata dell'ultimo tick (µs): misura per i test di carico e il monitoraggio.
    pub last_tick_us: u64,
    /// (altezza, µs di sync_data del journal) di ogni blocco: solo misura.
    pub sync_log: Vec<(u64, u32)>,
    /// Pausa dell'ultimo snapshot nel suo tick (µs: codifica degli stati e invio allo
    /// stato ombra; copia e scrittura sono su altri thread): solo misura.
    pub last_snapshot_us: u64,
    /// Stato ombra per lo snapshot in background (None con `snapshot_every = 0`).
    shadow: Option<crate::shadow::Shadow>,
    /// Blocchi oltre 100 ms (altezza, µs, fasi): solo misura, al massimo 64.
    pub slow_blocks: Vec<(u64, u64, crate::exec::Phases)>,
    /// Altezza dell'ultimo delta mandato all'ombra.
    shadow_at: u64,
    /// Delta dell'ultimo blocco (lo stesso per lo stream e per l'ombra).
    pub last_delta: Option<(u64, Arc<crate::stream::Accounts>)>,
    journal: Journal,
    dir: PathBuf,
    keypair: Keypair,
}

impl Node {
    /// Carica lo snapshot (se c'è) e riesegue il journal fino in fondo.
    pub fn open(cfg: Config, keypair: Keypair) -> Result<Node, String> {
        let params = cfg.genesis.params()?;
        if keypair.pubkey() != params.operator {
            return Err("operator key does not match the genesis".into());
        }
        let dir = cfg.node.data_dir.clone();
        std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        let (mut chain, snap_height) = chain_from_snapshot(&cfg, &dir)?;
        // Finestre in RAM dimensionate subito (nessun ridimensionamento): firme della
        // finestra anti-duplicati e stati delle tx degli stessi blocchi.
        let window = chain.params.blockhash_window as usize * cfg.node.max_block_txs;
        chain.reserve_signatures(window);
        let mut index = TxIndex::new(chain.params.blockhash_window as u64, cfg.node.rpc_recent_txs, window);
        index.load(&dir, snap_height).map_err(|e| format!("status.bin: {e}"))?;
        let (mut recent, mut recent_bytes) = (VecDeque::new(), 0);
        let limits = (cfg.node.stream_catchup_blocks, cfg.node.stream_catchup_bytes);
        let mut last_offset = None;
        let journal = Journal::open(&dir.join("journal.bin"), |b, offset| {
            last_offset = Some(offset);
            if b.header.height <= snap_height {
                if b.header.height == snap_height {
                    let h = chain.state_hash();
                    if h != b.header.state_hash {
                        return Err(format!("snapshot {snap_height}: state hash differs from the journal"));
                    }
                }
                chain.note_block(&b)
            } else {
                let ex = chain.apply(&b)?;
                index.record(&ex, |_| None);
                remember(&mut recent, &mut recent_bytes, limits, b.header.height, offset, &ex);
                Ok(())
            }
        })?;
        if chain.height < snap_height {
            return Err("journal shorter than the snapshot".into());
        }
        let limits = Limits { tps_per_ip: cfg.node.ingress_tps_per_ip, pending_per_payer: cfg.node.ingress_pending_per_payer };
        let mut n = Node { chain, ingress: Arc::new(Ingress::new(limits)), cfg, index, recent, recent_bytes, arrivals: Default::default(), last_tick_ms: None, last_offset, last_tick_us: 0, sync_log: vec![], last_snapshot_us: 0, shadow: None, slow_blocks: vec![], shadow_at: 0, last_delta: None, journal, dir, keypair };
        if n.cfg.node.snapshot_every > 0 {
            n.start_shadow()?;
        }
        Ok(n)
    }

    /// Salto di altezza: slot trascorsi dall'ultimo blocco di questo processo, tra 1 e
    /// heartbeat_slots. Dopo un avvio è 1: nessun recupero del tempo di fermo.
    pub fn gap(&self, now_ms: u64) -> u64 {
        let r = self.chain.params.rules;
        // Arrotondato allo slot più vicino: i blocchi partono ai confini di slot, con qualche ms di scarto.
        self.last_tick_ms.map_or(1, |t| ((now_ms.saturating_sub(t) + r.slot_ms / 2) / r.slot_ms).clamp(1, r.heartbeat_slots))
    }

    /// Produce il blocco successivo: preleva in ordine FIFO, esegue, firma, scrive nel journal.
    pub fn tick(&mut self, now_ms: u64) -> Result<Executed, String> {
        let started = Instant::now();
        let txs = self.ingress.drain(self.cfg.node.max_block_txs);
        let height = self.chain.height + self.gap(now_ms);
        self.last_tick_ms = Some(now_ms);
        let mut ex = self.chain.produce_at(height, txs, now_ms, &self.keypair);
        let t = Instant::now();
        let offset = self.journal.append(&ex.block).map_err(|e| format!("journal: {e}"))?;
        if self.sync_log.len() < 1_000_000 {
            self.sync_log.push((ex.block.header.height, self.journal.last_sync.as_micros() as u32));
        }
        ex.phases.journal = t.elapsed();
        let arrivals = &mut self.arrivals;
        let us: HashMap<_, _> = ex.block.txs.iter().filter_map(|t| arrivals.remove(&t.signatures[0]).map(|a| (t.signatures[0], a.elapsed().as_micros() as u64))).collect();
        self.index.record(&ex, |s| us.get(s).copied());
        if self.arrivals.len() > 100_000 {
            self.arrivals.retain(|_, t| t.elapsed() < Duration::from_secs(120)); // tx mai incluse
        }
        self.last_offset = Some(offset);
        let limits = (self.cfg.node.stream_catchup_blocks, self.cfg.node.stream_catchup_bytes);
        remember(&mut self.recent, &mut self.recent_bytes, limits, ex.block.header.height, offset, &ex);
        // Delta del blocco: per lo stream e per lo stato ombra (mai bloccante).
        let delta = Arc::new(crate::stream::block_delta(self, &ex));
        self.last_delta = Some((ex.block.header.height, delta.clone()));
        if let Some(s) = self.shadow.as_mut() {
            let statuses = TxIndex::statuses_of(&ex);
            s.send(crate::shadow::Msg::Block { height: ex.block.header.height, state_hash: ex.block.header.state_hash, delta, statuses });
            self.shadow_at = ex.block.header.height;
        }
        let every = self.cfg.node.snapshot_every;
        if every > 0 && ex.block.header.height % every == 0 {
            let t = Instant::now();
            self.snapshot()?;
            self.last_snapshot_us = t.elapsed().as_micros() as u64;
        }
        let p = &mut ex.phases;
        p.other = started.elapsed().saturating_sub(p.exec + p.hash + p.sign + p.journal);
        self.last_tick_us = started.elapsed().as_micros() as u64;
        if self.last_tick_us > 100_000 && self.slow_blocks.len() < 64 {
            self.slow_blocks.push((ex.block.header.height, self.last_tick_us, ex.phases));
        }
        Ok(ex)
    }

    /// Ciclo principale: a ogni slot un blocco se ci sono tx in coda, oppure se
    /// da `heartbeat_slots` slot non ne esce uno; tra uno slot e l'altro, le query.
    pub fn serve(mut self, queries: Receiver<Query>, mut on_block: impl FnMut(&Node, &Executed)) -> Result<(), String> {
        let slot = Duration::from_millis(self.cfg.genesis.slot_ms);
        let heartbeat = slot * self.cfg.genesis.heartbeat_slots.max(1) as u32;
        let mut next = Instant::now() + slot;
        // Primo blocco al primo slot: dopo un avvio o un riavvio l'altezza è subito quella reale.
        let mut last_block = Instant::now().checked_sub(heartbeat).unwrap_or_else(Instant::now);
        loop {
            let now = Instant::now();
            if now >= next {
                next = (next + slot).max(now);
                if self.ingress.is_empty() && now.duration_since(last_block) + slot / 2 < heartbeat {
                    continue; // slot vuoto: nessun blocco
                }
                let ex = self.tick(crate::node::now_ms())?;
                last_block = now;
                on_block(&self, &ex);
                // Anche se la produzione non sta nello slot, le query non restano mai ferme.
                if let Ok(q) = queries.try_recv() {
                    q(&mut self);
                }
                continue;
            }
            match queries.recv_timeout(next - now) {
                Ok(q) => q(&mut self),
                Err(RecvTimeoutError::Timeout) => {}
                Err(RecvTimeoutError::Disconnected) => std::thread::sleep(next - now),
            }
        }
    }

    pub fn journal_path(&self) -> PathBuf {
        self.dir.join("journal.bin")
    }

    /// Stato ombra dallo stato attuale: una copia della mappa, all'avvio (o se lo stato
    /// è cambiato fuori da `tick`, come nei test che producono blocchi a mano).
    fn start_shadow(&mut self) -> Result<(), String> {
        self.shadow = None; // il precedente finisce la scrittura in corso
        let c = &self.chain;
        let accounts = c.svm.accounts().clone();
        let status = self.index.encode(c.height);
        let window = c.params.blockhash_window as u64;
        self.shadow = Some(crate::shadow::Shadow::start(self.dir.clone(), c.height, c.state_hash(), accounts, c.params.rules, status, window)?);
        self.shadow_at = c.height;
        Ok(())
    }

    /// Snapshot all'altezza corrente, scritto in background dallo stato ombra (account e
    /// stati della finestra): sul thread del nodo resta un invio non bloccante.
    /// Se lo scrittore è occupato o l'ombra è persa, lo snapshot si salta (contato). Il
    /// file vale solo dopo il rename: un arresto a metà lascia valido il precedente.
    /// Senza ombra (`snapshot_every = 0`) lo snapshot si scrive qui, in modo sincrono.
    pub fn snapshot(&mut self) -> Result<(), String> {
        let c = &self.chain;
        let (height, gh, sh) = (c.height, c.genesis_hash, c.state_hash());
        if self.shadow.is_none() {
            let accounts = c.svm.accounts().iter().map(|(k, a)| (*k, a.clone())).collect();
            return write_snapshot_and_status(&self.dir, height, gh, sh, accounts, &self.index.encode(height));
        }
        if self.shadow_at != height {
            self.start_shadow()?;
        }
        let shadow = self.shadow.as_mut().unwrap();
        shadow.send(crate::shadow::Msg::Snapshot { height, genesis_hash: gh, state_hash: sh });
        Ok(())
    }

    /// Aspetta la scrittura dello snapshot in corso; restituisce il primo errore dello
    /// stato ombra (divergenza, scrittura), se c'è.
    pub fn wait_snapshot(&mut self) -> Result<(), String> {
        self.shadow.as_mut().map_or(Ok(()), |s| s.wait())
    }

    /// Snapshot saltati (scrittore occupato, ombra persa o in errore) e scritti.
    pub fn snapshot_counts(&self) -> (u64, u64) {
        use std::sync::atomic::Ordering::Relaxed;
        self.shadow.as_ref().map_or((0, 0), |s| (s.stats.skipped.load(Relaxed), s.stats.written.load(Relaxed)))
    }

    /// Durata dell'ultima scrittura (ms) e coda più lunga (blocchi) durante una scrittura.
    pub fn snapshot_write_stats(&self) -> (u64, u64) {
        use std::sync::atomic::Ordering::Relaxed;
        self.shadow.as_ref().map_or((0, 0), |s| (s.stats.last_write_ms.load(Relaxed), s.stats.max_queue.load(Relaxed)))
    }

    /// Solo test: accesso diretto al thread dell'ombra.
    #[doc(hidden)]
    pub fn shadow_for_test(&mut self) -> Option<&mut crate::shadow::Shadow> {
        self.shadow.as_mut()
    }
}

impl Drop for Node {
    /// Chi chiude il nodo trova su disco lo snapshot completo (o il precedente).
    fn drop(&mut self) {
        if let Err(e) = self.wait_snapshot() {
            eprintln!("{e}");
        }
    }
}

/// Ordina, scrive `snapshot.tmp` e `status.tmp` con fsync, poi li rinomina (prima lo
/// snapshot, poi gli stati della stessa altezza).
pub fn write_snapshot_and_status(
    dir: &std::path::Path,
    height: u64,
    genesis_hash: solana_hash::Hash,
    state_hash: solana_hash::Hash,
    mut accounts: Vec<(Address, solana_account::AccountSharedData)>,
    status: &[u8],
) -> Result<(), String> {
    accounts.sort_unstable_by_key(|(k, _)| *k);
    let tmp = dir.join("snapshot.tmp");
    ledger::write_snapshot_file(&tmp, height, genesis_hash, state_hash, accounts.iter().map(|(k, a)| (k, a))).map_err(|e| format!("snapshot {height}: {e}"))?;
    finish_snapshot(dir, height, status)
}

/// Dopo `snapshot.tmp` scritto con fsync: `status.tmp` con fsync, poi i due rename
/// (prima lo snapshot, poi gli stati della stessa altezza) e fsync della cartella.
pub fn finish_snapshot(dir: &std::path::Path, height: u64, status: &[u8]) -> Result<(), String> {
    use std::io::Write;
    let e = |e: std::io::Error| format!("snapshot {height}: {e}");
    let tmp = dir.join("snapshot.tmp");
    let stmp = dir.join("status.tmp");
    let mut f = std::fs::File::create(&stmp).map_err(e)?;
    f.write_all(status).map_err(e)?;
    f.sync_all().map_err(e)?;
    std::fs::rename(&tmp, ledger::snapshot_path(dir)).map_err(e)?;
    std::fs::rename(&stmp, dir.join("status.bin")).map_err(e)?;
    std::fs::File::open(dir).and_then(|d| d.sync_all()).map_err(e)
}

/// Nodo avviato: thread del nodo + canali per RPC e stream.
pub struct Running {
    pub gateway: crate::rpc::Gateway,
    pub stream: crate::stream::Stream,
    pub cfg: Config,
}

/// Stato da `dir/snapshot.bin` (o dal genesis se manca) e altezza dello snapshot: la
/// finestra dei blockhash si riempie poi con il journal fino a quell'altezza.
pub fn chain_from_snapshot(cfg: &Config, dir: &std::path::Path) -> Result<(Chain, u64), String> {
    let params = cfg.genesis.params()?;
    let Some(s) = ledger::read_snapshot(dir).map_err(|e| e.to_string())? else {
        return Ok((genesis::build(&cfg.genesis)?.0, 0));
    };
    // Lo snapshot deve appartenere a questo genesis (parametri inclusi nel suo hash).
    let expected = genesis::build(&cfg.genesis)?.0.genesis_hash;
    if s.genesis_hash != expected {
        return Err(format!("snapshot of another genesis ({} instead of {expected})", s.genesis_hash));
    }
    let (height, stated) = (s.height, s.state_hash);
    let svm = Svm::restore(s.accounts, params.lamports_per_signature, s.genesis_hash)?;
    let chain = Chain::new(svm, params, 0, s.genesis_hash, cfg.genesis.timestamp_ms, s.genesis_hash);
    // Lo snapshot porta l'hash di stato della sua altezza: ricalcolato, deve coincidere.
    if chain.state_hash() != stated {
        return Err(format!("snapshot {height}: state hash {} differs from its header {stated}", chain.state_hash()));
    }
    Ok((chain, height))
}

/// Apre il nodo su un thread dedicato (litesvm vive lì) e restituisce i canali.
pub fn start(cfg: Config, keypair: Keypair) -> Result<Running, String> {
    use crate::rpc::{BlockEvent, Gateway};
    let (qtx, qrx) = std::sync::mpsc::channel::<Query>();
    let (events, _) = tokio::sync::broadcast::channel(1024);
    let (blocks, _) = tokio::sync::broadcast::channel(cfg.node.stream_max_lag_blocks.max(1));
    let running_indexer = cfg.node.indexer_rpc.clone();
    let log = if cfg.node.rpc_log {
        std::fs::create_dir_all(&cfg.node.data_dir).map_err(|e| e.to_string())?;
        let path = cfg.node.data_dir.join("rpc.log");
        let f = std::fs::OpenOptions::new().create(true).append(true).open(&path).map_err(|e| format!("{}: {e}", path.display()))?;
        Some(Arc::new(std::sync::Mutex::new(f)))
    } else {
        None
    };
    let running = Running {
        gateway: Gateway { queries: qtx.clone(), events: events.clone(), log, indexer: running_indexer },
        stream: crate::stream::Stream {
            queries: qtx,
            blocks: blocks.clone(),
            max_wait: Duration::from_millis(cfg.genesis.slot_ms * cfg.node.stream_max_lag_blocks as u64),
        },
        cfg: cfg.clone(),
    };
    let (ready_tx, ready_rx) = std::sync::mpsc::channel();
    // Nome del thread: visibile in top -H e nelle misure di CPU per thread.
    let spawned = std::thread::Builder::new().name("centrifuga-node".into()).spawn(move || {
        let sym = cfg.genesis.symbol.clone();
        let node = match Node::open(cfg, keypair) {
            Ok(n) => n,
            Err(e) => return drop(ready_tx.send(Err(e))),
        };
        println!("{} ({}): height {}, hash {}", node.cfg.genesis.chain_name, node.cfg.genesis.chain_id, node.chain.height, node.chain.last_hash);
        let _ = ready_tx.send(Ok(()));
        let res = node.serve(qrx, |n, ex| {
            let _ = events.send(Arc::new(BlockEvent::new(ex)));
            // Il messaggio dello stream si costruisce solo se qualcuno ascolta.
            if blocks.receiver_count() > 0 {
                let _ = blocks.send(Arc::new((ex.block.header.height, crate::stream::block_message(n, ex))));
            }
            if !ex.block.txs.is_empty() {
                let s = &ex.settlement;
                println!(
                    "blocco {}: {} tx, fee base {}, operatore {}, riserva {}, premio {}, rimborso priority {}",
                    ex.block.header.height, ex.block.txs.len(), fmt_amount(s.base, &sym), fmt_amount(s.operator, &sym),
                    fmt_amount(s.reserve, &sym), fmt_amount(s.reward, &sym), fmt_amount(s.priority_refund, &sym),
                );
            }
        });
        if let Err(e) = res {
            eprintln!("node error: {e}");
            std::process::exit(1);
        }
    });
    spawned.map_err(|e| format!("node thread: {e}"))?;
    ready_rx.recv().map_err(|e| e.to_string())??;
    Ok(running)
}

impl Running {
    /// RPC (HTTP + WebSocket) e stream, finché uno dei due non fallisce.
    pub async fn serve_network(self) -> Result<(), String> {
        let rpc_addr = self.cfg.node.rpc_addr.parse().map_err(|e| format!("rpc_addr: {e}"))?;
        let stream_addr = self.cfg.node.stream_addr.parse().map_err(|e| format!("stream_addr: {e}"))?;
        tokio::try_join!(crate::rpc::serve(self.gateway, rpc_addr), crate::stream::serve(self.stream, stream_addr))
            .map_err(|e| e.to_string())?;
        Ok(())
    }
}

/// Byte di una voce di `recent`: gli indirizzi più l'intestazione (altezza, offset, Vec).
pub fn recent_entry_bytes(changed: usize) -> usize {
    changed * 32 + 48
}

/// Aggiunge un blocco alla finestra del recupero e toglie i più vecchi finché valgono
/// entrambi i limiti (blocchi e byte): vale il primo raggiunto.
fn remember(recent: &mut VecDeque<(u64, u64, Vec<Address>)>, bytes: &mut usize, (max_blocks, max_bytes): (usize, usize), height: u64, offset: u64, ex: &Executed) {
    let changed: Vec<Address> = ex.changed.iter().copied().collect();
    *bytes += recent_entry_bytes(changed.len());
    recent.push_back((height, offset, changed));
    while recent.len() > max_blocks || (*bytes > max_bytes && !recent.is_empty()) {
        let (_, _, old) = recent.pop_front().unwrap();
        *bytes -= recent_entry_bytes(old.len());
    }
}

/// Importo in unità del token: 12500000000 -> "12.5 CnFA".
pub fn fmt_amount(lamports: u64, symbol: &str) -> String {
    let (int, frac) = (lamports / genesis::LAMPORTS_PER_CNFA, lamports % genesis::LAMPORTS_PER_CNFA);
    if frac == 0 {
        return format!("{int} {symbol}");
    }
    let f = format!("{frac:09}");
    format!("{int}.{} {symbol}", f.trim_end_matches('0'))
}

/// Legge una chiave nel formato JSON di solana-keygen (array di 64 byte).
pub fn read_keypair(path: &std::path::Path) -> Result<Keypair, String> {
    let s = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let bytes: Result<Vec<u8>, _> = s.trim().trim_matches(['[', ']']).split(',').map(|x| x.trim().parse::<u8>()).collect();
    let bytes = bytes.map_err(|e| format!("{}: {e}", path.display()))?;
    Keypair::try_from(bytes.as_slice()).map_err(|e| format!("{}: {e}", path.display()))
}

pub fn now_ms() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_millis() as u64
}
