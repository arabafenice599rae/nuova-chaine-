//! Ingresso transazioni: verifica firme in batch, numero progressivo (seq), coda FIFO.
//! La priority fee non cambia l'ordine: si esegue rigorosamente per seq.

use std::collections::{HashMap, VecDeque};
use std::net::IpAddr;
use std::sync::Mutex;
use std::time::Instant;

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
}

/// Chi invia: l'IP per HTTP (e domani per QUIC); `None` = locale, senza limite di frequenza.
pub type Sender = Option<IpAddr>;

#[derive(Debug, PartialEq, Eq)]
pub enum Rejected {
    BadSignature,
    RateLimited,
    TooManyPending,
}

#[derive(Default)]
struct Queue {
    next_seq: u64,
    txs: VecDeque<(u64, VersionedTransaction)>,
    pending: HashMap<Address, u32>,
}

/// Coda di arrivo. seq e inserimento avvengono sotto lo stesso lock,
/// quindi l'ordine della coda coincide con l'ordine di seq.
pub struct Ingress {
    limits: Limits,
    queue: Mutex<Queue>,
    /// IP -> (gettoni, ultimo aggiornamento): token bucket.
    buckets: Mutex<HashMap<IpAddr, (f64, Instant)>>,
}

impl Ingress {
    pub fn new(limits: Limits) -> Ingress {
        Ingress { limits, queue: Mutex::default(), buckets: Mutex::default() }
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

    /// Verifica (firme in batch) e accoda; restituisce il seq assegnato a ogni tx.
    pub fn submit(&self, sender: Sender, txs: Vec<VersionedTransaction>) -> Vec<Result<u64, Rejected>> {
        let ok = verify_signatures(&txs);
        let mut q = self.queue.lock().unwrap();
        txs.into_iter()
            .zip(ok)
            .map(|(tx, ok)| {
                if !ok {
                    return Err(Rejected::BadSignature);
                }
                if sender.is_some_and(|ip| !self.take_token(ip)) {
                    return Err(Rejected::RateLimited);
                }
                let payer = tx.message.static_account_keys()[0];
                let n = q.pending.entry(payer).or_default();
                if *n >= self.limits.pending_per_payer {
                    return Err(Rejected::TooManyPending);
                }
                *n += 1;
                let seq = q.next_seq;
                q.next_seq += 1;
                q.txs.push_back((seq, tx));
                Ok(seq)
            })
            .collect()
    }

    pub fn is_empty(&self) -> bool {
        self.queue.lock().unwrap().txs.is_empty()
    }

    /// Preleva fino a `max` transazioni in ordine di seq.
    pub fn drain(&self, max: usize) -> Vec<VersionedTransaction> {
        let mut q = self.queue.lock().unwrap();
        let n = max.min(q.txs.len());
        let out: Vec<_> = q.txs.drain(..n).map(|(_, tx)| tx).collect();
        for tx in &out {
            let payer = tx.message.static_account_keys()[0];
            if let Some(c) = q.pending.get_mut(&payer) {
                *c -= 1;
                if *c == 0 {
                    q.pending.remove(&payer);
                }
            }
        }
        out
    }
}
