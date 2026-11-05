//! Hash di stato incrementale (LtHash): dopo ogni blocco coincide con il ricalcolo
//! completo, anche dopo riavvio (snapshot + journal, solo journal), riesecuzione e
//! replica dallo stream. Blocchi vari: trasferimenti, account svuotati, tx fallite,
//! token SPL, nonce durevoli (anche falliti), fee mining.

use std::path::PathBuf;

use base64::Engine;
use centrifuga::exec::Chain;
use centrifuga::genesis::{self, Config, LAMPORTS_PER_CNFA, Prefund, transfer_tx};
use centrifuga::node::Node;
use centrifuga::stream::{self, Replica};
use serde_json::json;
use solana_account::ReadableAccount;
use solana_address::Address;
use solana_hash::Hash;
use solana_instruction::{AccountMeta, Instruction};
use solana_keypair::Keypair;
use solana_message::{Message, VersionedMessage};
use solana_signer::Signer;
use solana_transaction::versioned::VersionedTransaction;

const EXAMPLE: &str = include_str!("../centrifuga.example.toml");
const TOKEN: &str = "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA";
const ATA: &str = "ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL";

fn tx(payer: &Keypair, signers: &[&Keypair], ixs: &[Instruction], bh: Hash) -> VersionedTransaction {
    let msg = Message::new_with_blockhash(ixs, Some(&payer.pubkey()), &bh);
    let mut all = vec![payer];
    all.extend_from_slice(signers);
    VersionedTransaction::try_new(VersionedMessage::Legacy(msg), &all).unwrap()
}

/// Mint (6 decimali), account associato di `owner` e 1000 token: tre programmi, molti account.
fn token_txs(owner: &Keypair, bh: Hash, rent_mint: u64) -> VersionedTransaction {
    let (mint, token, ata) = (Keypair::new(), TOKEN.parse::<Address>().unwrap(), ATA.parse::<Address>().unwrap());
    let create = solana_system_interface::instruction::create_account(&owner.pubkey(), &mint.pubkey(), rent_mint, 82, &token);
    let mut init = vec![20, 6];
    init.extend_from_slice(owner.pubkey().as_ref());
    init.push(0);
    let init = Instruction { program_id: token, accounts: vec![AccountMeta::new(mint.pubkey(), false)], data: init };
    let dest = Address::find_program_address(&[owner.pubkey().as_ref(), token.as_ref(), mint.pubkey().as_ref()], &ata).0;
    let metas = vec![
        AccountMeta::new(owner.pubkey(), true),
        AccountMeta::new(dest, false),
        AccountMeta::new_readonly(owner.pubkey(), false),
        AccountMeta::new_readonly(mint.pubkey(), false),
        AccountMeta::new_readonly(solana_sdk_ids::system_program::id(), false),
        AccountMeta::new_readonly(token, false),
    ];
    let create_ata = Instruction { program_id: ata, accounts: metas, data: vec![1] };
    let mut data = vec![7];
    data.extend_from_slice(&1_000_000_000u64.to_le_bytes());
    let mint_to = Instruction {
        program_id: token,
        accounts: vec![AccountMeta::new(mint.pubkey(), false), AccountMeta::new(dest, false), AccountMeta::new_readonly(owner.pubkey(), true)],
        data,
    };
    tx(owner, &[&mint], &[create, init, create_ata, mint_to], bh)
}

fn nonce_value(c: &Chain, k: &Address) -> Hash {
    use solana_nonce::state::State;
    use solana_nonce::versions::Versions;
    let v: Versions = wincode::deserialize(c.svm.get(k).unwrap().data()).unwrap();
    match v.state() {
        State::Initialized(d) => d.blockhash(),
        State::Uninitialized => panic!("nonce non inizializzato"),
    }
}

/// Stato completo per la replica, nel formato dello stream.
fn full_sync(c: &Chain) -> String {
    let b64 = |d: &[u8]| base64::engine::general_purpose::STANDARD.encode(d);
    let accounts: Vec<_> = c.svm.accounts().iter().map(|(k, a)| json!({
        "pubkey": k.to_string(), "lamports": a.lamports(), "owner": a.owner().to_string(),
        "data": b64(a.data()), "executable": a.executable(), "rent_epoch": a.rent_epoch(),
    })).collect();
    json!({"type": "sync", "height": c.height, "full": true, "accounts": accounts}).to_string()
}

fn assert_same(n: &Node, what: &str) {
    assert_eq!(n.chain.state_hash(), n.chain.full_state_hash(), "{what}: incrementale != completo");
}

#[test]
fn incremental_equals_full_everywhere() {
    let (op, user) = (Keypair::new(), Keypair::new());
    let mut cfg: Config = toml::from_str(EXAMPLE).unwrap();
    cfg.genesis.operator = op.pubkey().to_string();
    cfg.genesis.network = "devnet".into();
    cfg.genesis.chain_id = "centrifuga:devnet".into();
    cfg.genesis.prefund.push(Prefund { pubkey: user.pubkey().to_string(), lamports: 100 * LAMPORTS_PER_CNFA });
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("lthash");
    let _ = std::fs::remove_dir_all(&dir);
    cfg.node.data_dir = dir.clone();
    cfg.node.snapshot_every = 4;
    let kp = || Keypair::try_from(op.to_bytes().as_slice()).unwrap();
    let mut n = Node::open(cfg.clone(), kp()).unwrap();
    assert_same(&n, "genesis");
    let t0 = cfg.genesis.timestamp_ms;
    let mut replica = Replica::new(op.pubkey(), cfg.genesis.params().unwrap().rules);
    replica.max_future_ms = None;
    let (drained, nonce) = (Keypair::new(), Keypair::new());
    let mut blocks = vec![];
    let mut t = t0;
    for i in 0..24u64 {
        let bh = n.chain.last_hash;
        let rent = n.chain.svm.rent_exempt(82);
        let txs: Vec<VersionedTransaction> = match i % 8 {
            // Account nuovi e ripetuti.
            0 => (0..5).map(|j| transfer_tx(&user, &Address::new_from_array([(j + i as u8) % 7 + 1; 32]), 1_000_000 + i, bh)).collect(),
            // Un account riceve esattamente importo + fee e poi si svuota a zero.
            1 => vec![transfer_tx(&user, &drained.pubkey(), 2_000_000 + 5_000, bh)],
            2 => vec![transfer_tx(&drained, &Address::new_unique(), 2_000_000, bh)],
            // Tx fallita: solo la fee viene addebitata.
            3 => vec![transfer_tx(&user, &Address::new_unique(), 1_000 * LAMPORTS_PER_CNFA, bh)],
            4 => vec![token_txs(&user, bh, rent)],
            // Nonce durevole: creazione, poi una tx con nonce che fallisce (il nonce avanza).
            5 if i == 5 => {
                let ixs = solana_system_interface::instruction::create_nonce_account(&user.pubkey(), &nonce.pubkey(), &user.pubkey(), LAMPORTS_PER_CNFA);
                vec![tx(&user, &[&nonce], &ixs, bh)]
            }
            5 => {
                let adv = solana_system_interface::instruction::advance_nonce_account(&nonce.pubkey(), &user.pubkey());
                let too_much = solana_system_interface::instruction::transfer(&user.pubkey(), &Address::new_unique(), 1_000 * LAMPORTS_PER_CNFA);
                vec![tx(&user, &[], &[adv, too_much], nonce_value(&n.chain, &nonce.pubkey()))]
            }
            // Blocchi vuoti con salto di altezza (solo clock, fee mining a zero).
            _ => vec![],
        };
        let sent = txs.len();
        let before_nonce = n.chain.svm.get(&nonce.pubkey()).map(|_| nonce_value(&n.chain, &nonce.pubkey()));
        n.ingress.submit(None, txs);
        t += 300;
        let ex = n.tick(t).unwrap();
        assert_eq!(ex.block.txs.len(), sent, "blocco {i}: tutte le tx incluse");
        if i % 8 == 5 && i > 5 {
            assert!(ex.results[0].is_err(), "la tx con nonce fallisce");
            assert_ne!(Some(nonce_value(&n.chain, &nonce.pubkey())), before_nonce, "il nonce avanza comunque");
        }
        assert_same(&n, &format!("blocco {}", ex.block.header.height));
        assert_eq!(ex.block.header.state_hash, n.chain.full_state_hash());
        if blocks.is_empty() {
            replica.apply(&stream::header_message(&ex.block)).unwrap();
            replica.apply(&full_sync(&n.chain)).unwrap();
        } else {
            replica.apply(&stream::block_message(&n, &ex)).unwrap();
        }
        assert_eq!(replica.state_hash(), replica.full_state_hash(), "replica: incrementale != completo");
        assert_eq!(replica.state_hash(), ex.block.header.state_hash, "replica != nodo");
        blocks.push(ex.block);
    }
    assert!(n.chain.svm.get(&nonce.pubkey()).is_some(), "nonce creato");
    let (h, state) = (n.chain.last_hash, n.chain.state_hash());
    drop(n);

    // Riavvio da snapshot + journal.
    let n = Node::open(cfg.clone(), kp()).unwrap();
    assert_eq!((n.chain.last_hash, n.chain.state_hash()), (h, state));
    assert_same(&n, "riavvio da snapshot");
    drop(n);
    // Riavvio dal solo journal.
    std::fs::remove_file(dir.join("snapshot.bin")).unwrap();
    let n = Node::open(cfg.clone(), kp()).unwrap();
    assert_eq!((n.chain.last_hash, n.chain.state_hash()), (h, state));
    assert_same(&n, "riavvio dal journal");

    // Riesecuzione indipendente dei blocchi firmati.
    let mut c = genesis::build(&cfg.genesis).unwrap().0;
    for b in &blocks {
        c.apply(b).unwrap();
        assert_eq!(c.state_hash(), c.full_state_hash());
    }
    assert_eq!(c.state_hash(), state);
}
