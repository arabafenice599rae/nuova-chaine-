//! Codifica JSON di account e transazioni nel formato del JSON-RPC Solana.

use base64::Engine;
use serde_json::{Value, json};
use solana_account::{AccountSharedData, ReadableAccount};
use solana_address::Address;
use solana_message::VersionedMessage;
use solana_transaction::versioned::VersionedTransaction;

use crate::exec::litesvm_internal::Svm;

pub const TOKEN: &str = "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA";
pub const TOKEN_2022: &str = "TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb";

pub fn b58(x: impl AsRef<[u8]>) -> String {
    bs58::encode(x).into_string()
}

pub fn b64(x: impl AsRef<[u8]>) -> String {
    base64::engine::general_purpose::STANDARD.encode(x)
}

pub fn token_program(owner: &Address) -> Option<&'static str> {
    match owner.to_string().as_str() {
        TOKEN => Some("spl-token"),
        TOKEN_2022 => Some("spl-token-2022"),
        _ => None,
    }
}

fn u64_at(d: &[u8], o: usize) -> u64 {
    u64::from_le_bytes(d[o..o + 8].try_into().unwrap())
}

/// COption<Pubkey>: tag u32 + 32 byte.
fn opt_key(d: &[u8], o: usize) -> Value {
    if d[o] == 1 { json!(b58(&d[o + 4..o + 36])) } else { Value::Null }
}

fn is_token_account(d: &[u8]) -> bool {
    d.len() == 165 || (d.len() > 165 && d[165] == 2)
}

fn is_mint(d: &[u8]) -> bool {
    d.len() == 82 || (d.len() > 165 && d[165] == 1)
}

/// Importo in unità: 12500, 3 -> "12.5".
pub fn ui_string(amount: u64, decimals: u8) -> String {
    let p = 10u128.pow(decimals as u32);
    let (int, frac) = (amount as u128 / p, amount as u128 % p);
    if frac == 0 {
        return int.to_string();
    }
    let f = format!("{frac:0w$}", w = decimals as usize);
    format!("{int}.{}", f.trim_end_matches('0'))
}

pub fn ui_amount(amount: u64, decimals: u8) -> Value {
    let s = ui_string(amount, decimals);
    json!({"amount": amount.to_string(), "decimals": decimals, "uiAmount": s.parse::<f64>().ok(), "uiAmountString": s})
}

/// (mint, owner, amount) di un token account SPL.
pub fn token_account(a: &AccountSharedData) -> Option<(Address, Address, u64)> {
    let d = a.data();
    (token_program(a.owner()).is_some() && is_token_account(d)).then(|| {
        (Address::try_from(&d[..32]).unwrap(), Address::try_from(&d[32..64]).unwrap(), u64_at(d, 64))
    })
}

pub fn mint_decimals(svm: &Svm, mint: &Address) -> Option<u8> {
    let m = svm.get(mint)?;
    (token_program(m.owner()).is_some() && is_mint(m.data())).then(|| m.data()[44])
}

/// jsonParsed per token account e mint; None = non interpretabile (si usa base64).
fn parsed(svm: &Svm, a: &AccountSharedData) -> Option<Value> {
    let program = token_program(a.owner())?;
    let d = a.data();
    let (kind, info) = if is_token_account(d) {
        let (mint, owner, amount) = token_account(a)?;
        let dec = mint_decimals(svm, &mint)?;
        let state = ["uninitialized", "initialized", "frozen"].get(d[108] as usize)?;
        let mut info = json!({
            "isNative": d[109] == 1, "mint": mint.to_string(), "owner": owner.to_string(),
            "state": state, "tokenAmount": ui_amount(amount, dec),
        });
        if d[72] == 1 {
            info["delegate"] = opt_key(d, 72);
            info["delegatedAmount"] = ui_amount(u64_at(d, 121), dec);
        }
        if d[129] == 1 {
            info["closeAuthority"] = opt_key(d, 129);
        }
        ("account", info)
    } else if is_mint(d) {
        let info = json!({
            "mintAuthority": opt_key(d, 0), "supply": u64_at(d, 36).to_string(), "decimals": d[44],
            "isInitialized": d[45] == 1, "freezeAuthority": opt_key(d, 46),
        });
        ("mint", info)
    } else {
        return None;
    };
    Some(json!({"program": program, "parsed": {"type": kind, "info": info}, "space": d.len()}))
}

/// Oggetto account. `enc`: base64 | base58 | jsonParsed | assente (= base58 legacy).
pub fn account(svm: &Svm, a: &AccountSharedData, enc: Option<&str>, slice: Option<(usize, usize)>) -> Result<Value, String> {
    let full = a.data();
    let d = match slice {
        Some((o, l)) => &full[o.min(full.len())..(o + l).min(full.len())],
        None => full,
    };
    let data = match enc {
        None | Some("binary") => json!(b58(d)),
        Some("base58") => json!([b58(d), "base58"]),
        Some("base64") => json!([b64(d), "base64"]),
        Some("jsonParsed") => parsed(svm, a).unwrap_or_else(|| json!([b64(d), "base64"])),
        Some(e) => return Err(format!("unsupported encoding: {e}")),
    };
    Ok(json!({
        "lamports": a.lamports(), "owner": a.owner().to_string(), "data": data,
        "executable": a.executable(), "rentEpoch": a.rent_epoch(), "space": full.len(),
    }))
}

pub fn version(m: &VersionedMessage) -> Value {
    match m {
        VersionedMessage::Legacy(_) => json!("legacy"),
        VersionedMessage::V0(_) => json!(0),
        VersionedMessage::V1(_) => json!(1),
    }
}

/// Transazione in codifica "json" o, con `parsed`, "jsonParsed" nella forma
/// parzialmente decodificata (istruzioni con programId e data in base58).
pub fn tx_json(tx: &VersionedTransaction, parsed: bool) -> Value {
    let m = &tx.message;
    let h = m.header();
    let keys = m.static_account_keys();
    let key = |i: u8| keys.get(i as usize).map_or(String::new(), |k| k.to_string());
    let ixs: Vec<Value> = m
        .instructions()
        .iter()
        .map(|ix| match parsed {
            false => json!({"programIdIndex": ix.program_id_index, "accounts": ix.accounts, "data": b58(&ix.data), "stackHeight": null}),
            true => json!({
                "programId": key(ix.program_id_index), "accounts": ix.accounts.iter().map(|i| key(*i)).collect::<Vec<_>>(),
                "data": b58(&ix.data), "stackHeight": null,
            }),
        })
        .collect();
    let account_keys: Vec<Value> = keys
        .iter()
        .enumerate()
        .map(|(i, k)| match parsed {
            false => json!(k.to_string()),
            true => json!({
                "pubkey": k.to_string(), "signer": m.is_signer(i), "source": "transaction",
                "writable": m.is_maybe_writable_with_reserved_addresses(i, None::<&std::collections::HashSet<Address>>),
            }),
        })
        .collect();
    let mut msg = json!({
        "accountKeys": account_keys,
        "header": {
            "numRequiredSignatures": h.num_required_signatures,
            "numReadonlySignedAccounts": h.num_readonly_signed_accounts,
            "numReadonlyUnsignedAccounts": h.num_readonly_unsigned_accounts,
        },
        "recentBlockhash": m.recent_blockhash().to_string(),
        "instructions": ixs,
    });
    if let Some(l) = m.address_table_lookups() {
        msg["addressTableLookups"] = l
            .iter()
            .map(|l| json!({"accountKey": l.account_key.to_string(), "writableIndexes": l.writable_indexes, "readonlyIndexes": l.readonly_indexes}))
            .collect();
    }
    json!({"signatures": tx.signatures.iter().map(|s| s.to_string()).collect::<Vec<_>>(), "message": msg})
}
