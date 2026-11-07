//! Passo 1: genesis, transfer, blocco firmato, riavvio dal journal, stesso hash di stato.

use std::path::PathBuf;

use centrifuga::exec::Chain;
use centrifuga::genesis::{self, Config, LAMPORTS_PER_CNFA, Prefund, SUPPLY_LAMPORTS, transfer_tx};
use centrifuga::node::{Node, fmt_amount};
use solana_account::ReadableAccount;
use solana_address::Address;
use solana_keypair::Keypair;
use solana_signer::Signer;

const EXAMPLE: &str = include_str!("../centrifuga.example.toml");

fn config(dir: &str, op: &Keypair, payer: &Keypair) -> Config {
    let mut cfg: Config = toml::from_str(EXAMPLE).unwrap();
    cfg.genesis.operator = op.pubkey().to_string();
    cfg.genesis.network = "devnet".into();
    cfg.genesis.chain_id = "centrifuga:devnet".into();
    cfg.genesis.prefund.push(Prefund { pubkey: payer.pubkey().to_string(), lamports: 10 * LAMPORTS_PER_CNFA });
    let d = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(dir);
    let _ = std::fs::remove_dir_all(&d);
    cfg.node.data_dir = d;
    cfg
}

fn chain(cfg: &Config) -> Chain {
    genesis::build(&cfg.genesis).unwrap().0
}

#[test]
fn genesis_conservation_and_programs() {
    let (op, payer) = (Keypair::new(), Keypair::new());
    let cfg = config("g", &op, &payer);
    let (c, s) = genesis::build(&cfg.genesis).unwrap();
    assert_eq!(c.total_lamports(), SUPPLY_LAMPORTS as u128);
    // Account di sistema dell'ordine di qualche CnFA (≈6,35).
    assert!(s.system_lamports > LAMPORTS_PER_CNFA && s.system_lamports < 10 * LAMPORTS_PER_CNFA, "{}", s.system_lamports);
    let progs = [
        "11111111111111111111111111111111",
        "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA",
        "TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb",
        "ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL",
        "ComputeBudget111111111111111111111111111111",
        "MemoSq4gqABAXKb96qnH8TysNcWxMyWCqXgDLGmfcHr",
        "Ed25519SigVerify111111111111111111111111111",
        "KeccakSecp256k11111111111111111111111111111",
    ];
    for p in progs {
        let a = c.svm.get(&genesis::parse_address(p).unwrap()).unwrap_or_else(|| panic!("{p} mancante"));
        assert!(a.executable(), "{p}");
    }
    let rent = c.svm.rent_exempt(genesis::VAULT_LEN);
    assert_eq!(c.svm.lamports(&genesis::vault_address("airdrop")) - rent, 20_000_000 * LAMPORTS_PER_CNFA);
    assert_eq!(c.svm.lamports(&genesis::vault_address("mining")) - rent, 30_000_000 * LAMPORTS_PER_CNFA);
    // Determinismo: due costruzioni, stesso hash.
    assert_eq!(chain(&cfg).genesis_hash, c.genesis_hash);
    genesis::check_determinism(&cfg.genesis, 5).unwrap();
}

#[test]
fn transfer_signed_block_and_replay() {
    let (op, payer) = (Keypair::new(), Keypair::new());
    let cfg = config("t", &op, &payer);
    let mut a = chain(&cfg);
    let to = Address::new_unique();
    let t0 = cfg.genesis.timestamp_ms;
    let tx = transfer_tx(&payer, &to, 2 * LAMPORTS_PER_CNFA, a.last_hash);
    let ex = a.produce(vec![tx.clone()], t0 + 100, &op);
    assert_eq!(ex.block.txs.len(), 1);
    assert!(ex.results[0].is_ok());
    assert!(ex.block.verify_signature(&op.pubkey()));
    assert!(!ex.block.verify_signature(&payer.pubkey()));
    assert_eq!(a.svm.lamports(&to), 2 * LAMPORTS_PER_CNFA);
    // Fee mining (passo 2): 10% all'operatore, premio 90% al pagatore.
    assert_eq!(a.svm.lamports(&payer.pubkey()), 8 * LAMPORTS_PER_CNFA - 500);
    assert_eq!(a.svm.lamports(&op.pubkey()), 500);
    assert_eq!(a.total_lamports(), SUPPLY_LAMPORTS as u128);

    // Duplicato e blockhash sconosciuto: esclusi dal blocco.
    let stale = transfer_tx(&payer, &to, 1, solana_hash::Hash::new_from_array([9; 32]));
    let ex2 = a.produce(vec![tx, stale], t0 + 100, &op);
    assert!(ex2.block.txs.is_empty());
    assert_eq!(ex2.block.header.timestamp_ms, t0 + 150, "timestamp >= precedente + salto × slot_ms / 2");

    // Replica: rieseguire i blocchi dà lo stesso stato; un blocco alterato è rifiutato.
    let mut b = chain(&cfg);
    let mut bad = ex.block.clone();
    bad.header.timestamp_ms += 1;
    assert!(chain(&cfg).apply(&bad).is_err());
    b.apply(&ex.block).unwrap();
    b.apply(&ex2.block).unwrap();
    assert_eq!(b.last_hash, a.last_hash);
}

#[test]
fn restart_from_journal_and_snapshot() {
    let (op, payer) = (Keypair::new(), Keypair::new());
    let mut cfg = config("r", &op, &payer);
    cfg.node.snapshot_every = 4;
    let kp = || Keypair::try_from(op.to_bytes().as_slice()).unwrap();
    let mut n = Node::open(cfg.clone(), kp()).unwrap();
    let t0 = cfg.genesis.timestamp_ms;
    for i in 1..=10u64 {
        let tx = transfer_tx(&payer, &Address::new_from_array([i as u8; 32]), 1_000_000 * i, n.chain.last_hash);
        assert_eq!(n.ingress.submit(None, vec![tx]), vec![Ok(i - 1)]);
        n.tick(t0 + i * 100).unwrap();
    }
    let (h, state) = (n.chain.last_hash, n.chain.state_hash());
    drop(n);

    // Riavvio: snapshot all'altezza 8 + journal 9..10.
    let mut n = Node::open(cfg.clone(), kp()).unwrap();
    assert_eq!((n.chain.height, n.chain.last_hash), (10, h));
    assert_eq!(n.chain.state_hash(), state);
    // Il blockhash del blocco 9 è ancora valido dopo il riavvio.
    let tx = transfer_tx(&payer, &Address::new_unique(), 1, n.chain.last_hash);
    n.ingress.submit(None, vec![tx]);
    assert_eq!(n.tick(t0 + 1100).unwrap().block.txs.len(), 1);
    let h11 = n.chain.last_hash;
    drop(n);

    // Riavvio da solo journal (senza snapshot): stesso risultato.
    std::fs::remove_file(cfg.node.data_dir.join("snapshot.bin")).unwrap();
    let n = Node::open(cfg, kp()).unwrap();
    assert_eq!((n.chain.height, n.chain.last_hash), (11, h11));
}

#[test]
fn invalid_signature_rejected_by_ingress() {
    let (payer, to) = (Keypair::new(), Address::new_unique());
    let mut tx = transfer_tx(&payer, &to, 1, solana_hash::Hash::default());
    let good = tx.clone();
    tx.signatures[0] = Keypair::new().sign_message(b"x");
    assert_eq!(centrifuga::ingress::verify_signatures(&[good, tx]), vec![true, false]);
}

#[test]
fn amount_format() {
    assert_eq!(fmt_amount(12_500_000_000, "CnFA"), "12.5 CnFA");
    assert_eq!(fmt_amount(5000, "CnFA"), "0.000005 CnFA");
    assert_eq!(fmt_amount(3 * LAMPORTS_PER_CNFA, "CnFA"), "3 CnFA");
}

#[test]
fn spl_token_programs_are_live() {
    use solana_instruction::{AccountMeta, Instruction};
    use solana_message::{Message, VersionedMessage};
    use solana_transaction::versioned::VersionedTransaction;
    let (op, payer) = (Keypair::new(), Keypair::new());
    let cfg = config("s", &op, &payer);
    let mut c = chain(&cfg);
    for token in ["TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA", "TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb"] {
        let token = genesis::parse_address(token).unwrap();
        let mint = Keypair::new();
        let create = solana_system_interface::instruction::create_account(&payer.pubkey(), &mint.pubkey(), c.svm.rent_exempt(82), 82, &token);
        let mut data = vec![20, 9];
        data.extend_from_slice(payer.pubkey().as_ref());
        data.push(0);
        let init = Instruction { program_id: token, accounts: vec![AccountMeta::new(mint.pubkey(), false)], data };
        let msg = Message::new_with_blockhash(&[create, init], Some(&payer.pubkey()), &c.last_hash);
        let tx = VersionedTransaction::try_new(VersionedMessage::Legacy(msg), &[&payer, &mint]).unwrap();
        let ex = c.produce(vec![tx], cfg.genesis.timestamp_ms + 100 * (c.height + 1), &op);
        assert!(ex.results[0].is_ok(), "{token}: {:?}", ex.results[0]);
        assert_eq!(c.svm.get(&mint.pubkey()).unwrap().owner(), &token);
    }
}
