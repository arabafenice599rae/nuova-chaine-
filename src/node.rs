//! Nodo: apertura (snapshot + journal), produzione dei blocchi, snapshot periodici.

use std::path::PathBuf;
use std::sync::Arc;

use solana_keypair::Keypair;
use solana_signer::Signer;

use crate::exec::litesvm_internal::Svm;
use crate::exec::{Chain, Executed};
use crate::genesis::{self, Config};
use crate::ingress::Ingress;
use crate::ledger::{self, Journal};

pub struct Node {
    pub chain: Chain,
    pub ingress: Arc<Ingress>,
    pub cfg: Config,
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
        let journal = Journal::open(&dir.join("journal.bin"), |b| {
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
                chain.apply(&b).map(|_| ())
            }
        })?;
        if chain.height < snap_height {
            return Err("journal più corto dello snapshot".into());
        }
        Ok(Node { chain, ingress: Arc::default(), cfg, journal, dir, keypair })
    }

    /// Un ciclo di slot: preleva in ordine FIFO, esegue, firma, scrive nel journal.
    pub fn tick(&mut self, now_ms: u64) -> Result<Executed, String> {
        let txs = self.ingress.drain(self.cfg.node.max_block_txs);
        let ex = self.chain.produce(txs, now_ms, &self.keypair);
        self.journal.append(&ex.block).map_err(|e| format!("journal: {e}"))?;
        let every = self.cfg.node.snapshot_every;
        if every > 0 && ex.block.header.height % every == 0 {
            self.snapshot()?;
        }
        Ok(ex)
    }

    pub fn snapshot(&mut self) -> Result<(), String> {
        let c = &self.chain;
        ledger::write_snapshot(&self.dir, c.height, c.genesis_hash, c.svm.accounts().iter()).map_err(|e| format!("snapshot: {e}"))
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
