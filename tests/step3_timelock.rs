//! Passo 3: programma timelock eseguito dalla chain (il .so del genesis).

use centrifuga::exec::Chain;
use centrifuga::genesis::{self, Config, LAMPORTS_PER_CNFA, Prefund, SUPPLY_LAMPORTS, timelock_program_id, vault_address};
use centrifuga_timelock::claim_message;
use solana_address::Address;
use solana_instruction::{AccountMeta, Instruction};
use solana_keypair::Keypair;
use solana_message::{Message, VersionedMessage};
use solana_signer::Signer;
use solana_transaction::versioned::VersionedTransaction;

const EXAMPLE: &str = include_str!("../centrifuga.example.toml");
const OPERATOR_TOTAL: u64 = 20_000_000 * LAMPORTS_PER_CNFA;

struct T {
    c: Chain,
    op: Keypair,
    user: Keypair,
    t: u64,
}

/// Altezze piccole: sblocco operatore in 100 blocchi, riserva a 10, fee mining
/// fino a 20, claim su airdrop fino a 30.
fn setup() -> T {
    let (op, user) = (Keypair::new(), Keypair::new());
    let mut cfg: Config = toml::from_str(EXAMPLE).unwrap();
    cfg.genesis.network = "devnet".into();
    cfg.genesis.operator = op.pubkey().to_string();
    cfg.genesis.operator_unlock_blocks = 100;
    cfg.genesis.reserve_unlock_height = 10;
    cfg.genesis.fee_mining_end_height = 20;
    cfg.genesis.claims_end_height = 30;
    for k in [&op, &user] {
        cfg.genesis.prefund.push(Prefund { pubkey: k.pubkey().to_string(), lamports: 100 * LAMPORTS_PER_CNFA });
    }
    let c = genesis::build(&cfg.genesis).unwrap().0;
    T { t: cfg.genesis.timestamp_ms, c, op, user }
}

impl T {
    /// Esegue le istruzioni in un blocco; Ok(()) o il codice di errore del programma.
    fn run(&mut self, ixs: &[Instruction], payer: &Keypair, extra: &[&Keypair]) -> Result<(), String> {
        let msg = Message::new_with_blockhash(ixs, Some(&payer.pubkey()), &self.c.last_hash);
        let mut signers = vec![payer];
        signers.extend_from_slice(extra);
        let tx = VersionedTransaction::try_new(VersionedMessage::Legacy(msg), &signers).unwrap();
        self.t += 100;
        let ex = self.c.produce(vec![tx], self.t, &self.op);
        assert_eq!(self.c.total_lamports(), SUPPLY_LAMPORTS as u128, "supply costante");
        match &ex.results[0] {
            Ok(_) => Ok(()),
            Err(f) => Err(format!("{:?}", f.err)),
        }
    }
    fn until(&mut self, height: u64) {
        while self.c.height + 1 < height {
            self.t += 100;
            self.c.produce(vec![], self.t, &self.op);
        }
    }
    /// Blocchi vuoti a salti di 10 (il massimo con heartbeat_slots = 10): il prossimo
    /// blocco sarà all'altezza `height`. Le altezze intermedie non hanno un blocco.
    fn jump(&mut self, height: u64) {
        while self.c.height + 1 < height {
            self.t += 1_000;
            let next = (self.c.height + 10).min(height - 1);
            self.c.produce_at(next, vec![], self.t, &self.op);
        }
    }
    fn bal(&self, k: &Address) -> u64 {
        self.c.svm.lamports(k)
    }
    /// Crea e inizializza un'ondata su `vault` (firma `auth`).
    fn wave(&mut self, vault: &str, amount: u64, bits: usize, auth: &Keypair) -> Result<Address, String> {
        let w = Keypair::new();
        let len = centrifuga_timelock::WAVE_HEADER + bits.div_ceil(8);
        let create = solana_system_interface::instruction::create_account(
            &self.op.pubkey(), &w.pubkey(), self.c.svm.rent_exempt(len), len as u64, &timelock_program_id());
        let mut data = vec![1];
        data.extend_from_slice(&amount.to_le_bytes());
        let init = ix(data, vec![AccountMeta::new(w.pubkey(), false), AccountMeta::new_readonly(vault_address(vault), false), AccountMeta::new_readonly(auth.pubkey(), true)]);
        let op = Keypair::try_from(self.op.to_bytes().as_slice()).unwrap();
        let extra: Vec<&Keypair> = if auth.pubkey() == op.pubkey() { vec![&w] } else { vec![&w, auth] };
        self.run(&[create, init], &op, &extra).map(|_| w.pubkey())
    }
    /// Claim con firma di `signer` sul messaggio (ondata, `signed_for`, importo, indice).
    fn claim(&mut self, vault: &str, wave: Address, to: Address, amount: u64, index: u32, signer: &Keypair, signed_for: Address) -> Result<(), String> {
        let msg = claim_message(&wave.to_bytes(), &signed_for.to_bytes(), amount, index);
        let sig = ed25519_ix(signer, &msg);
        let mut data = vec![2];
        data.extend_from_slice(&index.to_le_bytes());
        let claim = ix(data, vec![
            AccountMeta::new(vault_address(vault), false),
            AccountMeta::new(wave, false),
            AccountMeta::new(to, false),
            AccountMeta::new_readonly(solana_sdk_ids::sysvar::instructions::id(), false),
        ]);
        let user = Keypair::try_from(self.user.to_bytes().as_slice()).unwrap();
        self.run(&[sig, claim], &user, &[])
    }
    fn withdraw(&mut self, vault: &str, to: Address) -> Result<(), String> {
        let user = Keypair::try_from(self.user.to_bytes().as_slice()).unwrap();
        self.run(&[ix(vec![0], vec![AccountMeta::new(vault_address(vault), false), AccountMeta::new(to, false)])], &user, &[])
    }
    fn to_reserve(&mut self, payer: &Keypair) -> Result<(), String> {
        let accs = vec![AccountMeta::new(vault_address("airdrop"), false), AccountMeta::new(vault_address("reserve"), false)];
        self.run(&[ix(vec![3], accs)], payer, &[])
    }
    fn spendable(&self, vault: &str) -> u64 {
        self.c.vault_available(&vault_address(vault))
    }
}

fn ix(data: Vec<u8>, accounts: Vec<AccountMeta>) -> Instruction {
    Instruction { program_id: timelock_program_id(), accounts, data }
}

/// Istruzione della precompile ed25519: chiave, firma e messaggio nella stessa istruzione.
fn ed25519_ix(signer: &Keypair, msg: &[u8]) -> Instruction {
    let (pk, sig, m) = (16u16, 48u16, 112u16);
    let mut d = vec![1, 0];
    for x in [sig, u16::MAX, pk, u16::MAX, m, msg.len() as u16, u16::MAX] {
        d.extend_from_slice(&x.to_le_bytes());
    }
    d.extend_from_slice(signer.pubkey().as_ref());
    d.extend_from_slice(signer.sign_message(msg).as_ref());
    d.extend_from_slice(msg);
    Instruction { program_id: genesis::parse_address("Ed25519SigVerify111111111111111111111111111").unwrap(), accounts: vec![], data: d }
}

fn custom(code: u32) -> String {
    format!("Custom({code})")
}

fn assert_err<T: std::fmt::Debug>(r: Result<T, String>, code: u32) {
    let e = r.expect_err(&format!("doveva fallire con Custom({code})"));
    assert!(e.contains(&custom(code)), "atteso Custom({code}), ottenuto {e}");
}

#[test]
fn withdraw_is_linear_and_only_from_operator_vault() {
    let mut t = setup();
    let op = t.op.pubkey();
    let start = t.bal(&op);
    t.until(25);
    // Fee mining finito al blocco 20: l'operatore incassa anche il 100% della fee base.
    t.withdraw("operator", op).unwrap(); // eseguito al blocco 25
    assert_eq!(t.bal(&op) - start, OPERATOR_TOTAL / 100 * 25 + 5_000, "25% del totale");
    t.withdraw("operator", op).unwrap(); // blocco 26: un altro centesimo
    assert_eq!(t.bal(&op) - start, OPERATOR_TOTAL / 100 * 26 + 10_000);
    // I lamport vanno solo al beneficiario registrato.
    assert_err(t.withdraw("operator", t.user.pubkey()), 1);
    t.until(120);
    t.withdraw("operator", op).unwrap();
    assert_eq!(t.spendable("operator"), 0, "a fine sblocco il vault è vuoto");

    // Nessun withdraw da reserve (prima e dopo lo sblocco), airdrop e mining.
    for (vault, to) in [("reserve", op), ("reserve", t.user.pubkey()), ("airdrop", op), ("mining", op)] {
        assert_err(t.withdraw(vault, to), 2);
    }
    let mut early = setup();
    assert_err(early.withdraw("reserve", early.op.pubkey()), 2);
}

#[test]
fn reserve_claims_after_unlock_with_fixed_amount() {
    let mut t = setup();
    let op = Keypair::try_from(t.op.to_bytes().as_slice()).unwrap();
    let five = 5 * LAMPORTS_PER_CNFA;
    let intruder = Keypair::new();
    assert_err(t.wave("reserve", five, 16, &intruder), 3);
    assert_err(t.wave("mining", five, 16, &op), 2);
    let wave = t.wave("reserve", five, 16, &op).unwrap();
    let x = Address::new_unique();
    assert_err(t.claim("reserve", wave, x, five, 0, &op, x), 4); // prima dello sblocco
    t.until(10);
    t.claim("reserve", wave, x, five, 0, &op, x).unwrap();
    assert_eq!(t.bal(&x), five);
    assert_err(t.claim("reserve", wave, x, five, 0, &op, x), 5); // doppio claim
    let y = Address::new_unique();
    assert_err(t.claim("reserve", wave, y, five, 1, &intruder, y), 6); // firma non dell'operatore
    assert_err(t.claim("reserve", wave, y, five, 1, &op, x), 6); // firma per un altro destinatario
    assert_err(t.claim("reserve", wave, y, five + 1, 1, &op, y), 6); // importo diverso da quello dell'ondata
    assert_err(t.claim("reserve", wave, y, five, 16, &op, y), 8); // fuori dalla bitmap
    t.claim("reserve", wave, y, five, 1, &op, y).unwrap();
    // Il limite è il saldo reale: un'ondata più grande del vault non paga
    // (margine di 1 CnFA: nel frattempo la riserva riceve il 90% delle fee).
    let huge = t.spendable("reserve") + LAMPORTS_PER_CNFA;
    let big = t.wave("reserve", huge, 8, &op).unwrap();
    assert_err(t.claim("reserve", big, x, huge, 0, &op, x), 7);
}

#[test]
fn airdrop_deadline_and_permissionless_to_reserve() {
    let mut t = setup();
    let op = Keypair::try_from(t.op.to_bytes().as_slice()).unwrap();
    let hundred = 100 * LAMPORTS_PER_CNFA;
    let wave = t.wave("airdrop", hundred, 64, &op).unwrap();
    let x = Address::new_unique();
    t.claim("airdrop", wave, x, hundred, 3, &op, x).unwrap();
    assert_eq!(t.bal(&x), hundred);

    // Prima della scadenza (blocco 30) nessuno può chiamare to_reserve.
    let funded = Keypair::try_from(t.user.to_bytes().as_slice()).unwrap();
    assert_err(t.to_reserve(&funded), 4);
    t.until(30);
    // Dalla scadenza: claim su airdrop rifiutato...
    let y = Address::new_unique();
    assert_err(t.claim("airdrop", wave, y, hundred, 4, &op, y), 4);
    // ...e un utente qualsiasi sposta tutto il residuo in reserve (anche il residuo del fee mining).
    let (left, reserve0) = (t.spendable("airdrop"), t.bal(&vault_address("reserve")));
    assert!(left > 20_000_000 * LAMPORTS_PER_CNFA - hundred, "airdrop contiene anche il residuo di mining");
    assert_ne!(funded.pubkey(), t.op.pubkey(), "chiamata da un utente qualsiasi, non dall'operatore");
    t.to_reserve(&funded).unwrap();
    assert_eq!(t.spendable("airdrop"), 0);
    assert!(t.bal(&vault_address("reserve")) >= reserve0 + left);
    t.to_reserve(&funded).unwrap(); // idempotente: sposta 0
}

#[test]
fn unlocks_and_deadline_crossed_by_height_jumps() {
    let mut t = setup();
    let op = Keypair::try_from(t.op.to_bytes().as_slice()).unwrap();
    let opk = op.pubkey();
    // Withdraw lineare a un'altezza raggiunta con un salto: 40/100 = 40%.
    let start = t.bal(&opk);
    t.jump(40);
    t.withdraw("operator", opk).unwrap();
    assert_eq!(t.bal(&opk) - start, OPERATOR_TOTAL / 100 * 40 + 5_000);
    // Airdrop: la scadenza dei claim (30) è stata superata dal salto a 40.
    let x = Address::new_unique();
    let wave = t.wave("airdrop", LAMPORTS_PER_CNFA, 8, &op).unwrap();
    assert_err(t.claim("airdrop", wave, x, LAMPORTS_PER_CNFA, 0, &op, x), 4);
    let funded = Keypair::try_from(t.user.to_bytes().as_slice()).unwrap();
    t.to_reserve(&funded).unwrap();
    assert_eq!(t.spendable("airdrop"), 0);
}

#[test]
fn reserve_unlock_crossed_by_height_jump() {
    let mut t = setup();
    let op = Keypair::try_from(t.op.to_bytes().as_slice()).unwrap();
    let five = 5 * LAMPORTS_PER_CNFA;
    let wave = t.wave("reserve", five, 8, &op).unwrap(); // altezza 1
    let x = Address::new_unique();
    assert_err(t.claim("reserve", wave, x, five, 0, &op, x), 4); // altezza 2 < 10
    t.jump(15); // salti 2 -> 12 -> 14: l'altezza 10 (sblocco) non ha un blocco
    t.claim("reserve", wave, x, five, 0, &op, x).unwrap();
    assert_eq!(t.bal(&x), five);
    // Airdrop: un claim prima della scadenza riesce anche dopo un salto che resta sotto.
    let mut t = setup();
    let op = Keypair::try_from(t.op.to_bytes().as_slice()).unwrap();
    let wave = t.wave("airdrop", five, 8, &op).unwrap();
    t.jump(29);
    t.claim("airdrop", wave, x, five, 0, &op, x).unwrap(); // altezza 29 < 30
}
