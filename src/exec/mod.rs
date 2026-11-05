//! Esecuzione dei blocchi: stessa funzione per produrre (operatore) e
//! rieseguire (riavvio, repliche, verificatori).

pub mod litesvm_internal;
pub mod state_hash;

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet, VecDeque};

use solana_account::{Account, ReadableAccount};
use solana_address::Address;
use solana_hash::Hash;
use solana_keypair::Keypair;
use solana_nonce::state::{Data, DurableNonce, State};
use solana_nonce::versions::Versions;
use solana_message::VersionedMessage;
use solana_sdk_ids::{ed25519_program, secp256k1_program, secp256r1_program, system_program, sysvar};
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
    /// Validità di un blockhash, in slot (N): il blockhash del blocco all'altezza h
    /// vale nei blocchi fino all'altezza h + N.
    pub blockhash_window: usize,
    /// Fee mining: premio massimo per blocco all'altezza 0.
    pub block_budget: u64,
    /// Il budget dimezza ogni `halving_blocks` blocchi.
    pub halving_blocks: u64,
    /// Da questa altezza (o a vault "mining" vuoto) il fee mining finisce.
    pub mining_end_height: u64,
    pub vaults: Vaults,
    pub rules: Rules,
}

/// Regole di validità dell'header tra un blocco e il successivo (parte del genesis).
#[derive(Clone, Copy, Debug)]
pub struct Rules {
    pub slot_ms: u64,
    /// Salto massimo di altezza tra due blocchi (e cadenza dell'heartbeat).
    pub heartbeat_slots: u64,
}

impl Rules {
    /// Timestamp minimo per un blocco `gap` slot dopo il precedente.
    pub fn min_timestamp(&self, prev_ts: u64, gap: u64) -> u64 {
        prev_ts + (gap * self.slot_ms / 2).max(1)
    }

    /// 1 <= gap <= heartbeat_slots e timestamp - precedente >= gap * slot_ms / 2.
    pub fn check(&self, prev_height: u64, prev_ts: u64, h: &Header) -> Result<(), String> {
        let gap = h.height.checked_sub(prev_height).filter(|g| (1..=self.heartbeat_slots).contains(g));
        let Some(gap) = gap else {
            return Err(format!("blocco {}: salto di altezza da {prev_height} fuori da 1..={}", h.height, self.heartbeat_slots));
        };
        if h.timestamp_ms < self.min_timestamp(prev_ts, gap) {
            return Err(format!("blocco {}: timestamp {} troppo vicino al precedente {prev_ts} (salto {gap})", h.height, h.timestamp_ms));
        }
        Ok(())
    }
}

impl Params {
    /// Hash dei parametri di consenso: entra nel genesis hash (prev_hash del blocco 0).
    pub fn hash(&self) -> Hash {
        use sha2::{Digest, Sha256};
        let mut d = Sha256::new();
        d.update(self.operator.as_ref());
        for x in [
            self.lamports_per_signature, self.blockhash_window as u64, self.block_budget, self.halving_blocks,
            self.mining_end_height, self.rules.slot_ms, self.rules.heartbeat_slots,
        ] {
            d.update(x.to_le_bytes());
        }
        for v in [&self.vaults.mining, &self.vaults.airdrop, &self.vaults.reserve] {
            d.update(v.as_ref());
        }
        Hash::new_from_array(d.finalize().into())
    }
}

/// Vault che il nodo muove direttamente (regole di fee nel nodo).
#[derive(Clone, Debug)]
pub struct Vaults {
    pub mining: Address,
    pub airdrop: Address,
    pub reserve: Address,
}

/// Fee base di un messaggio: firme della tx + firme verificate dalle precompile,
/// per `lamports_per_signature` (stessa formula dell'SVM). Esclude la priority fee.
pub fn base_fee(m: &VersionedMessage, lamports_per_signature: u64) -> u64 {
    let keys = m.static_account_keys();
    let pre = [ed25519_program::id(), secp256k1_program::id(), secp256r1_program::id()];
    let extra: u64 = m
        .instructions()
        .iter()
        .filter(|ix| keys.get(ix.program_id_index as usize).is_some_and(|k| pre.contains(k)))
        .map(|ix| u64::from(ix.data.first().copied().unwrap_or(0)))
        .sum();
    (u64::from(m.header().num_required_signatures) + extra).saturating_mul(lamports_per_signature)
}

/// Come sono state ripartite le fee di un blocco.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Settlement {
    pub mining_active: bool,
    /// Somma delle fee base pagate nel blocco.
    pub base: u64,
    /// Priority fee restituite ai pagatori.
    pub priority_refund: u64,
    pub operator: u64,
    pub reserve: u64,
    /// Premio pagato dal vault "mining" ai pagatori.
    pub reward: u64,
    /// Residuo del vault "mining" passato ad "airdrop" a fine periodo.
    pub to_airdrop: u64,
}

pub struct Chain {
    pub svm: Svm,
    pub params: Params,
    pub height: u64,
    pub last_hash: Hash,
    pub last_ts: u64,
    pub genesis_hash: Hash,
    /// Ultimi N blocchi: (hash, firme incluse).
    window: VecDeque<(u64, Hash, Vec<Signature>)>,
    /// blockhash -> altezza del blocco.
    hashes: HashMap<Hash, u64>,
    seen: HashSet<Signature>,
    hasher: Box<dyn StateHasher>,
}

/// Risultato di un blocco eseguito.
pub struct Executed {
    pub block: Block,
    /// Account (potenzialmente) cambiati: base del delta per lo stream.
    pub changed: BTreeSet<Address>,
    pub results: Vec<TransactionResult>,
    /// Saldi (prima, dopo) degli account statici di ogni tx, per getTransaction.
    pub balances: Vec<(Vec<u64>, Vec<u64>)>,
    pub settlement: Settlement,
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
            hashes: HashMap::new(),
            seen: HashSet::new(),
            hasher: Box::new(FullSha256),
        };
        c.push_window(height, last_hash, Vec::new());
        c
    }

    /// Stato di genesis: il "blocco 0" non è firmato e non sta nel journal.
    pub fn genesis(svm: Svm, params: Params, timestamp_ms: u64) -> Chain {
        let params_hash = params.hash();
        let mut c = Chain::new(svm, params, 0, Hash::default(), timestamp_ms, Hash::default());
        let g = Block {
            header: Header {
                height: 0,
                prev_hash: params_hash,
                timestamp_ms,
                state_hash: c.state_hash(&BTreeSet::new()),
                signature: Signature::default(),
            },
            txs: vec![],
        };
        let h = g.hash();
        c.window.clear();
        c.hashes.clear();
        c.push_window(0, h, Vec::new());
        c.last_hash = h;
        c.genesis_hash = h;
        c
    }

    pub fn state_hash(&mut self, changed: &BTreeSet<Address>) -> Hash {
        self.hasher.hash(self.svm.accounts(), changed)
    }

    fn push_window(&mut self, height: u64, h: Hash, sigs: Vec<Signature>) {
        self.seen.extend(sigs.iter().copied());
        self.hashes.insert(h, height);
        self.window.push_back((height, h, sigs));
    }

    /// Prima di un blocco all'altezza `height`: scadono i blockhash (e le firme
    /// dei loro blocchi) più vecchi di N slot.
    fn expire(&mut self, height: u64) {
        let n = self.params.blockhash_window as u64;
        while self.window.front().is_some_and(|w| w.0 + n < height) {
            let (_, h, sigs) = self.window.pop_front().unwrap();
            self.hashes.remove(&h);
            for s in sigs {
                self.seen.remove(&s);
            }
        }
    }

    /// Registra un blocco già incluso nello snapshot: nessuna esecuzione, ma
    /// concatenamento, regole dell'header e firma sono verificati come in `apply`.
    pub fn note_block(&mut self, b: &Block) -> Result<(), String> {
        self.check_header(b)?;
        self.height = b.header.height;
        self.last_ts = b.header.timestamp_ms;
        self.last_hash = b.hash();
        self.push_window(self.height, self.last_hash, b.txs.iter().map(|t| t.signatures[0]).collect());
        self.expire(self.height + 1);
        Ok(())
    }

    /// Concatenamento, regole di altezza e timestamp, firma dell'operatore.
    fn check_header(&self, b: &Block) -> Result<(), String> {
        let h = &b.header;
        if h.prev_hash != self.last_hash {
            return Err(format!("blocco {}: header non concatenato", h.height));
        }
        self.params.rules.check(self.height, self.last_ts, h)?;
        if !b.verify_signature(&self.params.operator) {
            return Err(format!("blocco {}: firma operatore invalida", h.height));
        }
        Ok(())
    }

    /// Controlli del nodo prima dell'esecuzione: duplicati, blockhash, durable nonce.
    /// Per il prossimo blocco possibile (altezza + 1); in un blocco: `check_at`.
    pub fn check(&self, tx: &VersionedTransaction) -> Result<Nonce, TransactionError> {
        self.check_at(tx, self.height + 1)
    }

    fn check_at(&self, tx: &VersionedTransaction, height: u64) -> Result<Nonce, TransactionError> {
        let sig = tx.signatures.first().ok_or(TransactionError::SanitizeFailure)?;
        if self.seen.contains(sig) {
            return Err(TransactionError::AlreadyProcessed);
        }
        let bh = tx.message.recent_blockhash();
        if self.hashes.get(bh).is_some_and(|h| h + self.params.blockhash_window as u64 >= height) {
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

    fn debit(&mut self, from: Address, lamports: u64, changed: &mut BTreeSet<Address>) {
        if lamports == 0 {
            return;
        }
        let mut acc: Account = self.svm.get(&from).cloned().expect("debit da account inesistente").into();
        acc.lamports = acc.lamports.checked_sub(lamports).expect("debit oltre il saldo");
        self.svm.set(from, acc);
        changed.insert(from);
    }

    /// Lamport spendibili di un vault (oltre il minimo per la rent exemption).
    pub fn vault_available(&self, k: &Address) -> u64 {
        self.svm.get(k).map_or(0, |a| a.lamports().saturating_sub(self.svm.rent_exempt(a.data().len())))
    }

    /// Budget del premio per blocco: dimezza ogni `halving_blocks`.
    pub fn block_budget(&self, height: u64) -> u64 {
        let halvings = height / self.params.halving_blocks.max(1);
        self.params.block_budget.checked_shr(halvings.min(64) as u32).unwrap_or(0)
    }

    /// Fee mining attivo: prima della fine e con il vault "mining" non vuoto.
    pub fn mining_active(&self, height: u64) -> bool {
        height < self.params.mining_end_height && self.vault_available(&self.params.vaults.mining) > 0
    }

    /// Fine blocco. `paid`: (pagatore, fee base, priority fee) per ogni tx inclusa.
    /// - priority fee: rimborsata al pagatore;
    /// - fee mining attivo: 10% della base all'operatore, 90% al vault "reserve";
    ///   premio = min(budget, 90% della base, saldo "mining") ai pagatori in
    ///   proporzione alla base pagata (il resto della divisione resta nel vault);
    /// - dopo: 100% della base all'operatore, residuo di "mining" ad "airdrop".
    fn settle(&mut self, height: u64, paid: &[(Address, u64, u64)], changed: &mut BTreeSet<Address>) -> Settlement {
        let v = self.params.vaults.clone();
        let mut s = Settlement { mining_active: self.mining_active(height), ..Settlement::default() };
        let mut by_payer: BTreeMap<Address, u64> = BTreeMap::new();
        for (payer, base, prio) in paid {
            s.base += base;
            s.priority_refund += prio;
            *by_payer.entry(*payer).or_default() += base;
            self.credit(*payer, *prio, changed);
        }
        if !s.mining_active {
            s.operator = s.base;
            self.credit(self.params.operator, s.base, changed);
            s.to_airdrop = self.vault_available(&v.mining);
            self.debit(v.mining, s.to_airdrop, changed);
            self.credit(v.airdrop, s.to_airdrop, changed);
            return s;
        }
        s.operator = s.base / 10;
        s.reserve = s.base - s.operator;
        self.credit(self.params.operator, s.operator, changed);
        self.credit(v.reserve, s.reserve, changed);
        let ninety = (s.base as u128 * 9 / 10) as u64;
        let reward = self.block_budget(height).min(ninety).min(self.vault_available(&v.mining));
        if reward > 0 {
            for (payer, base) in by_payer {
                let share = (reward as u128 * base as u128 / s.base as u128) as u64;
                self.credit(payer, share, changed);
                s.reward += share;
            }
            self.debit(v.mining, s.reward, changed);
        }
        s
    }

    /// Esegue un blocco. `strict` = riesecuzione: ogni tx deve essere includibile.
    fn run(&mut self, height: u64, ts: u64, txs: Vec<VersionedTransaction>, strict: bool) -> Result<Executed, String> {
        self.expire(height);
        self.svm.begin_block(height, (ts / 1000) as i64, self.last_hash);
        let mut changed = BTreeSet::from([sysvar::clock::id()]);
        let (mut included, mut results, mut sigs, mut balances) = (Vec::new(), Vec::new(), Vec::new(), Vec::new());
        let mut paid = Vec::new();
        for tx in txs {
            let nonce = match self.check_at(&tx, height) {
                Ok(n) => n,
                Err(e) if strict => return Err(format!("tx {} rifiutata: {e}", tx.signatures[0])),
                Err(_) => continue,
            };
            let writable = self.svm.writable_keys(&tx);
            let keys = tx.message.static_account_keys();
            let pre: Vec<u64> = keys.iter().map(|k| self.svm.lamports(k)).collect();
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
            balances.push((pre, keys.iter().map(|k| self.svm.lamports(k)).collect()));
            let base = base_fee(&tx.message, self.params.lamports_per_signature).min(ex.fee_charged);
            paid.push((keys[0], base, ex.fee_charged - base));
            changed.extend(writable);
            self.seen.insert(tx.signatures[0]);
            sigs.push(tx.signatures[0]);
            included.push(tx);
            results.push(ex.result);
        }
        let settlement = self.settle(height, &paid, &mut changed);
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
        Ok(Executed { block, changed, results, balances, settlement })
    }

    fn finish(&mut self, ex: &Executed) {
        self.last_hash = ex.block.hash();
        self.push_window(ex.block.header.height, self.last_hash, ex.block.txs.iter().map(|t| t.signatures[0]).collect());
    }

    /// Operatore: costruisce, esegue e firma il blocco all'altezza successiva.
    pub fn produce(&mut self, txs: Vec<VersionedTransaction>, now_ms: u64, kp: &Keypair) -> Executed {
        self.produce_at(self.height + 1, txs, now_ms, kp)
    }

    /// Operatore: blocco all'altezza `height` (salto di 1..=heartbeat_slots slot).
    /// Timestamp = max(ora, precedente + salto × slot_ms / 2, precedente + 1).
    pub fn produce_at(&mut self, height: u64, txs: Vec<VersionedTransaction>, now_ms: u64, kp: &Keypair) -> Executed {
        let gap = height.saturating_sub(self.height);
        assert!((1..=self.params.rules.heartbeat_slots).contains(&gap), "salto di altezza {gap} fuori dalle regole");
        let ts = now_ms.max(self.params.rules.min_timestamp(self.last_ts, gap));
        let mut ex = self.run(height, ts, txs, false).expect("produzione non strict");
        ex.block.sign(kp);
        self.finish(&ex);
        ex
    }

    /// Chiunque: riesegue un blocco firmato e verifica header e hash di stato.
    pub fn apply(&mut self, b: &Block) -> Result<Executed, String> {
        let h = &b.header;
        self.check_header(b)?;
        let valid = crate::ingress::verify_signatures(&b.txs);
        if valid.iter().any(|ok| !ok) {
            return Err(format!("blocco {}: firma tx invalida", h.height));
        }
        let mut ex = self.run(h.height, h.timestamp_ms, b.txs.clone(), true)?;
        if ex.block.header.state_hash != h.state_hash {
            return Err(format!("blocco {}: hash di stato diverso", h.height));
        }
        ex.block.header.signature = h.signature;
        self.finish(&ex);
        Ok(ex)
    }

    /// Valido per il prossimo blocco possibile (altezza + 1).
    pub fn blockhash_valid(&self, h: &Hash) -> bool {
        self.hashes.get(h).is_some_and(|x| x + self.params.blockhash_window as u64 > self.height)
    }

    pub fn total_lamports(&self) -> u128 {
        self.svm.accounts().values().map(|a| a.lamports() as u128).sum()
    }
}
