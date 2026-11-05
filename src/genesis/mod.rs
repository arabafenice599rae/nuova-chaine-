//! Configurazione (TOML) e costruzione dello stato iniziale.

use std::path::PathBuf;
use std::str::FromStr;

use serde::Deserialize;
use solana_account::{Account, ReadableAccount};
use solana_address::Address;
use solana_hash::Hash;
use solana_keypair::Keypair;
use solana_message::{Message, VersionedMessage};
use solana_signer::Signer;
use solana_transaction::versioned::VersionedTransaction;

use crate::exec::litesvm_internal::Svm;
use crate::exec::{Chain, Params};

pub const LAMPORTS_PER_CNFA: u64 = 1_000_000_000;
pub const SUPPLY_LAMPORTS: u64 = 100_000_000 * LAMPORTS_PER_CNFA;

/// Id del programma timelock (fisso nel genesis): sha256("centrifuga:timelock").
pub fn timelock_program_id() -> Address {
    use sha2::{Digest, Sha256};
    Address::new_from_array(Sha256::digest(b"centrifuga:timelock").into())
}

pub fn vault_address(name: &str) -> Address {
    Address::find_program_address(&[b"vault", name.as_bytes()], &timelock_program_id()).0
}

/// Layout del vault (condiviso con programs/timelock):
/// beneficiario(32) totale(8) inizio(8) fine(8) prelevato(8).
pub const VAULT_LEN: usize = 64;

pub fn vault_data(beneficiary: &Address, total: u64, start: u64, end: u64) -> Vec<u8> {
    let mut d = Vec::with_capacity(VAULT_LEN);
    d.extend_from_slice(beneficiary.as_ref());
    for x in [total, start, end, 0] {
        d.extend_from_slice(&x.to_le_bytes());
    }
    d
}

#[derive(Clone, Debug, Deserialize)]
pub struct Config {
    pub node: NodeConfig,
    pub genesis: GenesisConfig,
}

#[derive(Clone, Debug, Deserialize)]
pub struct NodeConfig {
    pub token_name: String,
    pub token_symbol: String,
    pub slot_ms: u64,
    pub data_dir: PathBuf,
    pub operator_keypair: PathBuf,
    pub snapshot_every: u64,
    pub max_block_txs: usize,
    /// Indirizzo HTTP del JSON-RPC; il WebSocket usa la porta successiva.
    pub rpc_addr: String,
    /// Registra ogni metodo RPC chiamato (per vedere cosa chiedono i wallet).
    #[serde(default)]
    pub rpc_trace: bool,
}

#[derive(Clone, Debug, Deserialize)]
pub struct GenesisConfig {
    pub timestamp_ms: u64,
    pub operator: String,
    pub lamports_per_signature: u64,
    pub blockhash_window: usize,
    pub users_cnfa: u64,
    pub operator_cnfa: u64,
    /// Sblocco lineare del vault operatore: blocchi dal genesis.
    pub operator_unlock_blocks: u64,
    /// Riserva bloccata fino a questa altezza (inizio = fine).
    pub reserve_unlock_height: u64,
    pub block_budget_lamports: u64,
    pub halving_blocks: u64,
    pub fee_mining_end_height: u64,
    /// Solo devnet/test: conti pre-finanziati, sottratti alla riserva.
    #[serde(default)]
    pub prefund: Vec<Prefund>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct Prefund {
    pub pubkey: String,
    pub lamports: u64,
}

pub fn parse_address(s: &str) -> Result<Address, String> {
    Address::from_str(s).map_err(|e| format!("indirizzo '{s}': {e}"))
}

impl Config {
    pub fn load(path: &std::path::Path) -> Result<Config, String> {
        let s = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
        toml::from_str(&s).map_err(|e| format!("{}: {e}", path.display()))
    }
}

impl GenesisConfig {
    pub fn params(&self) -> Result<Params, String> {
        Ok(Params {
            operator: parse_address(&self.operator)?,
            lamports_per_signature: self.lamports_per_signature,
            blockhash_window: self.blockhash_window,
        })
    }
}

pub struct Summary {
    pub system_lamports: u64,
    pub reserve_lamports: u64,
}

/// Costruisce lo stato iniziale e verifica la conservazione: somma esatta = 100M CnFA.
pub fn build(g: &GenesisConfig) -> Result<(Chain, Summary), String> {
    let params = g.params()?;
    let mut svm = Svm::genesis(g.lamports_per_signature);
    // Account di sistema creati da litesvm: sysvar, builtin, feature, programmi.
    let system: u64 = svm.accounts().values().map(|a| a.lamports()).sum();
    let rent = svm.rent_exempt(VAULT_LEN);
    let mut prefund = Vec::new();
    for p in &g.prefund {
        prefund.push((parse_address(&p.pubkey)?, p.lamports));
    }
    let users = g.users_cnfa.checked_mul(LAMPORTS_PER_CNFA).ok_or("users_cnfa")?;
    let operator = g.operator_cnfa.checked_mul(LAMPORTS_PER_CNFA).ok_or("operator_cnfa")?;
    let reserve = prefund
        .iter()
        .try_fold(SUPPLY_LAMPORTS, |acc, (_, l)| acc.checked_sub(*l))
        .and_then(|x| x.checked_sub(system)?.checked_sub(3 * rent)?.checked_sub(users)?.checked_sub(operator))
        .ok_or("allocazioni oltre la supply")?;

    let tl = timelock_program_id();
    let none = Address::default();
    for (name, benef, total, start, end) in [
        ("users", none, users, 0, 0),
        ("operator", params.operator, operator, 0, g.operator_unlock_blocks),
        ("reserve", params.operator, reserve, g.reserve_unlock_height, g.reserve_unlock_height),
    ] {
        let data = vault_data(&benef, total, start, end);
        svm.set(vault_address(name), Account { lamports: total + rent, data, owner: tl, executable: false, rent_epoch: u64::MAX });
    }
    for (k, l) in prefund {
        svm.set(k, Account { lamports: l, owner: solana_sdk_ids::system_program::id(), ..Account::default() });
    }
    let chain = Chain::genesis(svm, params, g.timestamp_ms);
    let total = chain.total_lamports();
    if total != SUPPLY_LAMPORTS as u128 {
        return Err(format!("conservazione violata: {total} != {SUPPLY_LAMPORTS}"));
    }
    Ok((chain, Summary { system_lamports: system + 3 * rent, reserve_lamports: reserve }))
}

/// Determinismo: costruisce due volte il genesis (più un conto di test), produce
/// `n` blocchi con trasferimenti sul primo, li riesegue sul secondo e confronta.
pub fn check_determinism(g: &GenesisConfig, n: u64, slot_ms: u64) -> Result<Hash, String> {
    let op = Keypair::new();
    let payer = Keypair::new_from_array([7; 32]);
    let mut g = g.clone();
    g.operator = op.pubkey().to_string();
    g.prefund.push(Prefund { pubkey: payer.pubkey().to_string(), lamports: LAMPORTS_PER_CNFA });
    let (mut a, _) = build(&g)?;
    let (mut b, _) = build(&g)?;
    if a.last_hash != b.last_hash {
        return Err("genesis non deterministico".into());
    }
    for i in 1..=n {
        let to = Address::new_from_array([i as u8; 32]);
        let tx = transfer_tx(&payer, &to, 1_000_000 + i, a.last_hash);
        let ex = a.produce(vec![tx], g.timestamp_ms + i * slot_ms, &op);
        let ey = b.apply(&ex.block)?;
        if ey.block.hash() != ex.block.hash() {
            return Err(format!("blocco {i}: hash diverso"));
        }
    }
    if a.total_lamports() != SUPPLY_LAMPORTS as u128 {
        return Err("conservazione violata dopo i blocchi di test".into());
    }
    Ok(a.last_hash)
}

pub fn transfer_tx(from: &Keypair, to: &Address, lamports: u64, blockhash: Hash) -> VersionedTransaction {
    let ix = solana_system_interface::instruction::transfer(&from.pubkey(), to, lamports);
    let msg = Message::new_with_blockhash(&[ix], Some(&from.pubkey()), &blockhash);
    VersionedTransaction::try_new(VersionedMessage::Legacy(msg), &[from]).expect("sign")
}
