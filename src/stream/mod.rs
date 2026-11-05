//! Stream: l'API principale per bot e repliche. WebSocket, un messaggio JSON per frame.
//!
//! - `{"type":"block", "header":{...}, "txs":[base64], "accounts":[...] | null}`
//!   live: a ogni blocco, con il valore (dopo il blocco) degli account cambiati;
//!   `accounts: null` per i blocchi di recupero (solo header e corpo).
//! - `{"type":"sync", "height":H, "full":bool, "accounts":[...]}`
//!   dopo il recupero: stato completo (`full`) o valore attuale degli account
//!   cambiati dall'altezza richiesta. L'hash di stato deve coincidere con l'header H.
//!
//! Connessione: `ws://host:porta/?from_height=h`. Senza `from_height` (o con 0) il
//! client riceve lo stato completo; con h riceve i blocchi h.. dal journal più un
//! delta cumulativo, se h è nella finestra `stream_catchup_blocks`.
//! Un account con 0 lamport è cancellato. Il nodo non aspetta mai i client: chi
//! resta indietro di più di `stream_max_lag_blocks` viene disconnesso.

use std::collections::{BTreeSet, HashMap};
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::mpsc::Sender;

use base64::Engine;
use futures_util::{SinkExt, StreamExt};
use serde_json::{Value, json};
use solana_account::{AccountSharedData, ReadableAccount, WritableAccount};
use solana_address::Address;
use solana_hash::Hash;
use solana_signature::Signature;
use solana_transaction::versioned::VersionedTransaction;
use tokio::sync::{broadcast, oneshot};
use tokio_tungstenite::tungstenite::Message;

use crate::exec::Executed;
use crate::exec::state_hash::{FullSha256, StateHasher};
use crate::ledger::{Block, Header, block::tx_bytes};
use crate::node::{Node, Query};
use crate::rpc::encode::b64;

/// Buffer di invio dei socket dello stream: oltre questo, un client lento
/// smette di ricevere e il canale del nodo lo fa restare indietro.
const SEND_BUFFER: u32 = 256 * 1024;

type Accounts = Vec<(Address, Option<AccountSharedData>)>;

fn account_json(k: &Address, a: Option<&AccountSharedData>) -> Value {
    match a {
        Some(a) => json!({
            "pubkey": k.to_string(), "lamports": a.lamports(), "owner": a.owner().to_string(),
            "executable": a.executable(), "rent_epoch": a.rent_epoch(), "data": b64(a.data()),
        }),
        None => json!({"pubkey": k.to_string(), "lamports": 0}),
    }
}

fn block_json(b: &Block, accounts: Option<&Accounts>) -> Value {
    let h = &b.header;
    json!({
        "type": "block",
        "header": {
            "height": h.height, "prev_hash": h.prev_hash.to_string(), "timestamp_ms": h.timestamp_ms,
            "state_hash": h.state_hash.to_string(), "signature": h.signature.to_string(),
        },
        "txs": b.txs.iter().map(|t| b64(tx_bytes(t))).collect::<Vec<_>>(),
        "accounts": accounts.map(|a| a.iter().map(|(k, v)| account_json(k, v.as_ref())).collect::<Vec<_>>()),
    })
}

/// Valore attuale degli account (copie economiche: i dati sono condivisi).
fn snapshot(n: &Node, keys: impl IntoIterator<Item = Address>) -> Accounts {
    keys.into_iter().map(|k| (k, n.chain.svm.get(&k).cloned())).collect()
}

/// Messaggio "solo header e corpo" (come nel recupero dal journal).
pub fn header_message(b: &Block) -> String {
    block_json(b, None).to_string()
}

/// Messaggio live per un blocco appena prodotto (sul thread del nodo).
pub fn block_message(n: &Node, ex: &Executed) -> String {
    block_json(&ex.block, Some(&snapshot(n, ex.changed.iter().copied()))).to_string()
}

#[derive(Clone)]
pub struct Stream {
    pub queries: Sender<Query>,
    /// (altezza, messaggio) per ogni blocco; capacità = ritardo massimo ammesso.
    pub blocks: broadcast::Sender<Arc<(u64, String)>>,
    /// Durata del ritardo massimo (blocchi × slot): un invio bloccato più a
    /// lungo vuol dire che il client non legge, e viene disconnesso.
    pub max_wait: std::time::Duration,
}

/// Cosa serve a un client per mettersi in pari, preso dal nodo in un colpo solo.
struct Catchup {
    height: u64,
    journal: PathBuf,
    /// Offset nel journal dei blocchi da rimandare.
    offsets: Vec<u64>,
    full: bool,
    accounts: Accounts,
}

fn catchup(n: &Node, from: u64) -> Option<Catchup> {
    let height = n.chain.height;
    let last = n.last_offset?; // altezza 0: nessun blocco a cui ancorare lo stato
    let in_window = from > 0 && n.recent.front().is_some_and(|r| r.0 <= from);
    let (offsets, full, accounts) = if from > height {
        (vec![], false, vec![])
    } else if in_window {
        let blocks: Vec<_> = n.recent.iter().filter(|r| r.0 >= from).collect();
        let keys: BTreeSet<Address> = blocks.iter().flat_map(|r| r.2.iter().copied()).collect();
        (blocks.iter().map(|r| r.1).collect(), false, snapshot(n, keys))
    } else {
        let all = n.chain.svm.accounts().iter().map(|(k, a)| (*k, Some(a.clone()))).collect();
        (vec![last], true, all)
    };
    Some(Catchup { height, journal: n.journal_path(), offsets, full, accounts })
}

pub async fn serve(s: Stream, addr: SocketAddr) -> std::io::Result<()> {
    let sock = if addr.is_ipv4() { tokio::net::TcpSocket::new_v4()? } else { tokio::net::TcpSocket::new_v6()? };
    sock.set_reuseaddr(true)?;
    sock.set_send_buffer_size(SEND_BUFFER)?; // ereditato dai socket accettati
    sock.bind(addr)?;
    let l = sock.listen(128)?;
    eprintln!("stream: ws://{addr}");
    loop {
        let (c, _) = l.accept().await?;
        tokio::spawn(conn(s.clone(), c));
    }
}

fn from_height(uri: &str) -> u64 {
    uri.split(['?', '&']).find_map(|p| p.strip_prefix("from_height=")).and_then(|v| v.parse().ok()).unwrap_or(0)
}

async fn conn(s: Stream, c: tokio::net::TcpStream) {
    let mut from = 0;
    let cb = |req: &tokio_tungstenite::tungstenite::handshake::server::Request, resp| {
        from = from_height(&req.uri().to_string());
        Ok(resp)
    };
    let Ok(ws) = tokio_tungstenite::accept_hdr_async(c, cb).await else { return };
    let (mut tx, mut rx) = ws.split();
    // Prima l'iscrizione, poi il recupero: nessun blocco può cadere nel mezzo.
    let mut live = s.blocks.subscribe();
    let (otx, orx) = oneshot::channel();
    if s.queries.send(Box::new(move |n: &mut Node| drop(otx.send(catchup(n, from))))).is_err() {
        return;
    }
    let Ok(Some(cu)) = orx.await else {
        let frame = tokio_tungstenite::tungstenite::protocol::CloseFrame { code: 1013.into(), reason: "nessun blocco ancora: riprova".into() };
        let _ = tx.send(Message::Close(Some(frame))).await;
        return;
    };
    let height = cu.height;
    let msgs = tokio::task::spawn_blocking(move || -> Result<Vec<String>, String> {
        let mut f = std::fs::File::open(&cu.journal).map_err(|e| e.to_string())?;
        let mut out = Vec::new();
        for off in cu.offsets {
            out.push(block_json(&crate::ledger::read_block_at(&mut f, off)?, None).to_string());
        }
        if !out.is_empty() {
            let accounts: Vec<Value> = cu.accounts.iter().map(|(k, a)| account_json(k, a.as_ref())).collect();
            out.push(json!({"type": "sync", "height": cu.height, "full": cu.full, "accounts": accounts}).to_string());
        }
        Ok(out)
    });
    let Ok(Ok(msgs)) = msgs.await else { return };
    for m in msgs {
        if tx.send(Message::text(m)).await.is_err() {
            return;
        }
    }
    loop {
        tokio::select! {
            m = rx.next() => match m {
                Some(Ok(Message::Ping(p))) => { let _ = tx.send(Message::Pong(p)).await; }
                Some(Ok(_)) => {}
                _ => return,
            },
            b = live.recv() => match b {
                Ok(b) if b.0 <= height => {}
                Ok(b) => match tokio::time::timeout(s.max_wait, tx.send(Message::text(b.1.clone()))).await {
                    Ok(Ok(())) => {}
                    _ => return, // errore, o client fermo: si chiude la connessione
                },
                // Troppo indietro: il nodo non aspetta, il client si riconnette con from_height.
                Err(broadcast::error::RecvError::Lagged(_)) => {
                    let reason = "troppo indietro: riconnettersi con from_height";
                    let frame = tokio_tungstenite::tungstenite::protocol::CloseFrame { code: 1008.into(), reason: reason.into() };
                    let _ = tx.send(Message::Close(Some(frame))).await;
                    return;
                }
                Err(_) => return,
            },
        }
    }
}

/// Replica dello stato costruita solo dallo stream. Verifica firma dell'operatore,
/// continuità degli hash e hash di stato.
pub struct Replica {
    pub operator: Address,
    pub height: u64,
    pub last_hash: Hash,
    last_state: Hash,
    /// Nuova ancora non contigua (recupero oltre la finestra): serve uno stato completo.
    needs_full: bool,
    last_ts: u64,
    /// Regole di altezza e timestamp del genesis.
    pub rules: crate::exec::Rules,
    /// In tempo reale: rifiuta timestamp oltre questo margine nel futuro (None = storico).
    pub max_future_ms: Option<u64>,
    pub accounts: HashMap<Address, AccountSharedData>,
}

fn field<'a>(v: &'a Value, k: &str) -> Result<&'a Value, String> {
    v.get(k).ok_or_else(|| format!("campo mancante: {k}"))
}

fn parse<T: std::str::FromStr>(v: &Value, k: &str) -> Result<T, String> {
    field(v, k)?.as_str().and_then(|s| s.parse().ok()).ok_or_else(|| format!("campo non valido: {k}"))
}

fn decode64(s: &str) -> Result<Vec<u8>, String> {
    base64::engine::general_purpose::STANDARD.decode(s).map_err(|e| e.to_string())
}

impl Replica {
    /// Replica in tempo reale: rifiuta i blocchi con timestamp oltre 5 s nel futuro.
    pub fn new(operator: Address, rules: crate::exec::Rules) -> Replica {
        Replica {
            operator,
            height: 0,
            last_hash: Hash::default(),
            last_state: Hash::default(),
            needs_full: false,
            last_ts: 0,
            rules,
            max_future_ms: Some(5_000),
            accounts: HashMap::new(),
        }
    }

    pub fn state_hash(&self) -> Hash {
        FullSha256.hash(&self.accounts, &Default::default())
    }

    fn apply_accounts(&mut self, list: &Value) -> Result<(), String> {
        for a in list.as_array().ok_or("accounts")? {
            let k: Address = parse(a, "pubkey")?;
            let lamports = field(a, "lamports")?.as_u64().ok_or("lamports")?;
            if lamports == 0 {
                self.accounts.remove(&k);
                continue;
            }
            let data = decode64(field(a, "data")?.as_str().ok_or("data")?)?;
            let mut acc = AccountSharedData::new(lamports, data.len(), &parse(a, "owner")?);
            acc.set_data_from_slice(&data);
            acc.set_executable(field(a, "executable")?.as_bool().ok_or("executable")?);
            acc.set_rent_epoch(field(a, "rent_epoch")?.as_u64().ok_or("rent_epoch")?);
            self.accounts.insert(k, acc);
        }
        Ok(())
    }

    fn check_state(&self) -> Result<(), String> {
        if self.state_hash() != self.last_state {
            return Err(format!("blocco {}: hash di stato diverso", self.height));
        }
        Ok(())
    }

    /// Applica un messaggio dello stream; errore = stream non valido o manomesso.
    pub fn apply(&mut self, msg: &str) -> Result<(), String> {
        let v: Value = serde_json::from_str(msg).map_err(|e| e.to_string())?;
        match field(&v, "type")?.as_str() {
            Some("block") => {
                let h = field(&v, "header")?;
                let header = Header {
                    height: field(h, "height")?.as_u64().ok_or("height")?,
                    prev_hash: parse(h, "prev_hash")?,
                    timestamp_ms: field(h, "timestamp_ms")?.as_u64().ok_or("timestamp_ms")?,
                    state_hash: parse(h, "state_hash")?,
                    signature: parse::<Signature>(h, "signature")?,
                };
                let txs = field(&v, "txs")?.as_array().ok_or("txs")?.iter().map(|t| {
                    let b = decode64(t.as_str().ok_or("tx")?)?;
                    wincode::deserialize::<VersionedTransaction>(&b).map_err(|e| e.to_string())
                });
                let block = Block { header, txs: txs.collect::<Result<_, String>>()? };
                if !block.verify_signature(&self.operator) {
                    return Err(format!("blocco {}: firma non valida", block.header.height));
                }
                let h = &block.header;
                if let Some(max) = self.max_future_ms {
                    if h.timestamp_ms > crate::node::now_ms() + max {
                        return Err(format!("blocco {}: timestamp nel futuro", h.height));
                    }
                }
                // Blocco concatenato (stesso prev_hash): valgono le regole di altezza e
                // timestamp. Un blocco solo header con prev_hash diverso è una nuova ancora
                // (recupero oltre la finestra): vale solo se segue uno stato completo.
                let accounts = field(&v, "accounts")?;
                let anchored = self.height > 0 || self.last_hash != Hash::default();
                if anchored && h.prev_hash == self.last_hash {
                    self.rules.check(self.height, self.last_ts, h)?;
                } else if anchored {
                    if !accounts.is_null() || h.height <= self.height {
                        return Err(format!("blocco {}: non concatenato a {}", h.height, self.height));
                    }
                    self.needs_full = true;
                }
                self.last_ts = h.timestamp_ms;
                if self.needs_full && !accounts.is_null() {
                    return Err("delta ricevuto prima dello stato completo".into());
                }
                self.height = block.header.height;
                self.last_hash = block.hash();
                self.last_state = block.header.state_hash;
                if !accounts.is_null() {
                    self.apply_accounts(accounts)?;
                    self.check_state()?;
                }
                Ok(())
            }
            Some("sync") => {
                if field(&v, "height")?.as_u64() != Some(self.height) {
                    return Err("sync non allineato all'ultimo header".into());
                }
                let full = field(&v, "full")?.as_bool() == Some(true);
                if self.needs_full && !full {
                    return Err("serve uno stato completo".into());
                }
                if full {
                    self.accounts.clear();
                    self.needs_full = false;
                }
                self.apply_accounts(field(&v, "accounts")?)?;
                self.check_state()
            }
            _ => Err("tipo di messaggio sconosciuto".into()),
        }
    }
}
