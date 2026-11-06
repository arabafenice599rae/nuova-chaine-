//! Deploy di programmi (loader upgradeable, come `solana program deploy`): buffer,
//! scrittura a pezzi, deploy, chiamata, aggiornamento, chiamata; poi riavvio da
//! snapshot + journal e dal solo journal, e riesecuzione: stesso stato, il
//! programma risponde con la versione aggiornata.

use std::path::PathBuf;

use centrifuga::exec::{Executed, TransactionResult};
use centrifuga::genesis::{self, Config, LAMPORTS_PER_CNFA, Prefund};
use centrifuga::node::Node;
use solana_address::Address;
use solana_instruction::Instruction;
use solana_keypair::Keypair;
use solana_loader_v3_interface::instruction as loader;
use solana_loader_v3_interface::state::UpgradeableLoaderState;
use solana_message::{Message, VersionedMessage};
use solana_signer::Signer;
use solana_transaction::versioned::VersionedTransaction;

const EXAMPLE: &str = include_str!("../centrifuga.example.toml");
const V1: &[u8] = include_bytes!("programs/echo_v1.so");
const V2: &[u8] = include_bytes!("programs/echo_v2.so");
/// Come la CLI: pezzi che stanno in una tx da 1232 byte.
const CHUNK: usize = 900;

struct T {
    n: Node,
    t: u64,
    blocks: Vec<centrifuga::ledger::Block>,
}

impl T {
    fn block(&mut self, txs: Vec<(Vec<Instruction>, Vec<&Keypair>)>, payer: &Keypair) -> Executed {
        let bh = self.n.chain.last_hash;
        let txs: Vec<VersionedTransaction> = txs
            .into_iter()
            .map(|(ixs, extra)| {
                let msg = Message::new_with_blockhash(&ixs, Some(&payer.pubkey()), &bh);
                let mut signers = vec![payer];
                signers.extend(extra);
                VersionedTransaction::try_new(VersionedMessage::Legacy(msg), &signers).unwrap()
            })
            .collect();
        let sent = txs.len();
        assert!(self.n.ingress.submit(None, txs).iter().all(|r| r.is_ok()));
        self.t += 100;
        let ex = self.n.tick(self.t).unwrap();
        assert_eq!(ex.block.txs.len(), sent, "tx incluse");
        for r in &ex.results {
            assert!(r.is_ok(), "tx fallita: {:?}", r.as_ref().err().map(|f| (&f.err, &f.meta.logs)));
        }
        assert_eq!(self.n.chain.state_hash(), self.n.chain.full_state_hash());
        self.blocks.push(ex.block.clone());
        ex
    }

    /// Buffer con l'ELF, scritto a pezzi (un blocco per la creazione, uno per le scritture).
    fn buffer(&mut self, payer: &Keypair, elf: &[u8]) -> Keypair {
        let buf = Keypair::new();
        let lamports = self.n.chain.svm.rent_exempt(UpgradeableLoaderState::size_of_buffer(elf.len()));
        let create = loader::create_buffer(&payer.pubkey(), &buf.pubkey(), &payer.pubkey(), lamports, elf.len()).unwrap();
        self.block(vec![(create, vec![&buf])], payer);
        let writes = elf.chunks(CHUNK).enumerate().map(|(i, c)| (vec![loader::write(&buf.pubkey(), &payer.pubkey(), (i * CHUNK) as u32, c.to_vec())], vec![])).collect();
        self.block(writes, payer);
        buf
    }

    fn call(&mut self, payer: &Keypair, program: &Address) -> String {
        let ix = Instruction { program_id: *program, accounts: vec![], data: vec![] };
        let ex = self.block(vec![(vec![ix], vec![])], payer);
        version(&ex.results[0])
    }
}

fn version(r: &TransactionResult) -> String {
    let meta = r.as_ref().unwrap();
    let ret = String::from_utf8(meta.return_data.data.clone()).unwrap();
    assert!(meta.logs.iter().any(|l| l == &format!("Program log: {ret}")), "{:?}", meta.logs);
    ret
}

#[test]
fn deploy_call_upgrade_and_replay() {
    let (op, user) = (Keypair::new(), Keypair::new());
    let mut cfg: Config = toml::from_str(EXAMPLE).unwrap();
    cfg.genesis.operator = op.pubkey().to_string();
    cfg.genesis.network = "devnet".into();
    cfg.genesis.chain_id = "centrifuga:devnet".into();
    cfg.genesis.prefund.push(Prefund { pubkey: user.pubkey().to_string(), lamports: 100 * LAMPORTS_PER_CNFA });
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("deploy");
    let _ = std::fs::remove_dir_all(&dir);
    cfg.node.data_dir = dir.clone();
    cfg.node.snapshot_every = 5;
    let kp = || Keypair::try_from(op.to_bytes().as_slice()).unwrap();
    let t0 = cfg.genesis.timestamp_ms;
    let mut t = T { n: Node::open(cfg.clone(), kp()).unwrap(), t: t0, blocks: vec![] };

    // Deploy (autorità di upgrade = pagatore, spazio per crescere).
    let program = Keypair::new();
    let buf = t.buffer(&user, V1);
    let lamports = t.n.chain.svm.rent_exempt(UpgradeableLoaderState::size_of_program());
    let deploy = loader::deploy_with_max_program_len(&user.pubkey(), &program.pubkey(), &buf.pubkey(), &user.pubkey(), lamports, 2 * V1.len(), true).unwrap();
    t.block(vec![(deploy, vec![&program])], &user);
    assert!(t.n.chain.svm.get(&buf.pubkey()).is_none(), "buffer consumato dal deploy");
    assert_eq!(t.call(&user, &program.pubkey()), "echo v1");

    // Aggiornamento con un nuovo buffer; la caparra del buffer torna al pagatore.
    let buf2 = t.buffer(&user, V2);
    t.block(vec![(vec![loader::upgrade(&program.pubkey(), &buf2.pubkey(), &user.pubkey(), &user.pubkey(), true)], vec![])], &user);
    assert_eq!(t.call(&user, &program.pubkey()), "echo v2");
    let (h, state, height) = (t.n.chain.last_hash, t.n.chain.state_hash(), t.n.chain.height);
    let blocks = std::mem::take(&mut t.blocks);
    let t_end = t.t;
    drop(t);

    // Riavvio da snapshot + journal, poi dal solo journal: stesso stato, programma v2.
    let n = Node::open(cfg.clone(), kp()).unwrap();
    assert!(centrifuga::ledger::read_snapshot(&dir).unwrap().unwrap().height > 0, "c'è uno snapshot");
    assert_eq!((n.chain.height, n.chain.last_hash, n.chain.state_hash()), (height, h, state));
    let mut t = T { n, t: t_end, blocks: vec![] };
    assert_eq!(t.call(&user, &program.pubkey()), "echo v2", "dopo il riavvio da snapshot");
    let (h2, state2) = (t.n.chain.last_hash, t.n.chain.state_hash());
    drop(t);
    std::fs::remove_file(dir.join("snapshot.bin")).unwrap();
    let n = Node::open(cfg.clone(), kp()).unwrap();
    assert_eq!((n.chain.last_hash, n.chain.state_hash()), (h2, state2), "riesecuzione dal journal");
    let mut t = T { n, t: t_end + 100, blocks: vec![] };
    assert_eq!(t.call(&user, &program.pubkey()), "echo v2", "dopo la riesecuzione dal journal");

    // Riesecuzione indipendente dei blocchi firmati.
    let mut c = genesis::build(&cfg.genesis).unwrap().0;
    for b in &blocks {
        c.apply(b).unwrap();
    }
    assert_eq!((c.last_hash, c.state_hash()), (h, state));
}
