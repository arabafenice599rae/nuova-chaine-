//! Identità della chain: nel genesis hash, letta da getChainInfo, /chain.json,
//! getVersion e dallo stream; transazioni non riutilizzabili tra chain diverse.

use std::io::{Read, Write};
use std::net::TcpStream;
use std::path::PathBuf;
use std::time::Duration;

use centrifuga::client::call;
use centrifuga::exec::Chain;
use centrifuga::genesis::{self, Config, LAMPORTS_PER_CNFA, Prefund, transfer_tx};
use centrifuga::node;
use centrifuga::stream::Replica;
use futures_util::StreamExt;
use serde_json::{Value, json};
use solana_address::Address;
use solana_keypair::Keypair;
use solana_signer::Signer;
use solana_transaction::TransactionError;
use tokio_tungstenite::tungstenite::Message;

const EXAMPLE: &str = include_str!("../centrifuga.example.toml");

fn devnet(op: &Keypair, user: &Keypair) -> Config {
    let mut c: Config = toml::from_str(EXAMPLE).unwrap();
    c.genesis.network = "devnet".into();
    c.genesis.chain_id = "centrifuga:devnet".into();
    c.genesis.operator = op.pubkey().to_string();
    c.genesis.timestamp_ms = node::now_ms() - 86_400_000;
    c.genesis.prefund.push(Prefund { pubkey: user.pubkey().to_string(), lamports: 10 * LAMPORTS_PER_CNFA });
    c
}

fn genesis_hash(c: &Config) -> solana_hash::Hash {
    genesis::build(&c.genesis).unwrap().0.genesis_hash
}

#[test]
fn identity_is_part_of_the_genesis_hash() {
    let (op, user) = (Keypair::new(), Keypair::new());
    let dev = devnet(&op, &user);
    let mut main = dev.clone();
    main.genesis.network = "mainnet".into();
    main.genesis.chain_id = "centrifuga:mainnet".into();
    main.genesis.prefund.clear();
    let mut dev_noprefund = dev.clone();
    dev_noprefund.genesis.prefund.clear();
    let mut renamed = dev.clone();
    renamed.genesis.chain_name = "Centrifuga 2".into();
    let h = [genesis_hash(&dev), genesis_hash(&main), genesis_hash(&dev_noprefund), genesis_hash(&renamed)];
    assert_ne!(h[1], h[2], "mainnet e devnet con lo stesso stato: hash diversi (solo l'identità cambia)");
    assert_ne!(h[0], h[3], "il nome entra nell'hash");
    assert_eq!(genesis_hash(&dev), h[0], "deterministico");
}

#[test]
fn genesis_rejects_wrong_symbol_chain_id_or_decimals() {
    let (op, user) = (Keypair::new(), Keypair::new());
    let base = devnet(&op, &user);
    for (field, f) in [
        ("symbol", Box::new(|c: &mut Config| c.genesis.symbol = "CNFA".into()) as Box<dyn Fn(&mut Config)>),
        ("symbol", Box::new(|c: &mut Config| c.genesis.symbol = "cnfa".into())),
        ("symbol", Box::new(|c: &mut Config| c.genesis.symbol = "SOL".into())),
        ("chain_id", Box::new(|c: &mut Config| c.genesis.chain_id = "centrifuga:mainnet".into())),
        ("chain_id", Box::new(|c: &mut Config| c.genesis.chain_id = "solana:devnet".into())),
        ("decimals", Box::new(|c: &mut Config| c.genesis.decimals = 8)),
    ] {
        let mut c = base.clone();
        f(&mut c);
        let e = genesis::build(&c.genesis).err().unwrap_or_else(|| panic!("{field}: doveva fallire"));
        assert!(e.contains(field), "{field}: {e}");
    }
    genesis::build(&base.genesis).unwrap();
}

#[test]
fn transactions_do_not_cross_chains() {
    let (op, user) = (Keypair::new(), Keypair::new());
    let a_cfg = devnet(&op, &user);
    let mut b_cfg = a_cfg.clone();
    b_cfg.genesis.chain_name = "Altra".into(); // stesso stato, stesso pagatore, genesis diverso
    let (mut a, mut b): (Chain, Chain) = (genesis::build(&a_cfg.genesis).unwrap().0, genesis::build(&b_cfg.genesis).unwrap().0);
    assert_ne!(a.genesis_hash, b.genesis_hash);
    let t = node::now_ms() - 1_000;
    let to = Address::new_unique();
    let for_a = transfer_tx(&user, &to, LAMPORTS_PER_CNFA, a.last_hash);
    let for_b = transfer_tx(&user, &to, LAMPORTS_PER_CNFA, b.last_hash);
    // Firma valida, ma blockhash sconosciuto all'altra chain: rifiutata.
    assert!(centrifuga::ingress::verify_signatures(&[for_a.clone(), for_b.clone()]).iter().all(|ok| *ok));
    assert_eq!(b.check(&for_a).err(), Some(TransactionError::BlockhashNotFound));
    assert_eq!(a.check(&for_b).err(), Some(TransactionError::BlockhashNotFound));
    assert!(b.produce(vec![for_a.clone()], t, &op).block.txs.is_empty());
    assert!(a.produce(vec![for_b.clone()], t, &op).block.txs.is_empty());
    // Ognuna è accettata dalla propria chain (blockhash ancora nella finestra).
    assert_eq!(a.produce(vec![for_a], t + 100, &op).block.txs.len(), 1);
    assert_eq!(b.produce(vec![for_b], t + 100, &op).block.txs.len(), 1);
    // Un blocco di una chain non si applica all'altra.
    let blk = a.produce(vec![], t + 200, &op).block;
    assert!(b.apply(&blk).is_err());
}

fn http_get(addr: &str, path: &str) -> (String, Value) {
    let mut s = TcpStream::connect(addr).unwrap();
    write!(s, "GET {path} HTTP/1.1\r\nHost: {addr}\r\nConnection: close\r\n\r\n").unwrap();
    let mut r = String::new();
    s.read_to_string(&mut r).unwrap();
    let (head, body) = r.split_once("\r\n\r\n").unwrap();
    (head.to_lowercase(), serde_json::from_str(body).unwrap())
}

#[tokio::test(flavor = "multi_thread")]
async fn chain_info_chain_json_version_and_stream_match_genesis() {
    let (op, user) = (Keypair::new(), Keypair::new());
    let mut c = devnet(&op, &user);
    c.node.data_dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("identity_node");
    let _ = std::fs::remove_dir_all(&c.node.data_dir);
    c.node.rpc_addr = "127.0.0.1:19160".into();
    c.node.stream_addr = "127.0.0.1:19162".into();
    c.node.logo_url = "https://example.org/centrifuga.png".into();
    let expected_hash = genesis_hash(&c);
    let rules = c.genesis.params().unwrap().rules;
    let running = node::start(c.clone(), Keypair::try_from(op.to_bytes().as_slice()).unwrap()).unwrap();
    tokio::spawn(running.serve_network());
    tokio::time::sleep(Duration::from_millis(300)).await;

    let expected = json!({
        "chainId": "centrifuga:devnet", "name": "Centrifuga", "symbol": "CnFA", "decimals": 9,
        "genesisHash": expected_hash.to_string(), "logoUrl": "https://example.org/centrifuga.png",
    });
    let (info, version, (head, chain_json)) = tokio::task::spawn_blocking(|| {
        let url = "http://127.0.0.1:19160";
        (call(url, "getChainInfo", json!([])).unwrap(), call(url, "getVersion", json!([])).unwrap(), http_get("127.0.0.1:19160", "/chain.json"))
    })
    .await
    .unwrap();
    assert_eq!(info, expected);
    assert_eq!(chain_json, expected, "/chain.json = getChainInfo");
    assert!(head.contains("access-control-allow-origin: *"), "CORS aperto");
    assert_eq!(version["chainId"], "centrifuga:devnet");
    assert!(version["solana-core"].is_string() && version["feature-set"].is_number(), "campi degli SDK Solana");
    let genesis_rpc = tokio::task::spawn_blocking(|| call("http://127.0.0.1:19160", "getGenesisHash", json!([])).unwrap()).await.unwrap();
    assert_eq!(genesis_rpc, expected_hash.to_string());

    // Stream: la replica accetta la propria chain e rifiuta un'altra identità.
    let url = "ws://127.0.0.1:19162/";
    let first = |exp: Replica| async move {
        let (mut ws, _) = tokio_tungstenite::connect_async(url).await.unwrap();
        let mut rep = exp;
        loop {
            if let Some(Ok(Message::Text(t))) = ws.next().await {
                return rep.apply(&t).map(|_| rep);
            }
        }
    };
    let ok = first(Replica::new(op.pubkey(), rules).expect_chain("centrifuga:devnet", expected_hash)).await;
    assert!(ok.is_ok());
    let wrong_id = first(Replica::new(op.pubkey(), rules).expect_chain("centrifuga:mainnet", expected_hash)).await;
    assert!(wrong_id.err().unwrap().contains("another chain"));
    let wrong_hash = first(Replica::new(op.pubkey(), rules).expect_chain("centrifuga:devnet", solana_hash::Hash::new_from_array([7; 32]))).await;
    assert!(wrong_hash.err().unwrap().contains("another chain"));
    // Un messaggio prima dell'identità è rifiutato da una replica che la attende.
    let mut strict = Replica::new(op.pubkey(), rules).expect_chain("centrifuga:devnet", expected_hash);
    let gw_block = centrifuga::stream::header_message(&genesis::build(&c.genesis).unwrap().0.produce(vec![], node::now_ms(), &op).block);
    assert!(strict.apply(&gw_block).unwrap_err().contains("chain identity missing"));
}
