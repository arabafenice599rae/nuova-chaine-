//! Blocchi solo quando servono: altezza = slot, salti, finestra di 300 slot, frode, heartbeat.

use std::time::{Duration, Instant};

use centrifuga::exec::Chain;
use centrifuga::genesis::{self, Config, LAMPORTS_PER_CNFA, Prefund, transfer_tx};
use centrifuga::ledger::block::is_fraud_proof;
use centrifuga::node::{self, Node};
use solana_address::Address;
use solana_keypair::Keypair;
use solana_signer::Signer;

const EXAMPLE: &str = include_str!("../centrifuga.example.toml");

fn cfg(op: &Keypair, user: &Keypair) -> Config {
    let mut c: Config = toml::from_str(EXAMPLE).unwrap();
    c.genesis.network = "devnet".into();
    c.genesis.operator = op.pubkey().to_string();
    c.genesis.prefund.push(Prefund { pubkey: user.pubkey().to_string(), lamports: 10 * LAMPORTS_PER_CNFA });
    c
}

fn chain(c: &Config) -> Chain {
    genesis::build(&c.genesis).unwrap().0
}

#[test]
fn height_gaps_are_produced_and_replayed() {
    let (op, user) = (Keypair::new(), Keypair::new());
    let c = cfg(&op, &user);
    let (mut a, mut b) = (chain(&c), chain(&c));
    let to = Address::new_unique();
    let t0 = c.genesis.timestamp_ms;
    let mut blocks = vec![];
    // Salti piccoli e grandi; le tx usano sempre un blockhash entro 300 slot.
    for (i, h) in [1u64, 2, 50, 51, 340, 620, 10_000, 10_001].into_iter().enumerate() {
        let txs = if h == 10_000 { vec![] } else { vec![transfer_tx(&user, &to, LAMPORTS_PER_CNFA + i as u64, a.last_hash)] };
        let ex = a.produce_at(h, txs, t0 + h * 100, &op);
        assert!(ex.results.iter().all(Result::is_ok) && ex.block.txs.len() == usize::from(h != 10_000), "altezza {h}");
        blocks.push(ex.block);
    }
    for blk in &blocks {
        b.apply(blk).unwrap();
    }
    assert_eq!((b.height, b.last_hash), (10_001, a.last_hash));
    // Altezza non crescente o prev_hash sbagliato: rifiutati.
    let mut c2 = chain(&c);
    c2.apply(&blocks[0]).unwrap();
    c2.apply(&blocks[1]).unwrap();
    let mut back = blocks[2].clone();
    back.header.height = 2;
    back.sign(&op);
    assert!(c2.apply(&back).err().unwrap().contains("non concatenato"));
    assert!(c2.apply(&blocks[3]).err().unwrap().contains("non concatenato"), "salta il blocco 50");
}

#[test]
fn fork_with_same_prev_hash_is_fraud() {
    let (op, user) = (Keypair::new(), Keypair::new());
    let c = cfg(&op, &user);
    let t0 = c.genesis.timestamp_ms;
    // Due catene identiche fino al blocco 6, poi due figli ad altezze diverse.
    let (mut a, mut b) = (chain(&c), chain(&c));
    for ch in [&mut a, &mut b] {
        ch.produce_at(5, vec![], t0 + 500, &op);
        ch.produce_at(6, vec![], t0 + 600, &op);
    }
    assert_eq!(a.last_hash, b.last_hash);
    let x = a.produce_at(10, vec![], t0 + 1_000, &op).block;
    let y = b.produce_at(20, vec![], t0 + 2_000, &op).block;
    assert_eq!(x.header.prev_hash, y.header.prev_hash);
    assert_ne!(x.header.height, y.header.height);
    assert!(is_fraud_proof(&x, &y, &op.pubkey()), "stesso prev_hash, due firme valide");
    assert!(!is_fraud_proof(&x, &x, &op.pubkey()), "lo stesso blocco non è frode");
    assert!(!is_fraud_proof(&x, &y, &Keypair::new().pubkey()), "firme di un altro: non è una prova");
    let mut forged = y.clone();
    forged.header.timestamp_ms += 1;
    assert!(!is_fraud_proof(&x, &forged, &op.pubkey()), "firma non valida: non è una prova");
    // Stessa altezza, prev_hash diverso: anche questa è frode.
    let mut c3 = chain(&c);
    let z = c3.produce_at(10, vec![], t0 + 1_000, &op).block;
    assert_ne!(z.header.prev_hash, x.header.prev_hash);
    assert!(is_fraud_proof(&x, &z, &op.pubkey()));
}

#[test]
fn blockhash_window_is_300_slots_across_gaps() {
    let (op, user) = (Keypair::new(), Keypair::new());
    let c = cfg(&op, &user);
    assert_eq!(c.genesis.blockhash_window, 300);
    let t0 = c.genesis.timestamp_ms;
    let to = Address::new_unique();
    // Blockhash del blocco 1.000, un salto a 1.250, poi la tx al blocco `next`.
    let included = |next: u64| {
        let mut ch = chain(&c);
        ch.produce_at(1_000, vec![], t0 + 100_000, &op);
        let tx = transfer_tx(&user, &to, LAMPORTS_PER_CNFA, ch.last_hash);
        ch.produce_at(1_250, vec![], t0 + 125_000, &op);
        ch.produce_at(next, vec![tx], t0 + next * 100, &op).block.txs.len()
    };
    assert_eq!(included(1_300), 1, "h + 300: valido");
    assert_eq!(included(1_301), 0, "h + 301: scaduto");

    // Duplicato attraverso un salto: la firma resta nota finché il blockhash è valido.
    let mut ch = chain(&c);
    let tx = transfer_tx(&user, &to, LAMPORTS_PER_CNFA, ch.last_hash);
    assert_eq!(ch.produce_at(10, vec![tx.clone()], t0 + 1_000, &op).block.txs.len(), 1);
    assert_eq!(ch.produce_at(250, vec![tx], t0 + 25_000, &op).block.txs.len(), 0);
    // Il blockhash del blocco 250 vale fino al blocco 550 (= lastValidBlockHeight di
    // getLatestBlockhash); isBlockhashValid guarda il prossimo blocco possibile.
    let bh = ch.last_hash;
    assert!(ch.blockhash_valid(&bh));
    ch.produce_at(549, vec![], t0 + 54_900, &op);
    assert!(ch.blockhash_valid(&bh), "il prossimo blocco può essere il 550");
    ch.produce_at(550, vec![], t0 + 55_000, &op);
    assert!(!ch.blockhash_valid(&bh), "il prossimo blocco è almeno il 551");
}

#[tokio::test(flavor = "multi_thread")]
async fn node_produces_on_demand_with_heartbeat_and_slot_heights() {
    let (op, user) = (Keypair::new(), Keypair::new());
    let mut c = cfg(&op, &user);
    let dir = std::path::PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("slots_node");
    let _ = std::fs::remove_dir_all(&dir);
    c.node.data_dir = dir;
    c.node.slot_ms = 20;
    c.node.heartbeat_slots = 10; // senza tx: un blocco ogni 200 ms
    c.node.rpc_addr = "127.0.0.1:19150".into();
    c.node.stream_addr = "127.0.0.1:19152".into();
    c.genesis.timestamp_ms = node::now_ms(); // altezza = slot reale dal genesis
    let running = node::start(c.clone(), Keypair::try_from(op.to_bytes().as_slice()).unwrap()).unwrap();
    let gw = running.gateway.clone();
    tokio::spawn(running.serve_network());
    let state = || async { gw.query(|n: &mut Node| (n.chain.height, n.chain.last_hash, n.index.by_sig.len())).await.unwrap() };

    // Senza tx: ~5 blocchi in 1 s (heartbeat), altezze che saltano di ~10 slot.
    tokio::time::sleep(Duration::from_millis(300)).await;
    let (h0, ..) = state().await;
    let mut heights = vec![];
    let start = Instant::now();
    while start.elapsed() < Duration::from_secs(1) {
        let (h, ..) = state().await;
        if heights.last() != Some(&h) {
            heights.push(h);
        }
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
    assert!((3..=8).contains(&heights.len()), "blocchi in 1 s: {heights:?}");
    for w in heights.windows(2) {
        assert!((8..=12).contains(&(w[1] - w[0])), "salto di altezza atteso ~10: {heights:?}");
    }
    let expected = (node::now_ms() - c.genesis.timestamp_ms) / 20;
    assert!(heights.last().unwrap().abs_diff(expected) <= 12, "altezza = slot reale");
    assert!(h0 > 0);

    // Con una tx: blocco allo slot successivo, non all'heartbeat.
    let (_, bh, n0) = state().await;
    let tx = transfer_tx(&user, &Address::new_unique(), LAMPORTS_PER_CNFA, bh);
    let t = Instant::now();
    assert!(gw.query(move |n: &mut Node| n.ingress.submit(None, vec![tx])).await.unwrap()[0].is_ok());
    while state().await.2 == n0 {
        tokio::time::sleep(Duration::from_millis(2)).await;
    }
    assert!(t.elapsed() < Duration::from_millis(100), "tx inclusa in {:?} (slot 20 ms, heartbeat 200 ms)", t.elapsed());
}
