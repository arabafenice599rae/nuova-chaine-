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
use crate::exec::{Chain, Identity, Params, Rules, Vaults};

pub const LAMPORTS_PER_CNFA: u64 = 1_000_000_000;
pub const SUPPLY_LAMPORTS: u64 = 100_000_000 * LAMPORTS_PER_CNFA;

/// Programma timelock compilato (`programs/timelock`, sha256 nel README).
pub const TIMELOCK_SO: &[u8] = include_bytes!("../../programs/timelock/centrifuga_timelock.so");

/// Id del programma timelock (fisso nel genesis): sha256("centrifuga:timelock").
pub fn timelock_program_id() -> Address {
    use sha2::{Digest, Sha256};
    Address::new_from_array(Sha256::digest(b"centrifuga:timelock").into())
}

pub fn vault_address(name: &str) -> Address {
    Address::find_program_address(&[b"vault", name.as_bytes()], &timelock_program_id()).0
}

/// Vault nel genesis: airdrop, mining, operator, reserve.
pub const VAULTS: u64 = 4;
pub const VAULT_NAMES: [&str; 4] = ["airdrop", "mining", "operator", "reserve"];

/// Layout del vault: definito dal programma timelock (`centrifuga_timelock`).
pub use centrifuga_timelock::VAULT_LEN;

pub fn vault_data(kind: u8, beneficiary: &Address, authority: &Address, total: u64, start: u64, end: u64) -> Vec<u8> {
    let mut d = vec![centrifuga_timelock::VAULT_TAG, kind];
    d.extend_from_slice(beneficiary.as_ref());
    d.extend_from_slice(authority.as_ref());
    for x in [total, start, end, 0] {
        d.extend_from_slice(&x.to_le_bytes());
    }
    debug_assert_eq!(d.len(), VAULT_LEN);
    d
}

#[derive(Clone, Debug, Deserialize)]
pub struct Config {
    pub node: NodeConfig,
    pub genesis: GenesisConfig,
}

#[derive(Clone, Debug, Deserialize)]
pub struct NodeConfig {
    /// URL del logo pubblicato in getChainInfo e /chain.json (non fa parte del genesis).
    #[serde(default)]
    pub logo_url: String,
    pub data_dir: PathBuf,
    pub operator_keypair: PathBuf,
    pub snapshot_every: u64,
    pub max_block_txs: usize,
    /// Indirizzo HTTP del JSON-RPC; il WebSocket usa la porta successiva.
    pub rpc_addr: String,
    /// Registra in `data_dir/rpc.log` ogni metodo RPC/WebSocket ricevuto, con l'esito.
    #[serde(default)]
    pub rpc_log: bool,
    /// Indirizzo dello stream (WebSocket): header firmato + delta a ogni blocco.
    pub stream_addr: String,
    /// Un client dello stream indietro di più blocchi di così viene disconnesso.
    pub stream_max_lag_blocks: usize,
    /// Blocchi recuperabili con `from_height` come delta; oltre, stato completo.
    pub stream_catchup_blocks: usize,
    /// Ingresso: tx al secondo per IP e tx in coda per pagatore.
    pub ingress_tps_per_ip: u32,
    pub ingress_pending_per_payer: u32,
}

#[derive(Clone, Debug, Deserialize)]
pub struct GenesisConfig {
    /// "mainnet" o "devnet". Solo devnet ammette `prefund`.
    pub network: String,
    /// Identità (nel genesis hash): chain_id = "centrifuga:<network>", simbolo "CnFA".
    pub chain_id: String,
    pub chain_name: String,
    pub symbol: String,
    pub decimals: u8,
    pub timestamp_ms: u64,
    pub operator: String,
    /// Durata di uno slot. Regola: timestamp - precedente >= salto × slot_ms / 2.
    pub slot_ms: u64,
    /// Salto massimo di altezza tra due blocchi; senza tx, un blocco ogni heartbeat_slots slot.
    pub heartbeat_slots: u64,
    pub lamports_per_signature: u64,
    pub blockhash_window: usize,
    /// Vault "airdrop": claim degli aventi diritto (e residuo del fee mining).
    pub airdrop_cnfa: u64,
    /// Vault "mining": premi del fee mining.
    pub mining_cnfa: u64,
    pub operator_cnfa: u64,
    /// Sblocco lineare del vault operatore: blocchi dal genesis.
    pub operator_unlock_blocks: u64,
    /// Riserva bloccata fino a questa altezza (inizio = fine).
    pub reserve_unlock_height: u64,
    /// Premio massimo per blocco all'altezza 0; dimezza ogni `halving_blocks`.
    pub block_budget_lamports: u64,
    pub halving_blocks: u64,
    /// Fine del fee mining (24 mesi), se il vault "mining" non si esaurisce prima.
    pub fee_mining_end_height: u64,
    /// Scadenza dei claim su "airdrop": da qui `to_reserve` sposta il residuo in "reserve".
    pub claims_end_height: u64,
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
            block_budget: self.block_budget_lamports,
            halving_blocks: self.halving_blocks,
            mining_end_height: self.fee_mining_end_height,
            vaults: Vaults {
                mining: vault_address("mining"),
                airdrop: vault_address("airdrop"),
                reserve: vault_address("reserve"),
            },
            rules: Rules { slot_ms: self.slot_ms, heartbeat_slots: self.heartbeat_slots },
            identity: Identity {
                chain_id: self.chain_id.clone(),
                name: self.chain_name.clone(),
                symbol: self.symbol.clone(),
                decimals: self.decimals,
            },
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
    match g.network.as_str() {
        "devnet" => {}
        "mainnet" if g.prefund.is_empty() => {}
        "mainnet" => return Err("prefund ammesso solo con network = \"devnet\"".into()),
        n => return Err(format!("network sconosciuta: {n} (mainnet | devnet)")),
    }
    // Identità: simbolo esatto (maiuscole e minuscole contano), chain_id coerente con network.
    if g.symbol != "CnFA" {
        return Err(format!("simbolo \"{}\": deve essere esattamente \"CnFA\"", g.symbol));
    }
    if g.chain_id != format!("centrifuga:{}", g.network) {
        return Err(format!("chain_id \"{}\" non corrisponde a network = \"{}\"", g.chain_id, g.network));
    }
    if g.decimals != 9 || g.chain_name.is_empty() {
        return Err("servono decimals = 9 (i lamport) e un nome".into());
    }
    if g.slot_ms < 2 || g.heartbeat_slots == 0 {
        return Err("servono slot_ms >= 2 e heartbeat_slots >= 1".into());
    }
    if g.claims_end_height < g.fee_mining_end_height {
        return Err("claims_end_height deve seguire fee_mining_end_height".into());
    }
    let mut svm = Svm::genesis(g.lamports_per_signature);
    svm.add_program(timelock_program_id(), TIMELOCK_SO)?;
    // Account di sistema: sysvar, builtin, feature, programmi (timelock incluso).
    let system: u64 = svm.accounts().values().map(|a| a.lamports()).sum();
    let rent = svm.rent_exempt(VAULT_LEN);
    let mut prefund = Vec::new();
    for p in &g.prefund {
        prefund.push((parse_address(&p.pubkey)?, p.lamports));
    }
    let cnfa = |x: u64, n: &str| x.checked_mul(LAMPORTS_PER_CNFA).ok_or(format!("{n} troppo grande"));
    let (airdrop, mining) = (cnfa(g.airdrop_cnfa, "airdrop_cnfa")?, cnfa(g.mining_cnfa, "mining_cnfa")?);
    let operator = cnfa(g.operator_cnfa, "operator_cnfa")?;
    let reserve = prefund
        .iter()
        .try_fold(SUPPLY_LAMPORTS, |acc, (_, l)| acc.checked_sub(*l))
        .and_then(|x| x.checked_sub(system)?.checked_sub(VAULTS * rent)?.checked_sub(airdrop)?.checked_sub(mining)?.checked_sub(operator))
        .ok_or("allocazioni oltre la supply")?;

    let tl = timelock_program_id();
    let none = Address::default();
    use centrifuga_timelock::{AIRDROP, MINING, OPERATOR, RESERVE};
    let op = params.operator;
    // (nome, tipo, beneficiario, autorità dei claim, totale, inizio, fine).
    // Solo "operator" ha un beneficiario (withdraw); "reserve" esce solo via claim.
    for (name, kind, benef, auth, total, start, end) in [
        ("airdrop", AIRDROP, none, op, airdrop, 0, g.claims_end_height),
        ("mining", MINING, none, none, mining, 0, 0),
        ("operator", OPERATOR, op, none, operator, 0, g.operator_unlock_blocks),
        ("reserve", RESERVE, none, op, reserve, g.reserve_unlock_height, g.reserve_unlock_height),
    ] {
        let data = vault_data(kind, &benef, &auth, total, start, end);
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
    Ok((chain, Summary { system_lamports: system + VAULTS * rent, reserve_lamports: reserve }))
}

/// Determinismo: costruisce due volte il genesis (più un conto di test), produce
/// `n` blocchi con trasferimenti sul primo, li riesegue sul secondo e confronta.
pub fn check_determinism(g: &GenesisConfig, n: u64) -> Result<Hash, String> {
    let op = Keypair::new();
    let payer = Keypair::new_from_array([7; 32]);
    let mut g = g.clone();
    // Copia di prova, mai pubblicata: può usare un conto prefinanziato.
    g.network = "devnet".into();
    g.chain_id = "centrifuga:devnet".into();
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
        let ex = a.produce(vec![tx], g.timestamp_ms + i * g.slot_ms, &op);
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
