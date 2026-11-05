//! Ingresso transazioni: verifica firme in batch, numero progressivo (seq), coda FIFO.
//! La priority fee non cambia l'ordine: si esegue rigorosamente per seq.

use std::collections::VecDeque;
use std::sync::Mutex;

use ed25519_dalek::{Signature as DSig, VerifyingKey};
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
        let parsed: Option<Vec<_>> = ks[..need]
            .iter()
            .map(|k| VerifyingKey::from_bytes(&k.to_bytes()).ok().filter(|k| !k.is_weak()))
            .collect();
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
    spans
        .iter()
        .map(|sp| {
            sp.is_some_and(|(a, b)| (a..b).all(|i| ed25519_dalek::Verifier::verify(&keys[i], refs[i], &sigs[i]).is_ok()))
        })
        .collect()
}

/// Coda di arrivo. seq e inserimento avvengono sotto lo stesso lock,
/// quindi l'ordine della coda coincide con l'ordine di seq.
#[derive(Default)]
pub struct Ingress {
    inner: Mutex<(u64, VecDeque<(u64, VersionedTransaction)>)>,
}

impl Ingress {
    /// Verifica e accoda; restituisce il seq assegnato (o None se firma invalida).
    pub fn submit(&self, txs: Vec<VersionedTransaction>) -> Vec<Option<u64>> {
        let ok = verify_signatures(&txs);
        let mut g = self.inner.lock().unwrap();
        txs.into_iter()
            .zip(ok)
            .map(|(tx, ok)| {
                ok.then(|| {
                    let seq = g.0;
                    g.0 += 1;
                    g.1.push_back((seq, tx));
                    seq
                })
            })
            .collect()
    }

    /// Preleva fino a `max` transazioni in ordine di seq.
    pub fn drain(&self, max: usize) -> Vec<VersionedTransaction> {
        let mut g = self.inner.lock().unwrap();
        let n = max.min(g.1.len());
        g.1.drain(..n).map(|(_, tx)| tx).collect()
    }
}
