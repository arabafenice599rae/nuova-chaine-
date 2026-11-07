//! Confronto tra tre hash di stato sugli stessi blocchi reali, con ricerca a gradini
//! del carico massimo sostenibile (p99 del blocco < 100 ms):
//!   A. LtHash a ogni blocco (quello nel nodo);
//!   B. Merkle incrementale a ogni blocco, con prove per account;
//!   C. ibrido denso: LtHash a ogni blocco + radice B ogni secondo (10 blocchi), su un
//!      thread separato con i soli account cambiati;
//!   D. Merkle sparso (posizione = hash della pubkey, prove di appartenenza e assenza);
//!   E. ibrido sparso: LtHash a ogni blocco + radice D ogni secondo, su un thread.
//! B e C (denso) sono solo un riferimento: la radice dipende dalla storia.
//!   cargo run --release --example compare -- <accounts> [blocks_per_step] [--steps 2000,2100,...] [--spl]
//! --spl: ogni tx crea (CreateIdempotent) l'account token associato del destinatario
//! e gli trasferisce un token SPL (TransferChecked), invece di un trasferimento CnFA.
//! A e B dallo stesso passaggio: B = blocco - hash LtHash misurato + aggiornamento
//! Merkle sugli stessi account. C in un secondo passaggio sugli stessi blocchi, con il
//! thread Merkle davvero in esecuzione (compete per la CPU con il nodo).

#[path = "common/merkle.rs"]
mod merkle;
#[path = "common/smt.rs"]
mod smt;

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::mpsc;
use std::time::Instant;

use centrifuga::exec::Phases;
use centrifuga::genesis::{self, Config, LAMPORTS_PER_CNFA, Prefund, transfer_tx};
use centrifuga::node::Node;
use merkle::Merkle;
use solana_account::{Account, AccountSharedData};
use solana_address::Address;
use solana_instruction::{AccountMeta, Instruction};
use solana_keypair::Keypair;
use solana_message::{Message, VersionedMessage};
use solana_signer::Signer;
use solana_transaction::versioned::VersionedTransaction;

const EXAMPLE: &str = include_str!("../centrifuga.example.toml");
const SENDERS: usize = 2_000;
const PER_ROOT: usize = 10;
const STEPS: [usize; 10] = [500, 1000, 1500, 2000, 2500, 3000, 3500, 4000, 4500, 5000];
const LIMIT_MS: f64 = 100.0;
const TOKEN: &str = "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA";
const ATA: &str = "ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL";

fn ata(owner: &Address, mint: &Address) -> Address {
    let (token, ata) = (TOKEN.parse::<Address>().unwrap(), ATA.parse::<Address>().unwrap());
    Address::find_program_address(&[owner.as_ref(), token.as_ref(), mint.as_ref()], &ata).0
}

/// Mint SPL (82 byte): autorità, supply, 6 decimali, inizializzato.
fn mint_data(authority: &Address, supply: u64) -> Vec<u8> {
    let mut d = vec![0u8; 82];
    d[..4].copy_from_slice(&1u32.to_le_bytes());
    d[4..36].copy_from_slice(authority.as_ref());
    d[36..44].copy_from_slice(&supply.to_le_bytes());
    d[44] = 6;
    d[45] = 1;
    d
}

/// Account token (165 byte): mint, proprietario, saldo, stato inizializzato.
fn token_account_data(mint: &Address, owner: &Address, amount: u64) -> Vec<u8> {
    let mut d = vec![0u8; 165];
    d[..32].copy_from_slice(mint.as_ref());
    d[32..64].copy_from_slice(owner.as_ref());
    d[64..72].copy_from_slice(&amount.to_le_bytes());
    d[108] = 1;
    d
}

/// Crea (se manca) l'account associato del destinatario e gli trasferisce `amount`.
fn spl_tx(from: &Keypair, to: &Address, mint: &Address, amount: u64, bh: solana_hash::Hash) -> VersionedTransaction {
    let (token, ata_id) = (TOKEN.parse::<Address>().unwrap(), ATA.parse::<Address>().unwrap());
    let (src, dst) = (ata(&from.pubkey(), mint), ata(to, mint));
    let create = Instruction {
        program_id: ata_id,
        accounts: vec![
            AccountMeta::new(from.pubkey(), true),
            AccountMeta::new(dst, false),
            AccountMeta::new_readonly(*to, false),
            AccountMeta::new_readonly(*mint, false),
            AccountMeta::new_readonly(solana_sdk_ids::system_program::id(), false),
            AccountMeta::new_readonly(token, false),
        ],
        data: vec![1],
    };
    let mut data = vec![12];
    data.extend_from_slice(&amount.to_le_bytes());
    data.push(6);
    let transfer = Instruction {
        program_id: token,
        accounts: vec![
            AccountMeta::new(src, false),
            AccountMeta::new_readonly(*mint, false),
            AccountMeta::new(dst, false),
            AccountMeta::new_readonly(from.pubkey(), true),
        ],
        data,
    };
    let msg = Message::new_with_blockhash(&[create, transfer], Some(&from.pubkey()), &bh);
    VersionedTransaction::try_new(VersionedMessage::Legacy(msg), &[from]).unwrap()
}

fn rss_mb() -> u64 {
    let s = std::fs::read_to_string("/proc/self/status").unwrap_or_default();
    s.lines().find(|l| l.starts_with("VmRSS:")).and_then(|l| l.split_whitespace().nth(1)?.parse::<u64>().ok()).unwrap_or(0) / 1024
}

fn addr(i: u64) -> Address {
    let mut b = [0u8; 32];
    b[..8].copy_from_slice(&i.to_le_bytes());
    b[8] = 0xCA;
    Address::new_from_array(b)
}

fn pct(v: &[f64], q: f64) -> f64 {
    let mut s = v.to_vec();
    s.sort_by(f64::total_cmp);
    s[((s.len() - 1) as f64 * q).round() as usize]
}

/// Un blocco: tempo totale e fasi (ms) per la variante.
#[derive(Clone, Copy, Default)]
struct B {
    total: f64,
    exec: f64,
    hash: f64,
    sign: f64,
    journal: f64,
    other: f64,
}

fn ms(d: std::time::Duration) -> f64 {
    d.as_secs_f64() * 1e3
}

fn from(p: &Phases, hash: f64) -> B {
    let (exec, sign, journal, other) = (ms(p.exec), ms(p.sign), ms(p.journal), ms(p.other));
    B { total: exec + hash + sign + journal + other, exec, hash, sign, journal, other }
}

/// Albero di Merkle (denso o sparso) usato da una variante.
trait Tree: Send + 'static {
    fn build(a: &HashMap<Address, AccountSharedData>) -> Self;
    fn update(&mut self, changed: &[(Address, Option<&AccountSharedData>)]);
    fn root(&self) -> [u8; 32];
}

impl Tree for Merkle {
    fn build(a: &HashMap<Address, AccountSharedData>) -> Self {
        Merkle::build(a)
    }
    fn update(&mut self, c: &[(Address, Option<&AccountSharedData>)]) {
        Merkle::update(self, c)
    }
    fn root(&self) -> [u8; 32] {
        Merkle::root(self)
    }
}

impl Tree for smt::Sparse {
    fn build(a: &HashMap<Address, AccountSharedData>) -> Self {
        smt::Sparse::build(a)
    }
    fn update(&mut self, c: &[(Address, Option<&AccountSharedData>)]) {
        smt::Sparse::update(self, c)
    }
    fn root(&self) -> [u8; 32] {
        smt::Sparse::root(self)
    }
}

const NAMES: [&str; 5] = ["LtHash", "Merkle denso (rif.)", "ibrido denso (rif.)", "Merkle sparso", "ibrido sparso"];

struct Step {
    load: usize,
    v: [Vec<B>; 5],
    root_ms: [Vec<f64>; 2],
}

/// Passaggio con il thread della radice ogni secondo (ibrido): tempo del blocco sul
/// thread del nodo = blocco + copia degli account cambiati.
fn hybrid<T: Tree>(node: &mut Node, all: &[Vec<VersionedTransaction>], load: usize) -> (Vec<B>, Vec<f64>) {
    let mut tree = T::build(node.chain.svm.accounts());
    let (tx, rx) = mpsc::channel::<Vec<(Address, Option<AccountSharedData>)>>();
    let worker = std::thread::spawn(move || {
        let mut times = vec![];
        for batch in rx {
            let t = Instant::now();
            let list: Vec<(Address, Option<&AccountSharedData>)> = batch.iter().map(|(k, a)| (*k, a.as_ref())).collect();
            tree.update(&list);
            std::hint::black_box(tree.root()); // qui la radice verrebbe firmata
            times.push(t.elapsed().as_secs_f64() * 1e3);
        }
        times
    });
    let mut c = vec![];
    let mut pending: HashMap<Address, Option<AccountSharedData>> = HashMap::new();
    let mut now = centrifuga::node::now_ms();
    for (i, txs) in all.iter().enumerate() {
        assert!(node.ingress.submit(None, txs[..load].to_vec()).iter().all(|x| x.is_ok()));
        now += 100;
        let ex = node.tick(now).unwrap();
        let t = Instant::now();
        let acc = node.chain.svm.accounts();
        for k in &ex.changed {
            pending.insert(*k, acc.get(k).cloned());
        }
        if i % PER_ROOT == PER_ROOT - 1 {
            tx.send(pending.drain().collect()).unwrap();
        }
        c.push(from(&ex.phases, ms(ex.phases.hash) + t.elapsed().as_secs_f64() * 1e3));
    }
    drop(tx);
    (c, worker.join().unwrap())
}

fn main() {
    let raw: Vec<String> = std::env::args().skip(1).collect();
    let spl = raw.iter().any(|a| a == "--spl");
    let steps_arg = raw.iter().position(|a| a == "--steps").map(|i| raw[i + 1].split(',').map(|x| x.parse::<usize>().unwrap()).collect::<Vec<_>>());
    let step_list = steps_arg.clone().unwrap_or(STEPS.to_vec());
    let args: Vec<u64> = raw
        .iter()
        .enumerate()
        .filter(|(i, a)| !a.starts_with("--") && !(*i > 0 && raw[i - 1] == "--steps"))
        .map(|(_, a)| a.replace('_', "").parse().expect("number"))
        .collect();
    let (n, blocks) = (args[0], *args.get(1).unwrap_or(&100) as usize);
    let mint = addr(u64::MAX);
    let op = Keypair::new();
    let senders: Vec<Keypair> = (0..SENDERS).map(|_| Keypair::new()).collect();
    let mut cfg: Config = toml::from_str(EXAMPLE).unwrap();
    cfg.genesis.operator = op.pubkey().to_string();
    cfg.genesis.network = "devnet".into();
    cfg.genesis.chain_id = "centrifuga:devnet".into();
    cfg.genesis.timestamp_ms = centrifuga::node::now_ms() - 86_400_000;
    cfg.genesis.prefund = senders.iter().map(|k| Prefund { pubkey: k.pubkey().to_string(), lamports: 1_000 * LAMPORTS_PER_CNFA }).collect();
    cfg.node.snapshot_every = 0;
    cfg.node.ingress_pending_per_payer = u32::MAX;
    let dir = std::env::var("LOAD_DIR").map(PathBuf::from).unwrap_or_else(|_| std::env::temp_dir().join("centrifuga-compare"));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    cfg.node.data_dir = dir.clone();

    let mut chain = genesis::build(&cfg.genesis).unwrap().0;
    let rent = chain.svm.rent_exempt(0);
    let system = solana_sdk_ids::system_program::id();
    for i in 0..n {
        chain.svm.set(addr(i), Account { lamports: rent, owner: system, ..Account::default() });
    }
    if spl {
        // Mint e account token dei mittenti, creati direttamente nello stato iniziale.
        let token = TOKEN.parse::<Address>().unwrap();
        let (mrent, trent) = (chain.svm.rent_exempt(82), chain.svm.rent_exempt(165));
        chain.svm.set(mint, Account { lamports: mrent, data: mint_data(&op.pubkey(), u64::MAX / 2), owner: token, ..Account::default() });
        for k in &senders {
            let data = token_account_data(&mint, &k.pubkey(), u64::MAX / 8_000);
            chain.svm.set(ata(&k.pubkey(), &mint), Account { lamports: trent, data, owner: token, ..Account::default() });
        }
    }
    chain.rehash();
    centrifuga::ledger::write_snapshot(&dir, 0, chain.genesis_hash, chain.full_state_hash(), chain.svm.accounts().iter()).unwrap();
    let snap_mb = std::fs::metadata(centrifuga::ledger::snapshot_path(&dir)).unwrap().len() as f64 / 1e6;
    drop(chain);
    let kp = || Keypair::try_from(op.to_bytes().as_slice()).unwrap();
    let open = |cfg: &Config| {
        let _ = std::fs::remove_file(dir.join("journal.bin"));
        Node::open(cfg.clone(), kp()).unwrap()
    };

    let t = Instant::now();
    let node = open(&cfg);
    let restart_lt = t.elapsed().as_secs_f64();
    let base_ram = rss_mb();
    let bh = node.chain.last_hash;
    let mut seed = 0x9E37_79B9_7F4A_7C15u64;
    let mut rnd = move || {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        seed
    };
    let acc = node.chain.svm.accounts();
    let ks: Vec<Address> = (0..10_000).map(|_| addr(rnd() % n)).collect();
    // Denso: RAM, costruzione, prove.
    let r0 = rss_mb();
    let t = Instant::now();
    let mk0 = Merkle::build(acc);
    let mk_build = t.elapsed().as_secs_f64();
    let mk_ram = rss_mb().saturating_sub(r0);
    let root = mk0.root();
    let proofs: Vec<_> = ks.iter().map(|k| mk0.proof(k)).collect();
    let t = Instant::now();
    for (k, (i, sib)) in ks.iter().zip(&proofs) {
        assert!(merkle::verify(&root, k, &acc[k], *i, sib));
    }
    let mk_verify = t.elapsed().as_secs_f64() * 1e6 / ks.len() as f64;
    let mk_proof = 8 + 32 * proofs[0].1.len();
    drop((mk0, proofs));
    // Sparso: RAM, costruzione, prove di appartenenza e di assenza.
    let r0 = rss_mb();
    let t = Instant::now();
    let sp0 = smt::Sparse::build(acc);
    let sp_build = t.elapsed().as_secs_f64();
    let sp_ram = rss_mb().saturating_sub(r0);
    let root = sp0.root();
    let proofs: Vec<Vec<u8>> = ks.iter().map(|k| sp0.proof(k)).collect();
    let t = Instant::now();
    for (k, p) in ks.iter().zip(&proofs) {
        assert!(smt::verify(&root, k, Some(&acc[k]), p));
    }
    let sp_verify = t.elapsed().as_secs_f64() * 1e6 / ks.len() as f64;
    let sp_proof = proofs.iter().map(|p| p.len()).sum::<usize>() / proofs.len();
    let absent: Vec<Address> = (0..1_000).map(|_| Address::new_unique()).collect();
    let ap: Vec<Vec<u8>> = absent.iter().map(|k| sp0.proof(k)).collect();
    let t = Instant::now();
    for (k, p) in absent.iter().zip(&ap) {
        assert!(smt::verify(&root, k, None, p));
    }
    let sp_absent_us = t.elapsed().as_secs_f64() * 1e6 / absent.len() as f64;
    let sp_absent = ap.iter().map(|p| p.len()).sum::<usize>() / ap.len();
    drop((sp0, proofs));
    drop(node);

    // Tx firmate in anticipo per il carico massimo; ogni gradino usa i primi `load` di ogni blocco.
    let max = *step_list.iter().max().unwrap();
    let all: Vec<Vec<VersionedTransaction>> = (0..blocks as u64)
        .map(|b| {
            (0..max as u64)
                .map(|j| {
                    let (from, to, amount) = (&senders[(j as usize) % SENDERS], addr(rnd() % n.max(1)), 1 + b * max as u64 + j);
                    if spl { spl_tx(from, &to, &mint, amount, bh) } else { transfer_tx(from, &to, amount, bh) }
                })
                .collect()
        })
        .collect();

    let mut steps: Vec<Step> = vec![];
    let mut peak_ram = 0;
    for &load in &step_list {
        cfg.node.max_block_txs = load;
        // Passaggio 1: A, B e D sugli stessi blocchi (B e D: costo LtHash tolto e sostituito).
        let mut node = open(&cfg);
        let (mut mk, mut sp) = (Merkle::build(node.chain.svm.accounts()), smt::Sparse::build(node.chain.svm.accounts()));
        let (mut a, mut b, mut d) = (vec![], vec![], vec![]);
        let mut now = centrifuga::node::now_ms();
        for txs in &all {
            assert!(node.ingress.submit(None, txs[..load].to_vec()).iter().all(|x| x.is_ok()));
            now += 100;
            let ex = node.tick(now).unwrap();
            assert_eq!(ex.block.txs.len(), load);
            assert!(ex.results.iter().all(|r| r.is_ok()), "tx fallita nel carico di prova");
            let acc = node.chain.svm.accounts();
            let list: Vec<(Address, Option<&AccountSharedData>)> = ex.changed.iter().map(|k| (*k, acc.get(k))).collect();
            let t = Instant::now();
            mk.update(&list);
            std::hint::black_box(mk.root());
            let mk_ms = t.elapsed().as_secs_f64() * 1e3;
            let t = Instant::now();
            sp.update(&list);
            std::hint::black_box(sp.root());
            let sp_ms = t.elapsed().as_secs_f64() * 1e3;
            a.push(from(&ex.phases, ms(ex.phases.hash)));
            b.push(from(&ex.phases, mk_ms));
            d.push(from(&ex.phases, sp_ms));
        }
        assert_eq!(mk.root(), mk.full_root(node.chain.svm.accounts()), "Merkle denso incrementale = completo");
        assert_eq!(sp.root(), smt::Sparse::build(node.chain.svm.accounts()).root(), "Merkle sparso incrementale = ricostruito");
        peak_ram = peak_ram.max(rss_mb());
        drop((node, mk, sp));
        // Passaggi 2 e 3: ibridi con il thread della radice in esecuzione.
        let mut node = open(&cfg);
        let (c, rc) = hybrid::<Merkle>(&mut node, &all, load);
        drop(node);
        let mut node = open(&cfg);
        let (e, re) = hybrid::<smt::Sparse>(&mut node, &all, load);
        peak_ram = peak_ram.max(rss_mb());
        drop(node);
        let p99 = |v: &[B]| pct(&v.iter().map(|x| x.total).collect::<Vec<_>>(), 0.99);
        eprintln!("n={n} load={load}: p99 {:.1} / {:.1} / {:.1} / {:.1} / {:.1} ms", p99(&a), p99(&b), p99(&c), p99(&d), p99(&e));
        let v = [a, b, c, d, e];
        let all_over = v.iter().all(|x| p99(x) >= LIMIT_MS);
        steps.push(Step { load, v, root_ms: [rc, re] });
        if all_over {
            break;
        }
    }

    for (v, name) in NAMES.iter().enumerate().take(5) {
        let ok = steps.iter().rfind(|s| pct(&s.v[v].iter().map(|x| x.total).collect::<Vec<_>>(), 0.99) < LIMIT_MS);
        let Some(s) = ok else {
            println!("| {n} | {} | < {} tx/blocco |", name, step_list[0]);
            continue;
        };
        let bl = &s.v[v];
        let tot: Vec<f64> = bl.iter().map(|x| x.total).collect();
        let mean = |f: fn(&B) -> f64| bl.iter().map(f).sum::<f64>() / bl.len() as f64;
        let (ram, build, proof) = match v {
            0 => (format!("{base_ram}"), 0.0, "—".to_string()),
            1 | 2 => (format!("{base_ram} + {mk_ram}"), mk_build, format!("{mk_proof} B, {mk_verify:.1} µs (no assenza)")),
            _ => (format!("{base_ram} + {sp_ram}"), sp_build, format!("{sp_proof} B, {sp_verify:.1} µs; assenza {sp_absent} B, {sp_absent_us:.1} µs")),
        };
        println!(
            "| {n} | {} | {} | {} | {:.1} / {:.1} / {:.1} | {:.1} / {:.1} / {:.2} / {:.1} / {:.1} | {ram} | {snap_mb:.0} | {:.1} | {proof} |",
            name,
            s.load,
            s.load * 10,
            pct(&tot, 0.5),
            pct(&tot, 0.95),
            pct(&tot, 0.99),
            mean(|x| x.exec),
            mean(|x| x.hash),
            mean(|x| x.sign),
            mean(|x| x.journal),
            mean(|x| x.other),
            restart_lt + build,
        );
        if v == 2 || v == 4 {
            let r = &s.root_ms[(v - 2) / 2];
            println!(
                "{}: radice ogni secondo in {:.1} ms medi, max {:.1} ms (thread separato)",
                name,
                r.iter().sum::<f64>() / r.len() as f64,
                r.iter().cloned().fold(0.0, f64::max)
            );
        }
    }
    println!("RAM di picco del processo: {peak_ram} MB");
    // Riavvio dopo uno snapshot preso a fine corsa: apertura e costruzione degli alberi.
    cfg.node.max_block_txs = step_list[0];
    let mut node = open(&cfg);
    let mut now = centrifuga::node::now_ms();
    for txs in &all {
        node.ingress.submit(None, txs[..step_list[0]].to_vec());
        now += 100;
        node.tick(now).unwrap();
    }
    node.snapshot().unwrap();
    node.wait_snapshot().unwrap();
    drop(node);
    let t = Instant::now();
    let node = Node::open(cfg.clone(), kp()).unwrap();
    let reopen = t.elapsed().as_secs_f64();
    let t = Instant::now();
    drop(Merkle::build(node.chain.svm.accounts()));
    let mk_re = t.elapsed().as_secs_f64();
    let t = Instant::now();
    drop(smt::Sparse::build(node.chain.svm.accounts()));
    let sp_re = t.elapsed().as_secs_f64();
    println!(
        "albero all'avvio (snapshot iniziale): apertura {restart_lt:.2} s, denso {mk_build:.2} s, sparso {sp_build:.2} s; dopo ripristino da snapshot all'altezza {}: apertura {reopen:.2} s, denso {mk_re:.2} s, sparso {sp_re:.2} s",
        node.chain.height
    );
}
