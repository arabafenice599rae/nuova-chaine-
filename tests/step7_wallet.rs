//! `centrifuga wallet` contro un nodo devnet: creazione chiave, saldo, invio CnFA
//! e SPL, cronologia, nodo errato rifiutato, file esistente non sovrascritto,
//! mai "SOL" nell'output.

use std::io::Write;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use base64::Engine;
use centrifuga::genesis::{self, Config, LAMPORTS_PER_CNFA, Prefund};
use centrifuga::node;
use centrifuga::rpc::client::call;
use serde_json::json;
use solana_address::Address;
use solana_instruction::{AccountMeta, Instruction};
use solana_keypair::Keypair;
use solana_message::{Message, VersionedMessage};
use solana_signer::Signer;
use solana_transaction::versioned::VersionedTransaction;

const EXAMPLE: &str = include_str!("../centrifuga.example.toml");
const TOKEN: &str = "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA";
const ATA: &str = "ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL";
const URL: &str = "http://127.0.0.1:19170";

struct Out {
    ok: bool,
    text: String,
}

/// Esegue il binario; tutto l'output (stdout + stderr) va controllato per "SOL".
fn wallet(dir: &Path, args: &[&str], stdin: &str, seen: &mut String) -> Out {
    let mut child = Command::new(env!("CARGO_BIN_EXE_centrifuga"))
        .arg("wallet")
        .args(args)
        .args(["--config", dir.join("wallet.toml").to_str().unwrap()])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(stdin.as_bytes()).unwrap();
    let o = child.wait_with_output().unwrap();
    let text = format!("{}{}", String::from_utf8_lossy(&o.stdout), String::from_utf8_lossy(&o.stderr));
    seen.push_str(&text);
    Out { ok: o.status.success(), text }
}

fn write_key(path: &Path, k: &Keypair) {
    let b: Vec<String> = k.to_bytes().iter().map(|x| x.to_string()).collect();
    std::fs::write(path, format!("[{}]\n", b.join(","))).unwrap();
}

fn conf(dir: &Path, chain_id: &str, hash: &str) {
    let key = dir.join("user.json");
    std::fs::write(dir.join("wallet.toml"), format!("rpc = \"{URL}\"\nkeypair = \"{}\"\nchain_id = \"{chain_id}\"\ngenesis_hash = \"{hash}\"\n", key.display())).unwrap();
}

/// Transazione preparata dal test (mint SPL), inviata e attesa.
fn submit(payer: &Keypair, extra: &[&Keypair], ixs: &[Instruction]) {
    let bh = call(URL, "getLatestBlockhash", json!([])).unwrap()["value"]["blockhash"].as_str().unwrap().parse().unwrap();
    let msg = Message::new_with_blockhash(ixs, Some(&payer.pubkey()), &bh);
    let mut signers = vec![payer];
    signers.extend_from_slice(extra);
    let tx = VersionedTransaction::try_new(VersionedMessage::Legacy(msg), &signers).unwrap();
    let raw = base64::engine::general_purpose::STANDARD.encode(wincode::serialize(&tx).unwrap());
    let sig = call(URL, "sendTransaction", json!([raw, {"encoding": "base64"}])).unwrap();
    let start = Instant::now();
    loop {
        let st = call(URL, "getSignatureStatuses", json!([[sig]])).unwrap()["value"][0].clone();
        if !st.is_null() {
            assert!(st["err"].is_null(), "{st}");
            return;
        }
        assert!(start.elapsed() < Duration::from_secs(10), "non inclusa");
        std::thread::sleep(Duration::from_millis(50));
    }
}

fn ata(owner: &Address, mint: &Address) -> Address {
    let (token, ata) = (TOKEN.parse::<Address>().unwrap(), ATA.parse::<Address>().unwrap());
    Address::find_program_address(&[owner.as_ref(), token.as_ref(), mint.as_ref()], &ata).0
}

/// Mint con 6 decimali; 100 token all'account associato di `user`.
fn make_token(user: &Keypair) -> Address {
    let (mint, token, ata_program) = (Keypair::new(), TOKEN.parse::<Address>().unwrap(), ATA.parse::<Address>().unwrap());
    let rent = call(URL, "getMinimumBalanceForRentExemption", json!([82])).unwrap().as_u64().unwrap();
    let create = solana_system_interface::instruction::create_account(&user.pubkey(), &mint.pubkey(), rent, 82, &token);
    let mut init = vec![20, 6];
    init.extend_from_slice(user.pubkey().as_ref());
    init.push(0);
    let init = Instruction { program_id: token, accounts: vec![AccountMeta::new(mint.pubkey(), false)], data: init };
    let dest = ata(&user.pubkey(), &mint.pubkey());
    let create_ata = Instruction {
        program_id: ata_program,
        accounts: vec![
            AccountMeta::new(user.pubkey(), true),
            AccountMeta::new(dest, false),
            AccountMeta::new_readonly(user.pubkey(), false),
            AccountMeta::new_readonly(mint.pubkey(), false),
            AccountMeta::new_readonly(solana_sdk_ids::system_program::id(), false),
            AccountMeta::new_readonly(token, false),
        ],
        data: vec![1],
    };
    let mut mint_to = vec![7];
    mint_to.extend_from_slice(&100_000_000u64.to_le_bytes());
    let mint_to = Instruction {
        program_id: token,
        accounts: vec![AccountMeta::new(mint.pubkey(), false), AccountMeta::new(dest, false), AccountMeta::new_readonly(user.pubkey(), true)],
        data: mint_to,
    };
    submit(user, &[&mint], &[create, init, create_ata, mint_to]);
    mint.pubkey()
}

#[tokio::test(flavor = "multi_thread")]
async fn wallet_against_devnet_node() {
    let (op, user) = (Keypair::new(), Keypair::new());
    let mut c: Config = toml::from_str(EXAMPLE).unwrap();
    c.genesis.network = "devnet".into();
    c.genesis.chain_id = "centrifuga:devnet".into();
    c.genesis.operator = op.pubkey().to_string();
    c.genesis.timestamp_ms = node::now_ms() - 86_400_000;
    c.genesis.prefund.push(Prefund { pubkey: user.pubkey().to_string(), lamports: 10 * LAMPORTS_PER_CNFA });
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("wallet_cli");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    c.node.data_dir = dir.join("node");
    c.node.rpc_addr = "127.0.0.1:19170".into();
    c.node.stream_addr = "127.0.0.1:19172".into();
    let hash = genesis::build(&c.genesis).unwrap().0.genesis_hash.to_string();
    let running = node::start(c, Keypair::try_from(op.to_bytes().as_slice()).unwrap()).unwrap();
    tokio::spawn(running.serve_network());
    tokio::time::sleep(Duration::from_millis(300)).await;

    tokio::task::spawn_blocking(move || {
        let mut seen = String::new();
        write_key(&dir.join("user.json"), &user);
        conf(&dir, "centrifuga:devnet", &hash);
        let me = user.pubkey().to_string();

        // Creazione: formato solana-keygen, permessi 600, mai sovrascritta.
        let fresh = dir.join("nuova.json");
        let k = fresh.to_str().unwrap();
        let o = wallet(&dir, &["new", "--keypair", k], "", &mut seen);
        assert!(o.ok, "{}", o.text);
        let created = node::read_keypair(&fresh).unwrap();
        assert!(o.text.contains(&created.pubkey().to_string()));
        assert_eq!(std::fs::metadata(&fresh).unwrap().permissions().mode() & 0o777, 0o600);
        let bytes = std::fs::read(&fresh).unwrap();
        let o = wallet(&dir, &["new", "--keypair", k], "", &mut seen);
        assert!(!o.ok && o.text.contains("esiste già"), "{}", o.text);
        assert_eq!(std::fs::read(&fresh).unwrap(), bytes, "file intatto");
        let o = wallet(&dir, &["address", "--keypair", k], "", &mut seen);
        assert_eq!(o.text.trim(), created.pubkey().to_string());
        // La chiave privata non compare mai: né in byte né in base58.
        assert!(!seen.contains(&created.to_base58_string()) && !seen.contains(&String::from_utf8_lossy(&bytes).to_string()));

        // Saldo, chiave da wallet.toml (file solana-keygen esistente).
        let o = wallet(&dir, &["address"], "", &mut seen);
        assert_eq!(o.text.trim(), me);
        let o = wallet(&dir, &["balance"], "", &mut seen);
        assert_eq!(o.text, format!("{me}: 10 CnFA\n"));

        // Invio CnFA: senza conferma non parte; con --yes sì, con il costo netto.
        let to = Address::new_unique().to_string();
        let o = wallet(&dir, &["send", &to, "1.5"], "n\n", &mut seen);
        assert!(!o.ok && o.text.contains("invio annullato"), "{}", o.text);
        assert!(o.text.contains(&format!("destinatario: {to}")) && o.text.contains("importo: 1.5 CnFA") && o.text.contains("fee: "), "{}", o.text);
        assert!(wallet(&dir, &["balance", &to], "", &mut seen).text.contains(": 0 CnFA"));
        let o = wallet(&dir, &["send", &to, "1.5"], "s\n", &mut seen);
        assert!(o.ok, "{}", o.text);
        let o = wallet(&dir, &["send", &to, "1", "--yes"], "", &mut seen);
        assert!(o.ok, "{}", o.text);
        assert!(o.text.contains("inviata nel blocco") && o.text.contains("premio") && o.text.contains("rimborso") && o.text.contains("costo netto"), "{}", o.text);
        assert_eq!(wallet(&dir, &["balance", &to], "", &mut seen).text, format!("{to}: 2.5 CnFA\n"));

        // Invio SPL: la prima volta crea l'account del destinatario, la seconda no.
        let mint = make_token(&user);
        let o = wallet(&dir, &["balance"], "", &mut seen);
        assert!(o.text.contains(&format!("  100 token {mint}")), "{}", o.text);
        let o = wallet(&dir, &["token-send", &mint.to_string(), &to, "2.5", "--yes"], "", &mut seen);
        assert!(o.ok && o.text.contains("crea l'account token") && o.text.contains("importo: 2.5 token"), "{}", o.text);
        let o = wallet(&dir, &["token-send", &mint.to_string(), &to, "0.25", "--yes"], "", &mut seen);
        assert!(o.ok && !o.text.contains("crea l'account"), "{}", o.text);
        assert!(wallet(&dir, &["balance", &to], "", &mut seen).text.contains(&format!("  2.75 token {mint}")));
        assert!(wallet(&dir, &["balance"], "", &mut seen).text.contains(&format!("  97.25 token {mint}")));
        let o = wallet(&dir, &["token-send", &mint.to_string(), &to, "0.0000001", "--yes"], "", &mut seen);
        assert!(!o.ok && o.text.contains("importo non valido"), "più decimali del mint: {}", o.text);

        // Cronologia: 2 invii CnFA, mint, 2 invii SPL.
        let o = wallet(&dir, &["history"], "", &mut seen);
        assert_eq!(o.text.lines().filter(|l| l.starts_with("blocco ") && l.ends_with("ok")).count(), 5, "{}", o.text);
        assert_eq!(wallet(&dir, &["history", &Address::new_unique().to_string()], "", &mut seen).text, "nessuna transazione\n");

        // Nodo errato: chain_id o genesis hash diversi, o identità non fissata.
        for (id, h) in [("centrifuga:mainnet", hash.as_str()), ("centrifuga:devnet", "11111111111111111111111111111111")] {
            conf(&dir, id, h);
            for args in [&["balance"][..], &["send", &to, "1", "--yes"][..], &["history"][..]] {
                let o = wallet(&dir, args, "", &mut seen);
                assert!(!o.ok && o.text.contains("nodo rifiutato"), "{args:?}: {}", o.text);
            }
        }
        std::fs::write(dir.join("wallet.toml"), format!("rpc = \"{URL}\"\n")).unwrap();
        let o = wallet(&dir, &["balance", &me], "", &mut seen);
        assert!(!o.ok && o.text.contains("chain_id e genesis_hash"), "{}", o.text);
        conf(&dir, "centrifuga:devnet", &hash);
        assert!(wallet(&dir, &["balance", &to], "", &mut seen).text.contains("2.5 CnFA"), "nulla inviato dal nodo rifiutato");

        assert!(!seen.contains("SOL"), "\"SOL\" nell'output:\n{seen}");
    })
    .await
    .unwrap();
}
