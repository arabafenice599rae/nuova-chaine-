//! `centrifuga wallet`: chiave, indirizzo, saldo (CnFA e token SPL), invii, cronologia.
//! Parla con un nodo via JSON-RPC (HTTPS verificato; http:// solo locale) e lo rifiuta
//! se chain_id o genesis hash non sono quelli fissati in `wallet.toml`.
//! Messaggi in inglese. Non stampa mai la chiave privata né "SOL".

use std::io::{BufRead, Write};
use std::path::PathBuf;
use std::time::{Duration, Instant};

use base64::Engine;
use serde::Deserialize;
use serde_json::json;
use solana_address::Address;
use solana_instruction::{AccountMeta, Instruction};
use solana_keypair::Keypair;
use solana_message::{Message, VersionedMessage};
use solana_signer::Signer;
use solana_transaction::versioned::VersionedTransaction;

use crate::client::{Tls, call_with, public_tls};
use crate::genesis::parse_address;
use crate::node::{fmt_amount, read_keypair};
use crate::rpc::encode::{TOKEN, TOKEN_2022, ui_string};

const ATA: &str = "ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL";
const USAGE: &str = "usage: centrifuga wallet <new|address|balance [address]|send <address> <amount>|
       token-send <mint> <address> <amount>|history [address]> [--rpc URL] [--keypair FILE]
       [--config wallet.toml] [--yes]";

/// `wallet.toml`: nodo, chiave e identità attesa della chain.
#[derive(Deserialize, Default)]
struct Conf {
    rpc: Option<String>,
    keypair: Option<PathBuf>,
    chain_id: Option<String>,
    genesis_hash: Option<String>,
}

struct Wallet {
    rpc: Option<String>,
    keypair: Option<PathBuf>,
    conf: Conf,
    yes: bool,
    tls: Tls,
}

/// Unica uscita del wallet: il token nativo è CnFA, mai "SOL".
fn say(s: impl AsRef<str>) {
    println!("{}", s.as_ref().replace("SOL", "CnFA"));
}

pub fn main(args: &[String]) -> Result<(), String> {
    run_with(args, public_tls())
}

/// Come `main`, con radici TLS date (solo i test: CA di prova o della CI).
pub fn run_with(args: &[String], tls: Tls) -> Result<(), String> {
    run(args, tls).map_err(|e| e.replace("SOL", "CnFA"))
}

fn run(args: &[String], tls: Tls) -> Result<(), String> {
    let (mut pos, mut rpc, mut keypair, mut config, mut yes) = (vec![], None, None, PathBuf::from("wallet.toml"), false);
    let mut it = args.iter();
    while let Some(a) = it.next() {
        let mut val = || it.next().cloned().ok_or(format!("{a}: missing value"));
        match a.as_str() {
            "--rpc" => rpc = Some(val()?),
            "--keypair" => keypair = Some(PathBuf::from(val()?)),
            "--config" => config = PathBuf::from(val()?),
            "--yes" => yes = true,
            _ => pos.push(a.as_str()),
        }
    }
    let conf: Conf = match std::fs::read_to_string(&config) {
        Ok(s) => toml::from_str(&s).map_err(|e| format!("{}: {e}", config.display()))?,
        Err(_) => Conf::default(),
    };
    let w = Wallet { rpc, keypair, conf, yes, tls };
    match pos.as_slice() {
        ["new"] => w.new_key(),
        ["address"] => {
            let _: () = say(w.key()?.pubkey().to_string());
            Ok(())
        }
        ["balance", rest @ ..] if rest.len() <= 1 => w.balance(rest.first().copied()),
        ["send", to, amount] => w.send(to, amount),
        ["token-send", mint, to, amount] => w.token_send(mint, to, amount),
        ["history", rest @ ..] if rest.len() <= 1 => w.history(rest.first().copied()),
        _ => Err(USAGE.into()),
    }
}

/// "12.5" con `decimals` decimali -> unità intere, senza virgola mobile.
fn parse_amount(s: &str, decimals: u8) -> Result<u64, String> {
    let (int, frac) = s.split_once('.').unwrap_or((s, ""));
    let bad = || format!("invalid amount: {s}");
    if frac.len() > decimals as usize || int.is_empty() && frac.is_empty() {
        return Err(bad());
    }
    let digits = format!("{int}{frac:0<w$}", w = decimals as usize);
    digits.parse::<u64>().ok().filter(|v| *v > 0).ok_or_else(bad)
}

fn b64(b: &[u8]) -> String {
    base64::engine::general_purpose::STANDARD.encode(b)
}

impl Wallet {
    fn key_path(&self) -> Result<PathBuf, String> {
        self.keypair.clone().or(self.conf.keypair.clone()).ok_or("--keypair required (or keypair in wallet.toml)".into())
    }

    fn key(&self) -> Result<Keypair, String> {
        read_keypair(&self.key_path()?)
    }

    /// Nuova chiave nel formato JSON di solana-keygen, permessi 600; mai sovrascritta.
    fn new_key(&self) -> Result<(), String> {
        use std::os::unix::fs::OpenOptionsExt;
        let path = self.key_path()?;
        let mut f = std::fs::OpenOptions::new().write(true).create_new(true).mode(0o600).open(&path).map_err(|e| match e.kind() {
            std::io::ErrorKind::AlreadyExists => format!("{} already exists: not overwriting it", path.display()),
            _ => format!("{}: {e}", path.display()),
        })?;
        let kp = Keypair::new();
        let bytes: Vec<String> = kp.to_bytes().iter().map(|b| b.to_string()).collect();
        f.write_all(format!("[{}]", bytes.join(",")).as_bytes()).map_err(|e| e.to_string())?;
        say(format!("address: {}\nkey saved to {} (mode 600)", kp.pubkey(), path.display()));
        Ok(())
    }

    fn call(&self, url: &str, method: &str, params: serde_json::Value) -> Result<serde_json::Value, String> {
        call_with(&self.tls, url, method, params)
    }

    /// URL del nodo, dopo aver verificato URL, chain_id e genesis hash fissati nella config.
    fn node(&self) -> Result<String, String> {
        let url = self.rpc.clone().or(self.conf.rpc.clone()).ok_or("--rpc required (or rpc in wallet.toml)")?;
        let (Some(id), Some(gh)) = (&self.conf.chain_id, &self.conf.genesis_hash) else {
            return Err("wallet.toml must set chain_id and genesis_hash".into());
        };
        crate::client::endpoint(&url)?;
        let info = self.call(&url, "getChainInfo", json!([]))?;
        if info["chainId"] != id.as_str() || info["genesisHash"] != gh.as_str() {
            let got = |k: &str| info[k].as_str().unwrap_or("?").to_string();
            return Err(format!("node rejected: it is {} {} (expected {id} {gh})", got("chainId"), got("genesisHash")));
        }
        Ok(url)
    }

    fn owner(&self, addr: Option<&str>) -> Result<Address, String> {
        match addr {
            Some(a) => parse_address(a),
            None => Ok(self.key()?.pubkey()),
        }
    }

    fn balance(&self, addr: Option<&str>) -> Result<(), String> {
        let (url, owner) = (self.node()?, self.owner(addr)?);
        let lamports = self.call(&url, "getBalance", json!([owner.to_string()]))?["value"].as_u64().unwrap_or(0);
        say(format!("{owner}: {}", fmt_amount(lamports, "CnFA")));
        for program in [TOKEN, TOKEN_2022] {
            let r = self.call(&url, "getTokenAccountsByOwner", json!([owner.to_string(), {"programId": program}, {"encoding": "jsonParsed"}]))?;
            for a in r["value"].as_array().into_iter().flatten() {
                let info = &a["account"]["data"]["parsed"]["info"];
                say(format!("  {} token {}", info["tokenAmount"]["uiAmountString"].as_str().unwrap_or("?"), info["mint"].as_str().unwrap_or("?")));
            }
        }
        Ok(())
    }

    fn history(&self, addr: Option<&str>) -> Result<(), String> {
        let (url, owner) = (self.node()?, self.owner(addr)?);
        let r = self.call(&url, "getSignaturesForAddress", json!([owner.to_string(), {"limit": 10}])).map_err(|e| {
            // Errore del nodo senza storico: si mostra il suo motivo (indexer assente o irraggiungibile).
            let v: serde_json::Value = e.split_once(": ").and_then(|(_, j)| serde_json::from_str(j).ok()).unwrap_or_default();
            match v["code"].as_i64() {
                Some(crate::rpc::methods::NO_HISTORY) => format!("history not available on this node: {}", v["message"].as_str().unwrap_or("")),
                _ => e,
            }
        })?;
        let list = r.as_array().cloned().unwrap_or_default();
        if list.is_empty() {
            say("no transactions");
        }
        for s in list {
            let outcome = if s["err"].is_null() { "ok" } else { "failed" };
            say(format!("block {}  {}  {outcome}", s["slot"], s["signature"].as_str().unwrap_or("")));
        }
        Ok(())
    }

    fn send(&self, to: &str, amount: &str) -> Result<(), String> {
        let (url, kp, to) = (self.node()?, self.key()?, parse_address(to)?);
        let lamports = parse_amount(amount, 9)?;
        let ix = solana_system_interface::instruction::transfer(&kp.pubkey(), &to, lamports);
        let lines = vec![format!("to: {to}"), format!("amount: {}", fmt_amount(lamports, "CnFA"))];
        self.submit(&url, &kp, vec![ix], lines, lamports)
    }

    fn token_send(&self, mint: &str, to: &str, amount: &str) -> Result<(), String> {
        let (url, kp, mint, to) = (self.node()?, self.key()?, parse_address(mint)?, parse_address(to)?);
        let acc = |a: &Address| self.call(&url, "getAccountInfo", json!([a.to_string(), {"encoding": "jsonParsed"}])).map(|v| v["value"].clone());
        let m = acc(&mint)?;
        let program = parse_address(m["owner"].as_str().ok_or("mint not found")?)?;
        let decimals = m["data"]["parsed"]["info"]["decimals"].as_u64().ok_or("not an SPL mint")? as u8;
        let units = parse_amount(amount, decimals)?;
        let mine = self.call(&url, "getTokenAccountsByOwner", json!([kp.pubkey().to_string(), {"mint": mint.to_string()}, {"encoding": "jsonParsed"}]))?;
        let source = mine["value"].as_array().and_then(|v| v.first()).and_then(|a| a["pubkey"].as_str()).ok_or("no account holds this token")?;
        let (source, ata_program) = (parse_address(source)?, parse_address(ATA)?);
        let dest = Address::find_program_address(&[to.as_ref(), program.as_ref(), mint.as_ref()], &ata_program).0;
        let mut ixs = vec![];
        let mut lines = vec![format!("to: {to}"), format!("amount: {} token {mint}", ui_string(units, decimals))];
        let mut out = 0;
        if acc(&dest)?.is_null() {
            out = self.call(&url, "getMinimumBalanceForRentExemption", json!([165]))?.as_u64().unwrap_or(0);
            lines.push(format!("creates the recipient's token account ({} deposit)", fmt_amount(out, "CnFA")));
            let metas = vec![
                AccountMeta::new(kp.pubkey(), true),
                AccountMeta::new(dest, false),
                AccountMeta::new_readonly(to, false),
                AccountMeta::new_readonly(mint, false),
                AccountMeta::new_readonly(solana_sdk_ids::system_program::id(), false),
                AccountMeta::new_readonly(program, false),
            ];
            ixs.push(Instruction { program_id: ata_program, accounts: metas, data: vec![1] }); // CreateIdempotent
        }
        let mut data = vec![12]; // TransferChecked
        data.extend_from_slice(&units.to_le_bytes());
        data.push(decimals);
        let metas = vec![
            AccountMeta::new(source, false),
            AccountMeta::new_readonly(mint, false),
            AccountMeta::new(dest, false),
            AccountMeta::new_readonly(kp.pubkey(), true),
        ];
        ixs.push(Instruction { program_id: program, accounts: metas, data });
        self.submit(&url, &kp, ixs, lines, out)
    }

    /// Riepilogo, conferma, invio, attesa del blocco e costo netto.
    /// `out`: lamport che lasciano il conto oltre alla fee (importo o deposito).
    fn submit(&self, url: &str, kp: &Keypair, ixs: Vec<Instruction>, lines: Vec<String>, out: u64) -> Result<(), String> {
        let bh = self.call(url, "getLatestBlockhash", json!([]))?["value"]["blockhash"].as_str().ok_or("blockhash")?.parse().map_err(|_| "blockhash")?;
        let msg = Message::new_with_blockhash(&ixs, Some(&kp.pubkey()), &bh);
        let fee = self.call(url, "getFeeForMessage", json!([b64(&msg.serialize())]))?["value"].as_u64().ok_or("fee not available")?;
        for l in &lines {
            say(l);
        }
        say(format!("fee: {}", fmt_amount(fee, "CnFA")));
        if !self.yes {
            print!("send? [y/N] ");
            std::io::stdout().flush().ok();
            let mut answer = String::new();
            std::io::stdin().lock().read_line(&mut answer).ok();
            if !matches!(answer.trim().to_lowercase().as_str(), "y" | "yes") {
                return Err("cancelled".into());
            }
        }
        let balance = || self.call(url, "getBalance", json!([kp.pubkey().to_string()])).map(|v| v["value"].as_u64().unwrap_or(0));
        let before = balance()?;
        let tx = VersionedTransaction::try_new(VersionedMessage::Legacy(msg), &[kp]).map_err(|e| e.to_string())?;
        let raw = b64(&wincode::serialize(&tx).map_err(|e| e.to_string())?);
        let sig = self.call(url, "sendTransaction", json!([raw, {"encoding": "base64"}]))?.as_str().unwrap_or("").to_string();
        let start = Instant::now();
        let status = loop {
            let st = self.call(url, "getSignatureStatuses", json!([[sig]]))?["value"][0].clone();
            if !st.is_null() {
                break st;
            }
            if start.elapsed() > Duration::from_secs(30) {
                return Err(format!("transaction {sig} not included within 30 s"));
            }
            std::thread::sleep(Duration::from_millis(100));
        };
        if !status["err"].is_null() {
            return Err(format!("transaction {sig} failed: {}", status["err"]));
        }
        let paid = self.call(url, "getTransaction", json!([sig]))?["meta"]["fee"].as_u64().unwrap_or(fee);
        let after = balance()?;
        // after = before - out - paid + rimborso + premio (premio e rimborso a fine blocco).
        let refund = paid.saturating_sub(fee);
        let reward = (after + out + paid).saturating_sub(before + refund);
        say(format!("sent in block {}: {sig}", status["slot"]));
        say(format!(
            "fee {}, reward {}, refund {}, net cost {}",
            fmt_amount(paid, "CnFA"),
            fmt_amount(reward, "CnFA"),
            fmt_amount(refund, "CnFA"),
            fmt_amount(before.saturating_sub(after + out), "CnFA")
        ));
        Ok(())
    }
}
