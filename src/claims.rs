//! Aventi diritto al residuo del fee mining, calcolati in modo deterministico
//! dal journal. Chiunque abbia journal, config del genesis e lista airdrop può
//! rieseguire `centrifuga eligible` e confrontare l'hash del file prodotto.
//!
//! Regole:
//! - ha diritto chi è nella lista airdrop e, come pagatore, ha almeno `MIN_TX`
//!   tx riuscite in almeno `MIN_DAYS` giorni UTC diversi (dai timestamp dei
//!   blocchi), contando i blocchi del periodo di fee mining;
//! - quota uguale per tutti = min(`MAX_QUOTA`, residuo / numero aventi diritto);
//! - la parte non assegnata (residuo − quota × N) va al vault "reserve".

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use sha2::{Digest, Sha256};
use solana_address::Address;

use crate::genesis::{self, GenesisConfig, LAMPORTS_PER_CNFA};
use crate::ledger::Journal;

pub const MIN_TX: u64 = 10;
pub const MIN_DAYS: usize = 5;
pub const MAX_QUOTA: u64 = 1_000 * LAMPORTS_PER_CNFA;
const DAY_MS: u64 = 86_400_000;

#[derive(Debug, PartialEq, Eq)]
pub struct Eligibility {
    /// Primo blocco senza fee mining (quello che sposta il residuo).
    pub end_height: u64,
    pub residual: u64,
    /// In ordine di pubkey: la posizione è l'indice del claim.
    pub eligible: Vec<Address>,
    pub quota: u64,
    pub unassigned: u64,
}

/// Lista airdrop: una pubkey per riga, `#` per i commenti.
pub fn parse_list(text: &str) -> Result<BTreeSet<Address>, String> {
    text.lines().map(|l| l.split('#').next().unwrap().trim()).filter(|l| !l.is_empty()).map(genesis::parse_address).collect()
}

/// Riesegue il journal dal genesis fino alla fine del fee mining.
pub fn compute(g: &GenesisConfig, journal: &Path, airdrop: &BTreeSet<Address>) -> Result<Eligibility, String> {
    let mut chain = genesis::build(g)?.0;
    let mut stats: BTreeMap<Address, (u64, BTreeSet<u64>)> = BTreeMap::new();
    let mut end: Option<(u64, u64)> = None;
    Journal::open(journal, |b, _| {
        if end.is_some() {
            return Ok(());
        }
        let ex = chain.apply(&b)?;
        if !ex.settlement.mining_active {
            end = Some((b.header.height, ex.settlement.to_airdrop));
            return Ok(());
        }
        let day = b.header.timestamp_ms / DAY_MS;
        for (tx, r) in b.txs.iter().zip(&ex.results) {
            if r.is_ok() {
                let s = stats.entry(tx.message.static_account_keys()[0]).or_default();
                s.0 += 1;
                s.1.insert(day);
            }
        }
        Ok(())
    })?;
    let (end_height, residual) = end.ok_or("fee mining has not ended yet in the journal")?;
    let eligible: Vec<Address> =
        stats.into_iter().filter(|(k, (n, days))| *n >= MIN_TX && days.len() >= MIN_DAYS && airdrop.contains(k)).map(|(k, _)| k).collect();
    let n = eligible.len() as u64;
    let quota = residual.checked_div(n).map_or(0, |q| MAX_QUOTA.min(q));
    Ok(Eligibility { end_height, residual, quota, unassigned: residual - quota * n, eligible })
}

impl Eligibility {
    /// File pubblicato (testo, deterministico) e il suo SHA-256 in esadecimale.
    pub fn render(&self) -> (String, String) {
        let mut s = format!(
            "# centrifuga: fee mining residual\nend_height={}\nresidual={}\neligible={}\nquota={}\nunassigned={}\n",
            self.end_height,
            self.residual,
            self.eligible.len(),
            self.quota,
            self.unassigned
        );
        for (i, k) in self.eligible.iter().enumerate() {
            s.push_str(&format!("{i} {k}\n"));
        }
        let hash = Sha256::digest(s.as_bytes()).iter().map(|b| format!("{b:02x}")).collect();
        (s, hash)
    }
}
