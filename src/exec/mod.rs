//! Esecuzione dei blocchi: stessa funzione per produrre (operatore) e
//! rieseguire (riavvio, repliche, verificatori).

pub mod litesvm_internal;
pub mod state_hash;

use std::collections::{BTreeSet, HashSet, VecDeque};

use solana_account::{Account, ReadableAccount};
use solana_address::Address;
use solana_hash::Hash;
use solana_keypair::Keypair;
use solana_nonce::state::{Data, DurableNonce, State};
use solana_nonce::versions::Versions;
use solana_sdk_ids::{system_program, sysvar};
use solana_signature::Signature;
use solana_transaction::versioned::VersionedTransaction;

use crate::ledger::{Block, Header};
use litesvm_internal::{Svm, TransactionError};
use state_hash::{FullSha256, StateHasher};

pub use litesvm::types::TransactionResult;

#[derive(Clone, Debug)]
pub struct Params {
    pub operator: Address,
    pub lamports_per_signature: u64,
    /// Validità di un blockhash, in blocchi (N).
    pub blockhash_window: usize,
}

pub struct Chain {
    pub svm: Svm,
    pub params: Params,
    pub height: u64,
    pub last_hash: Hash,
    pub last_ts: u64,
    pub genesis_hash: Hash,
    /// Ultimi N blocchi: (hash, firme incluse).
    window: VecDeque<(Hash, Vec<Signature>)>,
    hashes: HashSet<Hash>,
    seen: HashSet<Signature>,
    hasher: Box<dyn StateHasher>,
}

/// Risultato di un blocco eseguito.
pub struct Executed {
    pub block: Block,
    /// Account (potenzialmente) cambiati: base del delta per lo stream.
    pub changed: BTreeSet<Address>,
    pub results: Vec<TransactionResult>,
}

/// Una tx durable-nonce: (account nonce, autorità).
type Nonce = Option<(Address, Address)>;

impl Chain {
    /// Catena a partire da uno stato (genesis o snapshot) all'altezza `height`.
    pub fn new(svm: Svm, params: Params, height: u64, last_hash: Hash, last_ts: u64, genesis_hash: Hash) -> Chain {
        let mut c = Chain {
            svm,
            params,
            height,
            last_hash,
            last_ts,
            genesis_hash,
            window: VecDeque::new(),
            hashes: HashSet::new(),
            seen: HashSet::new(),
            hasher: Box::new(FullSha256),
        };
        c.push_window(last_hash, Vec::new());
        c
    }

    /// Stato di genesis: il "blocco 0" non è firmato e non sta nel journal.
    pub fn genesis(svm: Svm, params: Params, timestamp_ms: u64) -> Chain {
        let mut c = Chain::new(svm, params, 0, Hash::default(), timestamp_ms, Hash::default());
        let g = Block {
            header: Header {
                height: 0,
                prev_hash: Hash::default(),
                timestamp_ms,
                state_hash: c.state_hash(&BTreeSet::new()),
                signature: Signature::default(),
            },
            txs: vec![],
        };
        let h = g.hash();
        c.window.clear();
        c.hashes.clear();
        c.push_window(h, Vec::new());
        c.last_hash = h;
        c.genesis_hash = h;
        c
    }

    pub fn state_hash(&mut self, changed: &BTreeSet<Address>) -> Hash {
        self.hasher.hash(self.svm.accounts(), changed)
    }

    fn push_window(&mut self, h: Hash, sigs: Vec<Signature>) {
        self.seen.extend(sigs.iter().copied());
        self.hashes.insert(h);
        self.window.push_back((h, sigs));
        while self.window.len() > self.params.blockhash_window {
            let (h, sigs) = self.window.pop_front().unwrap();
            self.hashes.remove(&h);
            for s in sigs {
                self.seen.remove(&s);
            }
        }
    }

    /// Registra un blocco già incluso nello snapshot (solo finestra, nessuna esecuzione).
    pub fn note_block(&mut self, b: &Block) {
        self.height = b.header.height;
        self.last_ts = b.header.timestamp_ms;
        self.last_hash = b.hash();
        self.push_window(self.last_hash, b.txs.iter().map(|t| t.signatures[0]).collect());
    }

    /// Controlli del nodo prima dell'esecuzione: duplicati, blockhash, durable nonce.
    pub fn check(&self, tx: &VersionedTransaction) -> Result<Nonce, TransactionError> {
        let sig = tx.signatures.first().ok_or(TransactionError::SanitizeFailure)?;
        if self.seen.contains(sig) {
            return Err(TransactionError::AlreadyProcessed);
        }
        let bh = tx.message.recent_blockhash();
        if self.hashes.contains(bh) {
            return Ok(None);
        }
        self.check_nonce(tx).map(Some).ok_or(TransactionError::BlockhashNotFound)
    }

    /// Durable nonce: la prima istruzione è AdvanceNonceAccount, il nonce salvato
    /// coincide con il blockhash della tx, è avanzabile e l'autorità ha firmato.
    fn check_nonce(&self, tx: &VersionedTransaction) -> Option<(Address, Address)> {
        let m = &tx.message;
        let keys = m.static_account_keys();
        let ix = m.instructions().first()?;
        if keys.get(ix.program_id_index as usize)? != &system_program::id() || ix.data.get(..4)? != [4, 0, 0, 0] {
            return None;
        }
        let ni = *ix.accounts.first()? as usize;
        let nonce_key = *keys.get(ni)?;
        if !m.is_maybe_writable_with_reserved_addresses(ni, None::<&HashSet<Address>>) {
            return None;
        }
        let data = solana_nonce_account::verify_nonce_account(self.svm.get(&nonce_key)?, m.recent_blockhash())?;
        let next = DurableNonce::from_blockhash(&self.last_hash);
        let signers = &keys[..m.header().num_required_signatures as usize];
        (data.durable_nonce != next && signers.contains(&data.authority)).then_some((nonce_key, data.authority))
    }

    /// Una tx durable-nonce fallita fa comunque avanzare il nonce (come Agave),
    /// altrimenti potrebbe essere rieseguita dopo l'uscita dalla finestra.
    fn advance_nonce(&mut self, nonce_key: Address, authority: Address) {
        let Some(acc) = self.svm.get(&nonce_key) else { return };
        let mut acc: Account = acc.clone().into();
        let next = DurableNonce::from_blockhash(&self.last_hash);
        let st = Versions::new(State::Initialized(Data::new(authority, next, self.params.lamports_per_signature)));
        acc.data = wincode::serialize(&st).expect("nonce");
        self.svm.set(nonce_key, acc);
    }

    fn credit(&mut self, to: Address, lamports: u64, changed: &mut BTreeSet<Address>) {
        if lamports == 0 {
            return;
        }
        let mut acc: Account = self.svm.get(&to).cloned().map(Into::into).unwrap_or(Account {
            owner: system_program::id(),
            ..Account::default()
        });
        acc.lamports += lamports;
        self.svm.set(to, acc);
        changed.insert(to);
    }

    /// Esegue un blocco. `strict` = riesecuzione: ogni tx deve essere includibile.
    fn run(&mut self, ts: u64, txs: Vec<VersionedTransaction>, strict: bool) -> Result<Executed, String> {
        let height = self.height + 1;
        self.svm.begin_block(height, (ts / 1000) as i64, self.last_hash);
        let mut changed = BTreeSet::from([sysvar::clock::id()]);
        let (mut included, mut results, mut sigs) = (Vec::new(), Vec::new(), Vec::new());
        let mut fees = 0u64;
        for tx in txs {
            let nonce = match self.check(&tx) {
                Ok(n) => n,
                Err(e) if strict => return Err(format!("tx {} rifiutata: {e}", tx.signatures[0])),
                Err(_) => continue,
            };
            let writable = self.svm.writable_keys(&tx);
            let ex = self.svm.execute(tx.clone());
            if ex.result.is_err() && ex.fee_charged == 0 {
                if strict {
                    return Err(format!("tx {} non includibile", tx.signatures[0]));
                }
                continue;
            }
            if let (Err(_), Some((k, auth))) = (&ex.result, nonce) {
                self.advance_nonce(k, auth);
            }
            fees += ex.fee_charged;
            changed.extend(writable);
            self.seen.insert(tx.signatures[0]);
            sigs.push(tx.signatures[0]);
            included.push(tx);
            results.push(ex.result);
        }
        // Passo 1: tutte le fee all'operatore (supply conservata). Fee mining al passo 2.
        self.credit(self.params.operator, fees, &mut changed);
        let state_hash = self.state_hash(&changed);
        let block = Block {
            header: Header { height, prev_hash: self.last_hash, timestamp_ms: ts, state_hash, signature: Signature::default() },
            txs: included,
        };
        // `seen` ha già le firme del blocco: push_window le registra per la scadenza.
        for s in &sigs {
            self.seen.remove(s);
        }
        self.height = height;
        self.last_ts = ts;
        Ok(Executed { block, changed, results })
    }

    fn finish(&mut self, ex: &Executed) {
        self.last_hash = ex.block.hash();
        self.push_window(self.last_hash, ex.block.txs.iter().map(|t| t.signatures[0]).collect());
    }

    /// Operatore: costruisce, esegue e firma il blocco successivo.
    pub fn produce(&mut self, txs: Vec<VersionedTransaction>, now_ms: u64, kp: &Keypair) -> Executed {
        let ts = now_ms.max(self.last_ts + 1);
        let mut ex = self.run(ts, txs, false).expect("produzione non strict");
        ex.block.sign(kp);
        self.finish(&ex);
        ex
    }

    /// Chiunque: riesegue un blocco firmato e verifica header e hash di stato.
    pub fn apply(&mut self, b: &Block) -> Result<Executed, String> {
        let h = &b.header;
        if h.height != self.height + 1 || h.prev_hash != self.last_hash || h.timestamp_ms <= self.last_ts {
            return Err(format!("blocco {}: header non concatenato", h.height));
        }
        if !b.verify_signature(&self.params.operator) {
            return Err(format!("blocco {}: firma operatore invalida", h.height));
        }
        let valid = crate::ingress::verify_signatures(&b.txs);
        if valid.iter().any(|ok| !ok) {
            return Err(format!("blocco {}: firma tx invalida", h.height));
        }
        let mut ex = self.run(h.timestamp_ms, b.txs.clone(), true)?;
        if ex.block.header.state_hash != h.state_hash {
            return Err(format!("blocco {}: hash di stato diverso", h.height));
        }
        ex.block.header.signature = h.signature;
        self.finish(&ex);
        Ok(ex)
    }

    pub fn total_lamports(&self) -> u128 {
        self.svm.accounts().values().map(|a| a.lamports() as u128).sum()
    }
}
