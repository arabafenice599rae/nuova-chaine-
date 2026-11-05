//! Metodi JSON-RPC. Girano sul thread del nodo, tra un blocco e l'altro:
//! nessun lock, stato sempre coerente con l'ultimo blocco.

use std::str::FromStr;

use base64::Engine;
use serde_json::{Value, json};
use solana_account::ReadableAccount;
use solana_address::Address;
use solana_hash::Hash;
use solana_message::VersionedMessage;
use solana_signature::Signature;
use solana_transaction::versioned::VersionedTransaction;

use super::encode::{self, b58, b64};
use super::index::TxRecord;
use crate::genesis::parse_address;
use crate::node::Node;

/// Versione dell'SVM su cui gira litesvm 0.18.2 (agave 4.3.x).
const SVM_VERSION: &str = "4.3.2";
/// Metodi supportati. Tutto il resto risponde -32601 e viene registrato nel log.
pub const METHODS: &[&str] = &[
    "getBalance", "getAccountInfo", "getMultipleAccounts", "getTokenAccountsByOwner", "getLatestBlockhash",
    "getBlockHeight", "getSlot", "getFeeForMessage", "simulateTransaction", "sendTransaction",
    "getSignatureStatuses", "getTransaction", "getVersion", "getHealth", "getMinimumBalanceForRentExemption",
    "getSignaturesForAddress", "getTokenAccountBalance", "getRecentPrioritizationFees", "isBlockhashValid",
    "getEpochInfo", "getGenesisHash", "getChainInfo",
];

/// Identità della chain (getChainInfo e GET /chain.json): dal genesis, tranne il logo.
pub fn chain_info(n: &Node) -> Value {
    let id = &n.chain.params.identity;
    json!({
        "chainId": id.chain_id, "name": id.name, "symbol": id.symbol, "decimals": id.decimals,
        "genesisHash": n.chain.genesis_hash.to_string(), "logoUrl": n.cfg.node.logo_url,
    })
}

pub struct RpcError {
    pub code: i64,
    pub message: String,
    pub data: Value,
}

type R = Result<Value, RpcError>;

fn invalid(m: impl Into<String>) -> RpcError {
    RpcError { code: -32602, message: m.into(), data: Value::Null }
}

fn arg<T: serde::de::DeserializeOwned>(p: &Value, i: usize) -> Result<T, RpcError> {
    serde_json::from_value(p.get(i).cloned().unwrap_or(Value::Null)).map_err(|e| invalid(format!("parametro {i}: {e}")))
}

fn addr(p: &Value, i: usize) -> Result<Address, RpcError> {
    parse_address(&arg::<String>(p, i)?).map_err(invalid)
}

fn sig(s: &str) -> Result<Signature, RpcError> {
    Signature::from_str(s).map_err(|e| invalid(format!("firma: {e}")))
}

fn cfg(p: &Value, i: usize) -> &Value {
    p.get(i).unwrap_or(&Value::Null)
}

fn ctx(n: &Node, value: Value) -> Value {
    json!({"context": {"slot": n.chain.height, "apiVersion": SVM_VERSION}, "value": value})
}

fn decode(s: &str, enc: Option<&str>) -> Result<Vec<u8>, RpcError> {
    match enc.unwrap_or("base58") {
        "base58" => bs58::decode(s).into_vec().map_err(|e| invalid(e.to_string())),
        "base64" => base64::engine::general_purpose::STANDARD.decode(s).map_err(|e| invalid(e.to_string())),
        e => Err(invalid(format!("codifica non supportata: {e}"))),
    }
}

fn decode_tx(p: &Value) -> Result<VersionedTransaction, RpcError> {
    let bytes = decode(&arg::<String>(p, 0)?, cfg(p, 1)["encoding"].as_str())?;
    let tx: VersionedTransaction = wincode::deserialize(&bytes).map_err(|e| invalid(format!("tx: {e}")))?;
    tx.sanitize().map_err(|e| invalid(format!("tx: {e}")))?;
    Ok(tx)
}

fn account_opts(c: &Value) -> (Option<&str>, Option<(usize, usize)>) {
    let s = &c["dataSlice"];
    let slice = s["offset"].as_u64().zip(s["length"].as_u64()).map(|(o, l)| (o as usize, l as usize));
    (c["encoding"].as_str(), slice)
}

fn account_at(n: &Node, k: &Address, c: &Value) -> R {
    let (enc, slice) = account_opts(c);
    match n.chain.svm.get(k) {
        Some(a) => encode::account(&n.chain.svm, a, enc, slice).map_err(invalid),
        None => Ok(Value::Null),
    }
}

fn simulate(n: &Node, tx: VersionedTransaction) -> Value {
    match n.chain.svm.simulate(tx) {
        Ok(i) => json!({"err": null, "logs": i.meta.logs, "accounts": null, "unitsConsumed": i.meta.compute_units_consumed, "returnData": null}),
        Err(f) => json!({"err": f.err, "logs": f.meta.logs, "accounts": null, "unitsConsumed": f.meta.compute_units_consumed, "returnData": null}),
    }
}

fn status(r: &TxRecord) -> Value {
    let status = match r.err() {
        Some(e) => json!({"Err": e}),
        None => json!({"Ok": null}),
    };
    json!({"slot": r.slot, "confirmations": null, "err": r.err(), "status": status, "confirmationStatus": "finalized"})
}

fn transaction(r: &TxRecord, c: &Value) -> Value {
    let meta = match &r.result {
        Ok(m) => m,
        Err(f) => &f.meta,
    };
    let tx = match c["encoding"].as_str() {
        Some("base64") => json!([b64(crate::ledger::block::tx_bytes(&r.tx)), "base64"]),
        Some("base58") => json!([b58(crate::ledger::block::tx_bytes(&r.tx)), "base58"]),
        Some("jsonParsed") => encode::tx_json(&r.tx, true),
        _ => encode::tx_json(&r.tx, false),
    };
    let status = match r.err() {
        Some(e) => json!({"Err": e}),
        None => json!({"Ok": null}),
    };
    let mut out = json!({
        "slot": r.slot, "blockTime": r.time_ms / 1000, "version": encode::version(&r.tx.message), "transaction": tx,
        "meta": {
            "err": r.err(), "status": status, "fee": meta.fee, "preBalances": r.pre, "postBalances": r.post,
            "logMessages": meta.logs, "innerInstructions": [], "preTokenBalances": [], "postTokenBalances": [],
            "rewards": [], "computeUnitsConsumed": meta.compute_units_consumed,
            "loadedAddresses": {"writable": [], "readonly": []},
        },
    });
    // Campo extra: dall'arrivo della richiesta HTTP al blocco firmato e scritto (µs).
    if let Some(us) = r.node_us {
        out["centrifugaNodeLatencyUs"] = json!(us);
    }
    out
}

pub fn call(n: &mut Node, method: &str, p: &Value, peer: Option<std::net::IpAddr>, arrived: std::time::Instant) -> R {
    let height = n.chain.height;
    Ok(match method {
        "getHealth" => json!("ok"),
        "getSlot" | "getBlockHeight" => json!(height),
        "getGenesisHash" => json!(n.chain.genesis_hash.to_string()),
        "getChainInfo" => chain_info(n),
        "getVersion" => {
            let id = &n.chain.params.identity;
            json!({
                "solana-core": SVM_VERSION, "feature-set": 0, "centrifuga": env!("CARGO_PKG_VERSION"),
                "token": {"name": id.name, "symbol": id.symbol, "decimals": id.decimals},
                "chainId": id.chain_id,
            })
        }
        // Nessuna epoca: epoch 0 e slotsInEpoch = massimo intero sicuro in JavaScript.
        "getEpochInfo" => json!({
            "absoluteSlot": height, "blockHeight": height, "epoch": 0, "slotIndex": height,
            "slotsInEpoch": 9_007_199_254_740_991u64, "transactionCount": n.index.by_sig.len(),
        }),
        "getLatestBlockhash" => {
            let last_valid = height + n.chain.params.blockhash_window as u64;
            ctx(n, json!({"blockhash": n.chain.last_hash.to_string(), "lastValidBlockHeight": last_valid}))
        }
        "isBlockhashValid" => {
            let h = Hash::from_str(&arg::<String>(p, 0)?).map_err(|e| invalid(e.to_string()))?;
            ctx(n, json!(n.chain.blockhash_valid(&h)))
        }
        "getMinimumBalanceForRentExemption" => json!(n.chain.svm.rent_exempt(arg::<usize>(p, 0)?)),
        "getBalance" => ctx(n, json!(n.chain.svm.lamports(&addr(p, 0)?))),
        "getAccountInfo" => ctx(n, account_at(n, &addr(p, 0)?, cfg(p, 1))?),
        "getMultipleAccounts" => {
            let keys: Vec<String> = arg(p, 0)?;
            let v = keys.iter().map(|k| account_at(n, &parse_address(k).map_err(invalid)?, cfg(p, 1))).collect::<Result<Vec<_>, _>>()?;
            ctx(n, json!(v))
        }
        "getTokenAccountsByOwner" => {
            let owner = addr(p, 0)?;
            let f = cfg(p, 1);
            let mint = f["mint"].as_str().map(parse_address).transpose().map_err(invalid)?;
            let prog = f["programId"].as_str().map(parse_address).transpose().map_err(invalid)?;
            if mint.is_none() && prog.is_none() {
                return Err(invalid("serve il filtro mint o programId"));
            }
            let mut out = Vec::new();
            for (k, a) in n.chain.svm.accounts() {
                let Some((m, o, _)) = encode::token_account(a) else { continue };
                if o == owner && mint.is_none_or(|x| x == m) && prog.is_none_or(|x| &x == a.owner()) {
                    let (enc, slice) = account_opts(cfg(p, 2));
                    let acc = encode::account(&n.chain.svm, a, enc, slice).map_err(invalid)?;
                    out.push(json!({"pubkey": k.to_string(), "account": acc}));
                }
            }
            ctx(n, json!(out))
        }
        "getTokenAccountBalance" => {
            let a = n.chain.svm.get(&addr(p, 0)?).ok_or_else(|| invalid("account inesistente"))?;
            let (mint, _, amount) = encode::token_account(a).ok_or_else(|| invalid("non è un token account"))?;
            let dec = encode::mint_decimals(&n.chain.svm, &mint).ok_or_else(|| invalid("mint inesistente"))?;
            ctx(n, encode::ui_amount(amount, dec))
        }
        // La priority fee non cambia l'ordine ed è rimborsata: è sempre 0.
        "getRecentPrioritizationFees" => {
            json!((height.saturating_sub(149)..=height).map(|s| json!({"slot": s, "prioritizationFee": 0})).collect::<Vec<_>>())
        }
        "getFeeForMessage" => {
            let bytes = decode(&arg::<String>(p, 0)?, Some("base64"))?;
            let m: VersionedMessage = wincode::deserialize(&bytes).map_err(|e| invalid(format!("messaggio: {e}")))?;
            let fee = n.chain.blockhash_valid(m.recent_blockhash()).then(|| crate::exec::base_fee(&m, n.chain.params.lamports_per_signature));
            ctx(n, json!(fee))
        }
        "simulateTransaction" => {
            let mut tx = decode_tx(p)?;
            if cfg(p, 1)["replaceRecentBlockhash"].as_bool() == Some(true) {
                tx.message.set_recent_blockhash(n.chain.last_hash);
            } else if let Err(e) = n.chain.check(&tx) {
                return Ok(ctx(n, json!({"err": e, "logs": [], "accounts": null, "unitsConsumed": 0, "returnData": null})));
            }
            let mut v = simulate(n, tx);
            if cfg(p, 1)["replaceRecentBlockhash"].as_bool() == Some(true) {
                let last_valid = height + n.chain.params.blockhash_window as u64;
                v["replacementBlockhash"] = json!({"blockhash": n.chain.last_hash.to_string(), "lastValidBlockHeight": last_valid});
            }
            ctx(n, v)
        }
        "sendTransaction" => {
            let tx = decode_tx(p)?;
            if cfg(p, 1)["skipPreflight"].as_bool() != Some(true) {
                let fail = |data: Value, msg: String| RpcError { code: -32002, message: format!("Transaction simulation failed: {msg}"), data };
                if let Err(e) = n.chain.check(&tx) {
                    return Err(fail(json!({"err": e, "logs": [], "accounts": null, "unitsConsumed": 0}), e.to_string()));
                }
                let sim = simulate(n, tx.clone());
                if !sim["err"].is_null() {
                    return Err(fail(sim.clone(), sim["err"].to_string()));
                }
            }
            let s = tx.signatures[0];
            use crate::ingress::Rejected;
            let message = match n.ingress.submit(peer, vec![tx]).remove(0) {
                Ok(_) => {
                    n.arrivals.insert(s, arrived);
                    return Ok(json!(s.to_string()));
                }
                Err(Rejected::BadSignature) => "Transaction signature verification failure",
                Err(Rejected::RateLimited) => "limite di invio per IP superato, riprova",
                Err(Rejected::TooManyPending) => "troppe tx in coda per questo pagatore, riprova",
            };
            return Err(RpcError { code: -32003, message: message.into(), data: Value::Null });
        }
        "getSignatureStatuses" => {
            let sigs: Vec<String> = arg(p, 0)?;
            let v = sigs.iter().map(|s| Ok(n.index.by_sig.get(&sig(s)?).map(status))).collect::<Result<Vec<_>, RpcError>>()?;
            ctx(n, json!(v))
        }
        "getTransaction" => {
            let s = sig(&arg::<String>(p, 0)?)?;
            n.index.by_sig.get(&s).map_or(Value::Null, |r| transaction(r, cfg(p, 1)))
        }
        "getSignaturesForAddress" => {
            let a = addr(p, 0)?;
            let c = cfg(p, 1);
            let limit = c["limit"].as_u64().unwrap_or(1000).min(1000) as usize;
            let before = c["before"].as_str().map(sig).transpose()?;
            let until = c["until"].as_str().map(sig).transpose()?;
            let list = n.index.by_addr.get(&a).map_or(&[][..], |v| v.as_slice());
            let mut out = Vec::new();
            let mut started = before.is_none();
            for s in list.iter().rev() {
                if !started {
                    started = Some(*s) == before;
                    continue;
                }
                if Some(*s) == until || out.len() == limit {
                    break;
                }
                let r = &n.index.by_sig[s];
                out.push(json!({
                    "signature": s.to_string(), "slot": r.slot, "err": r.err(), "memo": null,
                    "blockTime": r.time_ms / 1000, "confirmationStatus": "finalized",
                }));
            }
            json!(out)
        }
        _ => {
            return Err(RpcError { code: -32601, message: "Method not found".into(), data: Value::Null });
        }
    })
}
