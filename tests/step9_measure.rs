//! `Node::last_tick_us` è solo una misura: non entra in blocchi, hash, journal o
//! snapshot e non cambia il comportamento del nodo.

use std::path::PathBuf;

use centrifuga::genesis::{self, Config, LAMPORTS_PER_CNFA, Prefund, transfer_tx};
use centrifuga::node::Node;
use solana_address::Address;
use solana_keypair::Keypair;
use solana_signer::Signer;

const EXAMPLE: &str = include_str!("../centrifuga.example.toml");

#[test]
fn tick_measure_does_not_touch_consensus() {
    let (op, user) = (Keypair::new_from_array([3; 32]), Keypair::new_from_array([4; 32]));
    let mut cfg: Config = toml::from_str(EXAMPLE).unwrap();
    cfg.genesis.operator = op.pubkey().to_string();
    cfg.genesis.network = "devnet".into();
    cfg.genesis.chain_id = "centrifuga:devnet".into();
    cfg.genesis.prefund.push(Prefund { pubkey: user.pubkey().to_string(), lamports: 10 * LAMPORTS_PER_CNFA });
    cfg.node.snapshot_every = 5;
    let base = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("measure");
    let _ = std::fs::remove_dir_all(&base);
    let dirs = [base.join("a"), base.join("b")];
    let open = |d: &PathBuf| {
        let mut c = cfg.clone();
        c.node.data_dir = d.clone();
        Node::open(c, Keypair::new_from_array([3; 32])).unwrap()
    };
    let (mut a, mut b) = (open(&dirs[0]), open(&dirs[1]));
    let mut chain = genesis::build(&cfg.genesis).unwrap().0; // nessun campo di misura
    let to = Address::new_from_array([9; 32]);
    let t0 = cfg.genesis.timestamp_ms;
    for i in 1..=12u64 {
        let txs: Vec<_> = (0..3).map(|j| transfer_tx(&user, &to, 1_000 * i + j, a.chain.last_hash)).collect();
        a.ingress.submit(None, txs.clone());
        b.ingress.submit(None, txs.clone());
        // Su b la misura vale qualsiasi cosa prima di ogni blocco.
        b.last_tick_us = [0, u64::MAX, 12345][i as usize % 3];
        let ea = a.tick(t0 + i * 100).unwrap();
        let eb = b.tick(t0 + i * 100).unwrap();
        let ec = chain.produce_at(ea.block.header.height, txs, t0 + i * 100, &op);
        assert!(a.last_tick_us > 0, "la misura c'è");
        assert_eq!(ea.block.hash(), eb.block.hash(), "blocco {i}: stesso hash con misure diverse");
        assert_eq!(ea.block.hash(), ec.block.hash(), "blocco {i}: stesso hash della catena senza misura");
        assert_eq!(ea.block.header.state_hash, ec.block.header.state_hash);
    }
    assert_eq!(a.chain.state_hash(&Default::default()), chain.state_hash(&Default::default()));
    drop((a, b));
    for f in ["journal.bin", "snapshot.bin", "status.bin"] {
        assert_eq!(std::fs::read(dirs[0].join(f)).unwrap(), std::fs::read(dirs[1].join(f)).unwrap(), "{f} identico");
    }
}
