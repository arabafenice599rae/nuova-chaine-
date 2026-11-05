//! Passo 2: fee mining 10/90, premio dal vault "mining", rimborso priority fee,
//! dimezzamento del budget, transizione a fine periodo o a vault esaurito.

use centrifuga::exec::{Chain, Settlement};
use centrifuga::genesis::{self, Config, LAMPORTS_PER_CNFA, Prefund, SUPPLY_LAMPORTS, transfer_tx, vault_address};
use solana_account::Account;
use solana_address::Address;
use solana_instruction::Instruction;
use solana_keypair::Keypair;
use solana_message::{Message, VersionedMessage};
use solana_signer::Signer;
use solana_transaction::versioned::VersionedTransaction;

const EXAMPLE: &str = include_str!("../centrifuga.example.toml");
const BIG: u64 = 1_000_000_000; // budget che non limita mai

struct T {
    c: Chain,
    op: Keypair,
    t: u64,
}

fn setup(payers: &[&Keypair], f: impl FnOnce(&mut Config)) -> T {
    let op = Keypair::new();
    let mut cfg: Config = toml::from_str(EXAMPLE).unwrap();
    cfg.genesis.network = "devnet".into();
    cfg.genesis.operator = op.pubkey().to_string();
    for p in payers.iter().copied().chain([&op]) {
        cfg.genesis.prefund.push(Prefund { pubkey: p.pubkey().to_string(), lamports: 10 * LAMPORTS_PER_CNFA });
    }
    f(&mut cfg);
    let c = genesis::build(&cfg.genesis).unwrap().0;
    T { t: cfg.genesis.timestamp_ms, c, op }
}

impl T {
    fn block(&mut self, txs: Vec<VersionedTransaction>) -> Settlement {
        self.t += 100;
        let ex = self.c.produce(txs, self.t, &self.op);
        assert_eq!(ex.block.txs.len(), ex.results.len());
        ex.settlement
    }
    /// Blocco all'altezza `h` (salto di slot vuoti, come col nodo vero).
    fn block_at(&mut self, h: u64, txs: Vec<VersionedTransaction>) -> Settlement {
        self.t += 100;
        self.c.produce_at(h, txs, self.t, &self.op).settlement
    }
    fn bal(&self, k: &Address) -> u64 {
        self.c.svm.lamports(k)
    }
    fn mining(&self) -> u64 {
        self.c.vault_available(&vault_address("mining"))
    }
}

/// Transfer con priority fee: limite 1000 CU a 1 lamport/CU = 1000 lamport.
fn priority_tx(from: &Keypair, to: &Address, lamports: u64, bh: solana_hash::Hash) -> VersionedTransaction {
    let cb = genesis::parse_address("ComputeBudget111111111111111111111111111111").unwrap();
    let mut limit = vec![2];
    limit.extend_from_slice(&1000u32.to_le_bytes());
    let mut price = vec![3];
    price.extend_from_slice(&1_000_000u64.to_le_bytes());
    let ixs = [
        Instruction { program_id: cb, accounts: vec![], data: limit },
        Instruction { program_id: cb, accounts: vec![], data: price },
        solana_system_interface::instruction::transfer(&from.pubkey(), to, lamports),
    ];
    let msg = Message::new_with_blockhash(&ixs, Some(&from.pubkey()), &bh);
    VersionedTransaction::try_new(VersionedMessage::Legacy(msg), &[from]).unwrap()
}

#[test]
fn ninety_ten_reward_and_priority_refund() {
    let (a, b) = (Keypair::new(), Keypair::new());
    let mut t = setup(&[&a, &b], |c| c.genesis.block_budget_lamports = BIG);
    let (to, bh) = (Address::new_unique(), t.c.last_hash);
    let (a0, b0, op0, res0, min0) = (t.bal(&a.pubkey()), t.bal(&b.pubkey()), t.bal(&t.op.pubkey()), t.bal(&vault_address("reserve")), t.mining());
    let c = LAMPORTS_PER_CNFA;
    let txs = vec![priority_tx(&a, &to, c, bh), transfer_tx(&a, &to, 2 * c, bh), transfer_tx(&b, &to, 3 * c, bh)];
    t.t += 100;
    let ex = t.c.produce(txs, t.t, &t.op);
    assert!(ex.results.iter().all(Result::is_ok), "{:?}", ex.results);
    let s = ex.settlement;
    // Base 15.000: operatore 1.500, riserva 13.500, premio 13.500 (A 10.000 -> 9.000, B 5.000 -> 4.500).
    assert_eq!(s, Settlement { mining_active: true, base: 15_000, priority_refund: 1_000, operator: 1_500, reserve: 13_500, reward: 13_500, to_airdrop: 0 });
    assert_eq!(t.bal(&a.pubkey()), a0 - 3 * c - 10_000 + 9_000, "priority fee rimborsata");
    assert_eq!(t.bal(&b.pubkey()), b0 - 3 * c - 5_000 + 4_500);
    assert_eq!(t.bal(&to), 6 * c);
    assert_eq!(t.bal(&t.op.pubkey()), op0 + 1_500);
    assert_eq!(t.bal(&vault_address("reserve")), res0 + 13_500);
    assert_eq!(t.mining(), min0 - 13_500);
    assert_eq!(t.c.total_lamports(), SUPPLY_LAMPORTS as u128, "supply costante, burn 0");
}

#[test]
fn reward_split_rounds_down_and_keeps_dust_in_vault() {
    let (a, b, c) = (Keypair::new(), Keypair::new(), Keypair::new());
    let mut t = setup(&[&a, &b, &c], |c| c.genesis.block_budget_lamports = 1_000);
    let (to, bh) = (Address::new_unique(), t.c.last_hash);
    let m0 = t.mining();
    let s = t.block(vec![transfer_tx(&a, &to, 1, bh), transfer_tx(&b, &to, 1, bh), transfer_tx(&c, &to, 1, bh)]);
    // Budget 1.000 diviso in tre: 333 a testa, 1 lamport resta nel vault.
    assert_eq!(s.reward, 999);
    assert_eq!(t.mining(), m0 - 999);
    assert_eq!(t.c.total_lamports(), SUPPLY_LAMPORTS as u128);
}

#[test]
fn budget_caps_and_halves() {
    let a = Keypair::new();
    let mut t = setup(&[&a], |c| {
        c.genesis.block_budget_lamports = 1_000;
        c.genesis.halving_blocks = 3;
    });
    let to = Address::new_unique();
    let s1 = t.block(vec![transfer_tx(&a, &to, 1, t.c.last_hash)]); // altezza 1
    assert_eq!((s1.reserve, s1.reward), (4_500, 1_000), "il budget limita il premio");
    t.block(vec![]);
    t.block(vec![]);
    let s4 = t.block(vec![transfer_tx(&a, &to, 1, t.c.last_hash)]); // altezza 4: un dimezzamento
    assert_eq!(s4.reward, 500);
    for _ in 5..7 {
        t.block(vec![]);
    }
    let s7 = t.block(vec![transfer_tx(&a, &to, 1, t.c.last_hash)]); // altezza 7: due dimezzamenti
    assert_eq!(s7.reward, 250);
    assert_eq!(t.c.block_budget(3 * 64), 0);
}

#[test]
fn operator_self_transactions_gain_nothing() {
    let mut t = setup(&[], |c| c.genesis.block_budget_lamports = BIG);
    let me = t.op.pubkey();
    let before = t.bal(&me);
    let tx = transfer_tx(&t.op, &me, 1_000, t.c.last_hash);
    t.block(vec![tx]);
    assert_eq!(t.bal(&me), before, "budget pieno: netto esattamente 0");

    let mut t = setup(&[], |c| c.genesis.block_budget_lamports = 1_000);
    let me = t.op.pubkey();
    let before = t.bal(&me);
    let tx = transfer_tx(&t.op, &me, 1_000, t.c.last_hash);
    t.block(vec![tx]);
    assert_eq!(t.bal(&me), before - 5_000 + 500 + 1_000, "budget limitato: netto negativo");
}

#[test]
fn transition_at_end_height() {
    let a = Keypair::new();
    let mut t = setup(&[&a], |c| {
        c.genesis.block_budget_lamports = BIG;
        c.genesis.fee_mining_end_height = 3;
    });
    let to = Address::new_unique();
    assert!(t.block(vec![transfer_tx(&a, &to, 1, t.c.last_hash)]).mining_active); // altezza 1
    t.block(vec![]);
    let airdrop0 = t.bal(&vault_address("airdrop"));
    let left = t.mining();
    let op0 = t.bal(&t.op.pubkey());
    let s = t.block(vec![transfer_tx(&a, &to, 2, t.c.last_hash)]); // altezza 3: fine
    assert_eq!(s, Settlement { mining_active: false, base: 5_000, operator: 5_000, to_airdrop: left, ..Default::default() });
    assert_eq!(t.bal(&t.op.pubkey()), op0 + 5_000, "100% della base all'operatore");
    assert_eq!(t.bal(&vault_address("airdrop")), airdrop0 + left, "residuo di mining ad airdrop");
    assert_eq!(t.mining(), 0);
    assert_eq!(t.block(vec![]).to_airdrop, 0, "il trasferimento avviene una volta sola");
    assert_eq!(t.c.total_lamports(), SUPPLY_LAMPORTS as u128);
}

#[test]
fn transition_when_mining_vault_runs_out() {
    let a = Keypair::new();
    let mut t = setup(&[&a], |c| c.genesis.block_budget_lamports = BIG);
    // Porta il vault "mining" a 7.000 lamport spendibili (spostando il resto in "reserve").
    let (mk, rk) = (vault_address("mining"), vault_address("reserve"));
    let excess = t.mining() - 7_000;
    let mut m: Account = t.c.svm.get(&mk).unwrap().clone().into();
    let mut r: Account = t.c.svm.get(&rk).unwrap().clone().into();
    m.lamports -= excess;
    r.lamports += excess;
    t.c.svm.set(mk, m);
    t.c.svm.set(rk, r);
    let to = Address::new_unique();
    assert_eq!(t.block(vec![transfer_tx(&a, &to, 1, t.c.last_hash)]).reward, 4_500);
    let s = t.block(vec![transfer_tx(&a, &to, 2, t.c.last_hash)]);
    assert_eq!((s.mining_active, s.reward), (true, 2_500), "premio limitato dal saldo");
    assert_eq!(t.mining(), 0);
    let s = t.block(vec![transfer_tx(&a, &to, 3, t.c.last_hash)]);
    assert_eq!((s.mining_active, s.operator, s.reward), (false, 5_000, 0));
    assert_eq!(t.c.total_lamports(), SUPPLY_LAMPORTS as u128);
}

#[test]
fn replay_reproduces_fee_settlement() {
    let (a, b) = (Keypair::new(), Keypair::new());
    let f = |c: &mut Config| {
        c.genesis.block_budget_lamports = 3_000;
        c.genesis.halving_blocks = 2;
        c.genesis.fee_mining_end_height = 4;
    };
    let mut t = setup(&[&a, &b], f);
    let mut cfg: Config = toml::from_str(EXAMPLE).unwrap();
    cfg.genesis.network = "devnet".into();
    cfg.genesis.operator = t.op.pubkey().to_string();
    for p in [&a, &b, &t.op] {
        cfg.genesis.prefund.push(Prefund { pubkey: p.pubkey().to_string(), lamports: 10 * LAMPORTS_PER_CNFA });
    }
    f(&mut cfg);
    let mut replica = genesis::build(&cfg.genesis).unwrap().0;
    let to = Address::new_unique();
    for i in 0..6u64 {
        let bh = t.c.last_hash;
        t.t += 100;
        let ex = t.c.produce(vec![priority_tx(&a, &to, i + 1, bh), transfer_tx(&b, &to, i + 1, bh)], t.t, &t.op);
        let ey = replica.apply(&ex.block).unwrap();
        assert_eq!(ey.settlement, ex.settlement);
    }
    assert_eq!(replica.last_hash, t.c.last_hash);
}

#[test]
fn prefund_rejected_on_mainnet() {
    let mut cfg: Config = toml::from_str(EXAMPLE).unwrap();
    cfg.genesis.operator = Keypair::new().pubkey().to_string();
    assert_eq!(cfg.genesis.network, "mainnet");
    assert!(genesis::build(&cfg.genesis).is_ok());
    cfg.genesis.prefund.push(Prefund { pubkey: Address::new_unique().to_string(), lamports: 1 });
    let err = genesis::build(&cfg.genesis).err().unwrap();
    assert!(err.contains("devnet"), "{err}");
    // Il controllo di determinismo usa una copia devnet e resta possibile su mainnet.
    cfg.genesis.prefund.clear();
    genesis::check_determinism(&cfg.genesis, 3, 100).unwrap();
}

#[test]
fn halving_and_end_crossed_by_height_jumps() {
    let a = Keypair::new();
    let mut t = setup(&[&a], |c| {
        c.genesis.block_budget_lamports = 1_000;
        c.genesis.halving_blocks = 100;
        c.genesis.fee_mining_end_height = 1_000;
    });
    let to = Address::new_unique();
    // Altezza 250 raggiunta con un salto: due dimezzamenti (1.000 -> 250).
    t.block_at(240, vec![]);
    let s = t.block_at(250, vec![transfer_tx(&a, &to, 1, t.c.last_hash)]);
    assert_eq!((s.mining_active, s.reward), (true, 250));
    // Salto oltre la fine (da 990 a 1.200): il primo blocco dopo chiude il periodo,
    // una sola volta. La tx usa il blockhash del 990 (entro 300 slot).
    let (airdrop0, left) = (t.bal(&vault_address("airdrop")), t.mining());
    t.block_at(990, vec![]);
    let s = t.block_at(1_200, vec![transfer_tx(&a, &to, 2, t.c.last_hash)]);
    assert_eq!((s.mining_active, s.operator, s.reward, s.to_airdrop), (false, 5_000, 0, left));
    assert_eq!(t.bal(&vault_address("airdrop")), airdrop0 + left);
    assert_eq!(t.block_at(9_000, vec![]).to_airdrop, 0);
    assert_eq!(t.c.total_lamports(), SUPPLY_LAMPORTS as u128);
}
