//! Blocchi solo quando servono: regole di altezza (1 <= salto <= heartbeat_slots) e di
//! timestamp (>= precedente + salto × slot_ms / 2), applicate da Chain::apply, dalla
//! riesecuzione del journal e da stream::Replica; finestra di 300 slot; frode; heartbeat.

use std::path::PathBuf;
use std::time::{Duration, Instant};

use centrifuga::exec::Chain;
use centrifuga::genesis::{self, Config, LAMPORTS_PER_CNFA, Prefund, transfer_tx};
use centrifuga::ledger::block::is_fraud_proof;
use centrifuga::ledger::{Block, Journal};
use centrifuga::node::{self, Node};
use centrifuga::stream::{Replica, header_message};
use solana_address::Address;
use solana_keypair::Keypair;
use solana_signer::Signer;

const EXAMPLE: &str = include_str!("../centrifuga.example.toml");

fn cfg(op: &Keypair, user: &Keypair) -> Config {
    let mut c: Config = toml::from_str(EXAMPLE).unwrap();
    c.genesis.network = "devnet".into();
    c.genesis.chain_id = "centrifuga:devnet".into();
    c.genesis.operator = op.pubkey().to_string();
    // Genesis di ieri: i blocchi dei test sono nel passato per la replica in tempo reale.
    c.genesis.timestamp_ms = node::now_ms() - 86_400_000;
    c.genesis.prefund.push(Prefund { pubkey: user.pubkey().to_string(), lamports: 10 * LAMPORTS_PER_CNFA });
    c
}

fn chain(c: &Config) -> Chain {
    genesis::build(&c.genesis).unwrap().0
}

/// Blocchi vuoti alle altezze date (salti validi), con timestamp di 1 s per salto.
fn blocks(c: &Config, op: &Keypair, heights: &[u64]) -> (Chain, Vec<Block>) {
    let mut ch = chain(c);
    let out = heights.iter().map(|h| ch.produce_at(*h, vec![], c.genesis.timestamp_ms + h * 1_000, op).block).collect();
    (ch, out)
}

/// Ri-firma un blocco alterato: la firma è valida, il rifiuto deve venire dalla regola.
fn resign(mut b: Block, op: &Keypair, f: impl FnOnce(&mut Block)) -> Block {
    f(&mut b);
    b.sign(op);
    assert!(b.verify_signature(&op.pubkey()), "firma reale e valida");
    b
}

/// Esito dei tre controlli su `prefix` (validi) + `last`: apply, journal, replica.
fn three_ways(c: &Config, op: &Keypair, prefix: &[Block], last: &Block, name: &str) -> [Result<(), String>; 3] {
    // 1. Chain::apply
    let mut ch = chain(c);
    for b in prefix {
        ch.apply(b).unwrap();
    }
    let apply = ch.apply(last).map(|_| ());
    // 2. Riesecuzione del journal (Node::open legge e verifica journal.bin).
    let mut nc = c.clone();
    nc.node.data_dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(format!("rules_{name}"));
    let _ = std::fs::remove_dir_all(&nc.node.data_dir);
    std::fs::create_dir_all(&nc.node.data_dir).unwrap();
    let mut j = Journal::open(&nc.node.data_dir.join("journal.bin"), |_, _| Ok(())).unwrap();
    for b in prefix.iter().chain([last]) {
        j.append(b).unwrap();
    }
    drop(j);
    let journal = Node::open(nc, Keypair::try_from(op.to_bytes().as_slice()).unwrap()).map(|_| ());
    // 3. Replica (messaggi "solo header" dallo stream, regole del genesis).
    let mut rep = Replica::new(op.pubkey(), c.genesis.params().unwrap().rules);
    let mut replica = Ok(());
    for b in prefix.iter().chain([last]) {
        replica = rep.apply(&header_message(b));
        if replica.is_err() {
            break;
        }
    }
    [apply, journal, replica]
}

fn all_rejected(r: [Result<(), String>; 3], why: &str) {
    for (way, res) in ["apply", "journal", "replica"].iter().zip(r) {
        let e = res.err().unwrap_or_else(|| panic!("{way}: doveva rifiutare ({why})"));
        assert!(e.contains(why), "{way}: rifiutato per un altro motivo: {e}");
    }
}

#[test]
fn gap_at_limit_accepted_limit_plus_one_rejected() {
    let (op, user) = (Keypair::new(), Keypair::new());
    let c = cfg(&op, &user);
    assert_eq!(c.genesis.heartbeat_slots, 10);
    let (_, bs) = blocks(&c, &op, &[5, 15]); // 15 - 5 = 10 = limite
    for r in three_ways(&c, &op, &bs[..1], &bs[1], "limite") {
        r.unwrap();
    }
    let bad = resign(bs[1].clone(), &op, |b| b.header.height = 16); // salto 11
    all_rejected(three_ways(&c, &op, &bs[..1], &bad, "limite_piu_uno"), "height gap");
    let zero = resign(bs[1].clone(), &op, |b| b.header.height = 5); // salto 0
    all_rejected(three_ways(&c, &op, &bs[..1], &zero, "salto_zero"), "height gap");
}

#[test]
fn gap_onto_unlock_height_rejected() {
    let (op, user) = (Keypair::new(), Keypair::new());
    let mut c = cfg(&op, &user);
    c.genesis.reserve_unlock_height = 100;
    c.genesis.operator_unlock_blocks = 100;
    let (_, bs) = blocks(&c, &op, &[10, 20, 30, 40, 50, 60, 70, 80, 89, 99]);
    // Dall'89 direttamente allo sblocco (100): salto 11, rifiutato.
    let onto = resign(bs[9].clone(), &op, |b| b.header.height = 100);
    all_rejected(three_ways(&c, &op, &bs[..9], &onto, "verso_sblocco"), "height gap");
}

#[test]
fn timestamp_too_close_rejected() {
    let (op, user) = (Keypair::new(), Keypair::new());
    let c = cfg(&op, &user);
    let (_, bs) = blocks(&c, &op, &[5, 9]); // salto 4: servono almeno 4 × 100 / 2 = 200 ms
    let prev = bs[0].header.timestamp_ms;
    let ok = resign(bs[1].clone(), &op, |b| b.header.timestamp_ms = prev + 200);
    let r = three_ways(&c, &op, &bs[..1], &ok, "ts_al_limite");
    assert!(!r[0].as_ref().unwrap_err().contains("timestamp"), "al limite la regola passa (fallisce solo l'hash di stato)");
    assert!(r[2].is_ok(), "replica: timestamp al limite accettato");
    let close = resign(bs[1].clone(), &op, |b| b.header.timestamp_ms = prev + 199);
    all_rejected(three_ways(&c, &op, &bs[..1], &close, "ts_vicino"), "too close");
}

#[test]
fn future_timestamp_rejected_only_by_realtime_replica() {
    let (op, user) = (Keypair::new(), Keypair::new());
    let c = cfg(&op, &user);
    let mut ch = chain(&c);
    let b1 = ch.produce_at(1, vec![], node::now_ms(), &op).block;
    let b2 = ch.produce_at(2, vec![], node::now_ms() + 60_000, &op).block; // blocco valido, 60 s nel futuro
    let [apply, journal, replica] = three_ways(&c, &op, std::slice::from_ref(&b1), &b2, "futuro");
    apply.unwrap();
    journal.unwrap();
    assert!(replica.unwrap_err().contains("in the future"), "replica in tempo reale");
    let mut historical = Replica::new(op.pubkey(), c.genesis.params().unwrap().rules);
    historical.max_future_ms = None;
    historical.apply(&header_message(&b1)).unwrap();
    historical.apply(&header_message(&b2)).unwrap();
}

#[test]
fn restart_after_long_pause_does_not_jump() {
    let (op, user) = (Keypair::new(), Keypair::new());
    let mut c = cfg(&op, &user);
    c.node.data_dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("restart_pause");
    let _ = std::fs::remove_dir_all(&c.node.data_dir);
    let kp = || Keypair::try_from(op.to_bytes().as_slice()).unwrap();
    let t0 = c.genesis.timestamp_ms;
    let mut n = Node::open(c.clone(), kp()).unwrap();
    assert_eq!(n.tick(t0 + 100).unwrap().block.header.height, 1, "dopo l'avvio: +1");
    assert_eq!(n.tick(t0 + 400).unwrap().block.header.height, 4, "3 slot trascorsi");
    assert_eq!(n.tick(t0 + 60_000).unwrap().block.header.height, 14, "pausa lunga nel processo: al massimo +10");
    drop(n);
    let mut n = Node::open(c, kp()).unwrap();
    let ex = n.tick(t0 + 3_600_000).unwrap(); // un'ora dopo
    assert_eq!(ex.block.header.height, 15, "riavvio: ultima altezza + 1, nessun recupero");
    assert_eq!(ex.block.header.timestamp_ms, t0 + 3_600_000);
}

#[test]
fn gaps_within_limit_are_produced_and_replayed() {
    let (op, user) = (Keypair::new(), Keypair::new());
    let c = cfg(&op, &user);
    let (mut a, mut b) = (chain(&c), chain(&c));
    let to = Address::new_unique();
    let t0 = c.genesis.timestamp_ms;
    let mut out = vec![];
    for (i, h) in [1u64, 2, 10, 11, 21, 30, 31].into_iter().enumerate() {
        let tx = transfer_tx(&user, &to, LAMPORTS_PER_CNFA + i as u64, a.last_hash);
        let ex = a.produce_at(h, vec![tx], t0 + h * 1_000, &op);
        assert!(ex.results[0].is_ok(), "altezza {h}");
        out.push(ex.block);
    }
    for blk in &out {
        b.apply(blk).unwrap();
    }
    assert_eq!((b.height, b.last_hash), (31, a.last_hash));
    let mut c2 = chain(&c);
    c2.apply(&out[0]).unwrap();
    assert!(c2.apply(&out[2]).err().unwrap().contains("not chained"), "salta il blocco 2");
}

#[test]
fn fork_with_same_prev_hash_is_fraud() {
    let (op, user) = (Keypair::new(), Keypair::new());
    let c = cfg(&op, &user);
    let (mut a, _) = blocks(&c, &op, &[5, 6]);
    let (mut b, _) = blocks(&c, &op, &[5, 6]);
    assert_eq!(a.last_hash, b.last_hash);
    let t0 = c.genesis.timestamp_ms;
    let x = a.produce_at(10, vec![], t0 + 10_000, &op).block;
    let y = b.produce_at(15, vec![], t0 + 15_000, &op).block;
    assert_eq!(x.header.prev_hash, y.header.prev_hash);
    assert_ne!(x.header.height, y.header.height);
    assert!(is_fraud_proof(&x, &y, &op.pubkey()), "stesso prev_hash, due firme valide");
    assert!(!is_fraud_proof(&x, &x, &op.pubkey()), "lo stesso blocco non è frode");
    assert!(!is_fraud_proof(&x, &y, &Keypair::new().pubkey()), "firme di un altro: non è una prova");
    let mut forged = y.clone();
    forged.header.timestamp_ms += 1;
    assert!(!is_fraud_proof(&x, &forged, &op.pubkey()), "firma non valida: non è una prova");
    let (mut c3, _) = blocks(&c, &op, &[5]);
    let z = c3.produce_at(10, vec![], t0 + 10_000, &op).block;
    assert_ne!(z.header.prev_hash, x.header.prev_hash);
    assert!(is_fraud_proof(&x, &z, &op.pubkey()), "stessa altezza, prev_hash diverso");
}

#[test]
fn blockhash_window_is_300_slots_across_gaps() {
    let (op, user) = (Keypair::new(), Keypair::new());
    let c = cfg(&op, &user);
    assert_eq!(c.genesis.blockhash_window, 300);
    let t0 = c.genesis.timestamp_ms;
    let to = Address::new_unique();
    let advance = |ch: &mut Chain, h: u64| {
        while ch.height < h {
            let next = (ch.height + 10).min(h);
            ch.produce_at(next, vec![], t0 + next * 1_000, &op);
        }
    };
    // Blockhash del blocco 1.000, poi la tx nel blocco `next` (raggiunto a salti di 10).
    let included = |before: u64, next: u64| {
        let mut ch = chain(&c);
        advance(&mut ch, 1_000);
        let tx = transfer_tx(&user, &to, LAMPORTS_PER_CNFA, ch.last_hash);
        advance(&mut ch, before);
        ch.produce_at(next, vec![tx], t0 + next * 1_000, &op).block.txs.len()
    };
    assert_eq!(included(1_290, 1_300), 1, "h + 300: valido");
    assert_eq!(included(1_291, 1_301), 0, "h + 301: scaduto");

    // Duplicato attraverso i salti: la firma resta nota finché il blockhash vale.
    let mut ch = chain(&c);
    let tx = transfer_tx(&user, &to, LAMPORTS_PER_CNFA, ch.last_hash);
    assert_eq!(ch.produce_at(10, vec![tx.clone()], t0 + 10_000, &op).block.txs.len(), 1);
    advance(&mut ch, 240);
    assert_eq!(ch.produce_at(250, vec![tx], t0 + 250_000, &op).block.txs.len(), 0);
    // Il blockhash del blocco 250 vale fino al blocco 550 (= lastValidBlockHeight).
    let bh = ch.last_hash;
    advance(&mut ch, 549);
    assert!(ch.blockhash_valid(&bh), "il prossimo blocco può essere il 550");
    advance(&mut ch, 550);
    assert!(!ch.blockhash_valid(&bh), "il prossimo blocco è almeno il 551");
}

#[tokio::test(flavor = "multi_thread")]
async fn node_produces_on_demand_with_heartbeat() {
    let (op, user) = (Keypair::new(), Keypair::new());
    let mut c = cfg(&op, &user);
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("slots_node");
    let _ = std::fs::remove_dir_all(&dir);
    c.node.data_dir = dir;
    c.genesis.slot_ms = 20;
    c.genesis.heartbeat_slots = 10; // senza tx: un blocco ogni 200 ms, salto 10
    c.node.rpc_addr = "127.0.0.1:19150".into();
    c.node.stream_addr = "127.0.0.1:19152".into();
    let running = node::start(c.clone(), Keypair::try_from(op.to_bytes().as_slice()).unwrap()).unwrap();
    let gw = running.gateway.clone();
    tokio::spawn(running.serve_network());
    let state = || async { gw.query(|n: &mut Node| (n.chain.height, n.chain.last_hash, n.index.count as usize)).await.unwrap() };

    tokio::time::sleep(Duration::from_millis(300)).await;
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
        assert_eq!(w[1] - w[0], 10, "senza tx il salto è heartbeat_slots: {heights:?}");
    }

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
