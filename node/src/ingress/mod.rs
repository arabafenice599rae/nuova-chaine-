//! Ingresso transazioni. All'arrivo ogni tx riceve un numero progressivo (seq, contatore
//! atomico), prima di qualunque verifica: è il seq a decidere l'ordine di esecuzione. Un
//! pool di thread decodifica, verifica le firme e il blockhash; il nodo preleva solo il
//! prefisso contiguo di seq già verificati, e una tx scartata lascia un buco che si salta.
//! La priority fee non cambia l'ordine.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::net::IpAddr;
use std::sync::atomic::{AtomicU64, Ordering::Relaxed};
use std::sync::mpsc::{Receiver, SyncSender, TrySendError};
use std::sync::{Arc, Mutex, RwLock};
use std::time::Instant;

use solana_hash::Hash;
use tokio::sync::oneshot;

use ed25519_dalek::{Signature as DSig, VerifyingKey};
use solana_address::Address;
use solana_transaction::versioned::VersionedTransaction;

/// Verifica le firme di più transazioni con una sola verifica batch ed25519.
/// Se il batch fallisce si ripiega sulla verifica singola (stessa semantica) per
/// trovare le tx invalide. Il batch di dalek è deterministico (casualità derivata
/// dagli input), quindi chi riesegue il journal ottiene lo stesso esito.
pub fn verify_signatures(txs: &[VersionedTransaction]) -> Vec<bool> {
    let mut msgs = Vec::new();
    let mut sigs = Vec::new();
    let mut keys = Vec::new();
    let mut spans = Vec::with_capacity(txs.len()); // (inizio, fine) in sigs, o None se malformata
    for tx in txs {
        let need = tx.message.header().num_required_signatures as usize;
        let ks = tx.message.static_account_keys();
        if tx.signatures.len() != need || need == 0 || ks.len() < need || tx.message.sanitize().is_err() {
            spans.push(None);
            continue;
        }
        let parsed: Option<Vec<_>> = ks[..need].iter().map(|k| VerifyingKey::from_bytes(&k.to_bytes()).ok().filter(|k| !k.is_weak())).collect();
        let Some(parsed) = parsed else {
            spans.push(None);
            continue;
        };
        let start = sigs.len();
        let m = tx.message.serialize();
        for (s, k) in tx.signatures.iter().zip(parsed) {
            msgs.push(m.clone());
            sigs.push(DSig::from_bytes(&<[u8; 64]>::from(*s)));
            keys.push(k);
        }
        spans.push(Some((start, sigs.len())));
    }
    let refs: Vec<&[u8]> = msgs.iter().map(|m| m.as_slice()).collect();
    if !sigs.is_empty() && ed25519_dalek::verify_batch(&refs, &sigs, &keys).is_ok() {
        return spans.iter().map(Option::is_some).collect();
    }
    spans.iter().map(|sp| sp.is_some_and(|(a, b)| (a..b).all(|i| ed25519_dalek::Verifier::verify(&keys[i], refs[i], &sigs[i]).is_ok()))).collect()
}

/// Limiti di ingresso (dalla config del nodo).
#[derive(Clone, Copy, Debug)]
pub struct Limits {
    /// Transazioni al secondo per indirizzo IP (burst = 2 secondi).
    pub tps_per_ip: u32,
    /// Transazioni in coda non ancora eseguite per pagatore.
    pub pending_per_payer: u32,
    /// Thread di verifica (0 = core - 2, almeno 1) e tx in attesa di verifica, al massimo.
    pub workers: usize,
    pub queue: usize,
}

/// Chi invia: l'IP per HTTP (e domani per QUIC); `None` = locale, senza limite di frequenza.
pub type Sender = Option<IpAddr>;

#[derive(Debug, PartialEq, Eq)]
pub enum Rejected {
    BadSignature,
    RateLimited,
    TooManyPending,
    QueueFull,
    BlockhashNotFound,
    /// Tx non decodificabile (codifica, formato, sanitize).
    Invalid(String),
}

/// Una tx da verificare: già decodificata, oppure i parametri di sendTransaction.
pub enum Item {
    Tx(VersionedTransaction),
    Rpc(serde_json::Value),
}

type Reply = oneshot::Sender<Result<(u64, solana_signature::Signature), Rejected>>;

/// Seq già verificati: `None` = scartato (buco da saltare).
#[derive(Default)]
struct Ready {
    next: u64,
    slots: BTreeMap<u64, Option<(VersionedTransaction, Option<Instant>)>>,
    pending: HashMap<Address, u32>,
}

struct Shared {
    ready: Mutex<Ready>,
    /// Blockhash validi per il prossimo blocco (li aggiorna il nodo a ogni blocco).
    hashes: RwLock<HashSet<Hash>>,
    pending_per_payer: u32,
}

pub struct Ingress {
    limits: Limits,
    seq: AtomicU64,
    jobs: SyncSender<(u64, Item, Instant, Reply)>,
    shared: Arc<Shared>,
    /// IP -> (gettoni, ultimo aggiornamento): token bucket.
    buckets: Mutex<HashMap<IpAddr, (f64, Instant)>>,
}

impl Ingress {
    pub fn new(limits: Limits) -> Ingress {
        let ready = Mutex::new(Ready::default());
        let shared = Arc::new(Shared { ready, hashes: Default::default(), pending_per_payer: limits.pending_per_payer });
        let (jobs, rx) = std::sync::mpsc::sync_channel(limits.queue.max(1));
        let rx = Arc::new(Mutex::new(rx));
        let cores = std::thread::available_parallelism().map_or(1, |n| n.get());
        let n = if limits.workers > 0 { limits.workers } else { cores.saturating_sub(2).max(1) };
        for i in 0..n {
            let (rx, sh) = (rx.clone(), shared.clone());
            std::thread::Builder::new().name(format!("centrifuga-verify-{i}")).spawn(move || worker(&rx, &sh)).expect("verify thread");
        }
        Ingress { limits, seq: AtomicU64::new(0), jobs, shared, buckets: Mutex::default() }
    }

    /// Un gettone per tx; ricarica `tps_per_ip` al secondo, massimo 2 secondi di scorta.
    fn take_token(&self, ip: IpAddr) -> bool {
        let (rate, now) = (self.limits.tps_per_ip as f64, Instant::now());
        let mut b = self.buckets.lock().unwrap();
        if b.len() > 100_000 {
            b.retain(|_, (_, t)| now.duration_since(*t).as_secs() < 2);
        }
        let (tokens, last) = b.entry(ip).or_insert((2.0 * rate, now));
        *tokens = (*tokens + now.duration_since(*last).as_secs_f64() * rate).min(2.0 * rate);
        *last = now;
        (*tokens >= 1.0).then(|| *tokens -= 1.0).is_some()
    }

    /// Seq all'arrivo, poi in coda verso il pool; la risposta arriva quando la tx è
    /// verificata (o scartata). Con la coda piena il seq diventa subito un buco.
    pub fn send(&self, sender: Sender, item: Item, arrived: Instant) -> oneshot::Receiver<Result<(u64, solana_signature::Signature), Rejected>> {
        let (tx, rx) = oneshot::channel();
        if sender.is_some_and(|ip| !self.take_token(ip)) {
            let _ = tx.send(Err(Rejected::RateLimited));
            return rx;
        }
        let seq = self.seq.fetch_add(1, Relaxed);
        if let Err(TrySendError::Full((seq, _, _, tx)) | TrySendError::Disconnected((seq, _, _, tx))) = self.jobs.try_send((seq, item, arrived, tx)) {
            self.shared.ready.lock().unwrap().slots.insert(seq, None);
            let _ = tx.send(Err(Rejected::QueueFull));
        }
        rx
    }

    /// Accoda e aspetta la verifica; restituisce il seq assegnato a ogni tx.
    pub fn submit(&self, sender: Sender, txs: Vec<VersionedTransaction>) -> Vec<Result<u64, Rejected>> {
        let rxs: Vec<_> = txs.into_iter().map(|t| self.send(sender, Item::Tx(t), Instant::now())).collect();
        rxs.into_iter().map(|rx| rx.blocking_recv().unwrap_or(Err(Rejected::QueueFull)).map(|(seq, _)| seq)).collect()
    }

    /// Blockhash validi per il prossimo blocco: le tx con un altro blockhash si scartano
    /// (salvo le durable nonce, che il nodo controlla all'esecuzione).
    pub fn set_blockhashes(&self, hashes: HashSet<Hash>) {
        *self.shared.hashes.write().unwrap() = hashes;
    }

    /// Nessuna tx verificata pronta per il prossimo blocco.
    pub fn is_empty(&self) -> bool {
        let r = self.shared.ready.lock().unwrap();
        r.slots.first_key_value().is_none_or(|(k, _)| *k != r.next)
    }

    /// Preleva fino a `max` tx del prefisso contiguo di seq verificati (i buchi si saltano),
    /// con l'istante di arrivo per quelle arrivate via RPC.
    pub fn drain(&self, max: usize) -> Vec<(VersionedTransaction, Option<Instant>)> {
        let mut guard = self.shared.ready.lock().unwrap();
        let r = &mut *guard;
        let mut out = vec![];
        while out.len() < max && r.slots.first_key_value().is_some_and(|(k, _)| *k == r.next) {
            r.next += 1;
            let Some((tx, at)) = r.slots.pop_first().unwrap().1 else { continue };
            let payer = tx.message.static_account_keys()[0];
            if let Some(c) = r.pending.get_mut(&payer) {
                *c -= 1;
                if *c == 0 {
                    r.pending.remove(&payer);
                }
            }
            out.push((tx, at));
        }
        out
    }
}

/// Thread di verifica: decodifica, firme, blockhash, limite per pagatore; scrive l'esito
/// nello slot del suo seq e risponde a chi ha inviato.
fn worker(rx: &Mutex<Receiver<(u64, Item, Instant, Reply)>>, sh: &Shared) {
    loop {
        let Ok((seq, item, arrived, reply)) = rx.lock().unwrap().recv() else { return };
        let (tx, arrived) = match item {
            Item::Tx(t) => (Ok(t), None),
            Item::Rpc(p) => (crate::rpc::methods::decode_tx(&p).map_err(|e| Rejected::Invalid(e.message)), Some(arrived)),
        };
        let res = tx.and_then(|tx| {
            if !verify_signatures(std::slice::from_ref(&tx))[0] {
                return Err(Rejected::BadSignature);
            }
            if !sh.hashes.read().unwrap().contains(tx.message.recent_blockhash()) && !maybe_nonce(&tx) {
                return Err(Rejected::BlockhashNotFound);
            }
            Ok(tx)
        });
        let mut r = sh.ready.lock().unwrap();
        let res = res.and_then(|tx| {
            let n = r.pending.entry(tx.message.static_account_keys()[0]).or_default();
            if *n >= sh.pending_per_payer {
                return Err(Rejected::TooManyPending);
            }
            *n += 1;
            Ok(tx)
        });
        let out = res.map(|tx| {
            let sig = tx.signatures[0];
            r.slots.insert(seq, Some((tx, arrived)));
            (seq, sig)
        });
        if out.is_err() {
            r.slots.insert(seq, None);
        }
        drop(r);
        let _ = reply.send(out);
    }
}

/// Prima istruzione AdvanceNonceAccount: forse una durable nonce (la verifica completa
/// la fa il nodo all'esecuzione).
fn maybe_nonce(tx: &VersionedTransaction) -> bool {
    let (m, keys) = (&tx.message, tx.message.static_account_keys());
    m.instructions()
        .first()
        .is_some_and(|ix| keys.get(ix.program_id_index as usize) == Some(&solana_sdk_ids::system_program::id()) && ix.data.starts_with(&[4, 0, 0, 0]))
}
