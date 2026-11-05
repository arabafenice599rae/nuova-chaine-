//! Nodo: apertura (snapshot + journal), produzione dei blocchi, snapshot periodici.

use std::collections::VecDeque;
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
    pub recent: VecDeque<(u64, u64, Vec<Address>)>,
    /// Istante di arrivo delle richieste sendTransaction, per misurare il tempo nel nodo.
    pub arrivals: std::collections::HashMap<solana_signature::Signature, Instant>,
    /// Offset nel journal dell'ultimo blocco (None all'altezza 0).
    pub last_offset: Option<u64>,
    journal: Journal,
    dir: PathBuf,
    keypair: Keypair,
}

impl Node {
    /// Carica lo snapshot (se c'è) e riesegue il journal fino in fondo.
    pub fn open(cfg: Config, keypair: Keypair) -> Result<Node, String> {
        let params = cfg.genesis.params()?;
        if keypair.pubkey() != params.operator {
            return Err("la chiave dell'operatore non corrisponde al genesis".into());
        }
        let dir = cfg.node.data_dir.clone();
        std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        let snap = ledger::read_snapshot(&dir).map_err(|e| e.to_string())?;
        let snap_height = snap.as_ref().map_or(0, |s| s.height);
        let mut chain = match snap {
            None => genesis::build(&cfg.genesis)?.0,
            Some(s) => {
                let svm = Svm::restore(s.accounts, params.lamports_per_signature, s.genesis_hash)?;
                // La finestra parte dal genesis; il journal fino a `snap_height` la riempie.
                Chain::new(svm, params, 0, s.genesis_hash, cfg.genesis.timestamp_ms, s.genesis_hash)
            }
        };
        let mut index = TxIndex::default();
        let mut recent = VecDeque::new();
        let window = cfg.node.stream_catchup_blocks;
        let mut last_offset = None;
        let journal = Journal::open(&dir.join("journal.bin"), |b, offset| {
            last_offset = Some(offset);
            if b.header.height <= snap_height {
                if b.header.height == snap_height {
                    let h = chain.state_hash(&Default::default());
                    if h != b.header.state_hash {
                        return Err(format!("snapshot {snap_height}: hash di stato diverso dal journal"));
                    }
                }
                chain.note_block(&b);
                Ok(())
            } else {
                let ex = chain.apply(&b)?;
                index.record(&ex);
                remember(&mut recent, window, b.header.height, offset, &ex);
                Ok(())
            }
        })?;
        if chain.height < snap_height {
            return Err("journal più corto dello snapshot".into());
        }
        let limits = Limits { tps_per_ip: cfg.node.ingress_tps_per_ip, pending_per_payer: cfg.node.ingress_pending_per_payer };
        Ok(Node { chain, ingress: Arc::new(Ingress::new(limits)), cfg, index, recent, arrivals: Default::default(), last_offset, journal, dir, keypair })
    }

    /// Un ciclo di slot: preleva in ordine FIFO, esegue, firma, scrive nel journal.
    pub fn tick(&mut self, now_ms: u64) -> Result<Executed, String> {
        let txs = self.ingress.drain(self.cfg.node.max_block_txs);
        let ex = self.chain.produce(txs, now_ms, &self.keypair);
        let offset = self.journal.append(&ex.block).map_err(|e| format!("journal: {e}"))?;
        self.index.record(&ex);
        for tx in &ex.block.txs {
            if let Some(t) = self.arrivals.remove(&tx.signatures[0]) {
                let us = t.elapsed().as_micros() as u64;
                if let Some(r) = self.index.by_sig.get_mut(&tx.signatures[0]) {
                    r.node_us = Some(us);
                }
            }
        }
        if self.arrivals.len() > 100_000 {
            self.arrivals.retain(|_, t| t.elapsed() < Duration::from_secs(120)); // tx mai incluse
        }
        self.last_offset = Some(offset);
        remember(&mut self.recent, self.cfg.node.stream_catchup_blocks, ex.block.header.height, offset, &ex);
        let every = self.cfg.node.snapshot_every;
        if every > 0 && ex.block.header.height % every == 0 {
            self.snapshot()?;
        }
        Ok(ex)
    }

    /// Ciclo principale: un blocco ogni slot; tra un blocco e l'altro esegue le query.
    pub fn serve(mut self, queries: Receiver<Query>, mut on_block: impl FnMut(&Node, &Executed)) -> Result<(), String> {
        let slot = Duration::from_millis(self.cfg.node.slot_ms);
        let mut next = Instant::now() + slot;
        loop {
            let now = Instant::now();
            if now >= next {
                let ex = self.tick(crate::node::now_ms())?;
                on_block(&self, &ex);
                next = (next + slot).max(now);
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

    pub fn snapshot(&mut self) -> Result<(), String> {
        let c = &self.chain;
        ledger::write_snapshot(&self.dir, c.height, c.genesis_hash, c.svm.accounts().iter()).map_err(|e| format!("snapshot: {e}"))
    }
}

/// Nodo avviato: thread del nodo + canali per RPC e stream.
pub struct Running {
    pub gateway: crate::rpc::Gateway,
    pub stream: crate::stream::Stream,
    pub cfg: Config,
}

/// Apre il nodo su un thread dedicato (litesvm vive lì) e restituisce i canali.
pub fn start(cfg: Config, keypair: Keypair) -> Result<Running, String> {
    use crate::rpc::{BlockEvent, Gateway};
    let (qtx, qrx) = std::sync::mpsc::channel::<Query>();
    let (events, _) = tokio::sync::broadcast::channel(1024);
    let (blocks, _) = tokio::sync::broadcast::channel(cfg.node.stream_max_lag_blocks.max(1));
    let log = if cfg.node.rpc_log {
        std::fs::create_dir_all(&cfg.node.data_dir).map_err(|e| e.to_string())?;
        let path = cfg.node.data_dir.join("rpc.log");
        let f = std::fs::OpenOptions::new().create(true).append(true).open(&path).map_err(|e| format!("{}: {e}", path.display()))?;
        Some(Arc::new(std::sync::Mutex::new(f)))
    } else {
        None
    };
    let running = Running {
        gateway: Gateway { queries: qtx.clone(), events: events.clone(), log },
        stream: crate::stream::Stream {
            queries: qtx,
            blocks: blocks.clone(),
            max_wait: Duration::from_millis(cfg.node.slot_ms * cfg.node.stream_max_lag_blocks as u64),
        },
        cfg: cfg.clone(),
    };
    let (ready_tx, ready_rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let sym = cfg.node.token_symbol.clone();
        let node = match Node::open(cfg, keypair) {
            Ok(n) => n,
            Err(e) => return drop(ready_tx.send(Err(e))),
        };
        println!("{}: altezza {}, hash {}", node.cfg.node.token_name, node.chain.height, node.chain.last_hash);
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
            eprintln!("errore nodo: {e}");
            std::process::exit(1);
        }
    });
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

fn remember(recent: &mut VecDeque<(u64, u64, Vec<Address>)>, window: usize, height: u64, offset: u64, ex: &Executed) {
    recent.push_back((height, offset, ex.changed.iter().copied().collect()));
    while recent.len() > window {
        recent.pop_front();
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
