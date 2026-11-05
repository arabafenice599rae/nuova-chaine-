//! Passo 3: aventi diritto al residuo del fee mining, calcolati dal journal.

use std::collections::BTreeSet;
use std::path::PathBuf;

use centrifuga::claims::{self, MAX_QUOTA};
use centrifuga::genesis::{Config, LAMPORTS_PER_CNFA, Prefund, transfer_tx, vault_address};
use centrifuga::node::Node;
use solana_keypair::Keypair;
use solana_signer::Signer;

const EXAMPLE: &str = include_str!("../centrifuga.example.toml");
const DAY: u64 = 86_400_000;

/// Ogni voce: (chi, in lista airdrop, tx riuscite, giorni distinti).
#[test]
fn eligibility_from_journal() {
    let op = Keypair::new();
    let users: Vec<(Keypair, bool, u64, u64)> = vec![
        (Keypair::new(), true, 10, 5),  // ha diritto
        (Keypair::new(), true, 12, 6),  // ha diritto
        (Keypair::new(), true, 10, 4),  // pochi giorni
        (Keypair::new(), true, 9, 5),   // poche tx
        (Keypair::new(), false, 10, 5), // non in lista
    ];
    let mut cfg: Config = toml::from_str(EXAMPLE).unwrap();
    cfg.genesis.network = "devnet".into();
    cfg.genesis.operator = op.pubkey().to_string();
    cfg.genesis.fee_mining_end_height = 40;
    for (k, ..) in &users {
        cfg.genesis.prefund.push(Prefund { pubkey: k.pubkey().to_string(), lamports: 10 * LAMPORTS_PER_CNFA });
    }
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("eligible");
    let _ = std::fs::remove_dir_all(&dir);
    cfg.node.data_dir = dir.clone();
    let mut node = Node::open(cfg.clone(), Keypair::try_from(op.to_bytes().as_slice()).unwrap()).unwrap();

    // Le tx di ciascuno sono spalmate sui suoi giorni (un giorno = un salto di timestamp).
    let t0 = cfg.genesis.timestamp_ms;
    for day in 0..6u64 {
        let mut txs = Vec::new();
        for (u, (k, _, n, days)) in users.iter().enumerate() {
            if day >= *days {
                continue;
            }
            let per_day = n / days + u64::from(day < n % days);
            for i in 0..per_day {
                // Trasferimento a se stessi: riesce sempre, firma diversa per importo.
                txs.push(transfer_tx(k, &k.pubkey(), 1 + day * 100 + i + u as u64 * 10_000, node.chain.last_hash));
            }
        }
        node.ingress.submit(txs);
        node.tick(t0 + day * DAY + 1_000).unwrap();
    }
    // Una tx fallita (oltre il saldo) non conta.
    let (k0, ..) = &users[3];
    let fail = solana_system_interface::instruction::transfer(&k0.pubkey(), &op.pubkey(), 100 * LAMPORTS_PER_CNFA);
    let msg = solana_message::Message::new_with_blockhash(&[fail], Some(&k0.pubkey()), &node.chain.last_hash);
    let tx = solana_transaction::versioned::VersionedTransaction::try_new(solana_message::VersionedMessage::Legacy(msg), &[k0]).unwrap();
    node.ingress.submit(vec![tx]);
    let mut t = t0 + 6 * DAY;
    while node.chain.height <= 40 {
        t += 100;
        node.tick(t).unwrap();
    }
    let residual = node.chain.svm.lamports(&vault_address("airdrop")) - node.chain.svm.rent_exempt(centrifuga::genesis::VAULT_LEN) - 20_000_000 * LAMPORTS_PER_CNFA;
    drop(node);

    let list: BTreeSet<_> = users.iter().filter(|u| u.1).map(|u| u.0.pubkey()).collect();
    let journal = dir.join("journal.bin");
    let e = claims::compute(&cfg.genesis, &journal, &list).unwrap();
    let mut expected = vec![users[0].0.pubkey(), users[1].0.pubkey()];
    expected.sort();
    assert_eq!(e.eligible, expected);
    assert_eq!(e.end_height, 40);
    assert_eq!(e.residual, residual, "residuo = quanto è passato da mining ad airdrop");
    assert_eq!(e.quota, MAX_QUOTA, "il residuo (~30M) supera 1.000 CnFA a testa");
    assert_eq!(e.unassigned, residual - 2 * MAX_QUOTA);

    // Deterministico: stesso file e stesso hash a ogni riesecuzione.
    let (file, hash) = e.render();
    let (file2, hash2) = claims::compute(&cfg.genesis, &journal, &list).unwrap().render();
    assert_eq!((file, hash), (file2, hash2));
}

#[test]
fn render_and_list_format() {
    let e = claims::Eligibility { end_height: 1, residual: 10, eligible: vec![], quota: 0, unassigned: 10 };
    assert!(e.render().0.contains("eligible=0\nquota=0\nunassigned=10"));
    let list = claims::parse_list("# commento\n11111111111111111111111111111111 # system\n\n").unwrap();
    assert_eq!(list.len(), 1);
}
