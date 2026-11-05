//! HTTPS del wallet: certificato e nome host sempre verificati, `http://` solo locale.
//! La CA di prova è generata qui con `openssl` e caricata solo dai test
//! (`wallet::run_with` / `client::call_with`); il binario usa solo le CA pubbliche.

use std::io::{ErrorKind, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;
use std::time::Duration;

use centrifuga::genesis::{self, Config, LAMPORTS_PER_CNFA, Prefund};
use centrifuga::node;
use centrifuga::rpc::client::{Endpoint, Tls, call_with, endpoint, tls_with};
use rustls::pki_types::pem::PemObject;
use rustls::pki_types::{CertificateDer, PrivateKeyDer};
use serde_json::json;
use solana_address::Address;
use solana_keypair::Keypair;
use solana_signer::Signer;

const EXAMPLE: &str = include_str!("../centrifuga.example.toml");
const NODE: &str = "127.0.0.1:19180";

fn openssl(dir: &Path, args: &[&str]) {
    let o = Command::new("openssl").args(args).current_dir(dir).output().expect("openssl");
    assert!(o.status.success(), "openssl {args:?}: {}", String::from_utf8_lossy(&o.stderr));
}

/// CA di prova e certificati foglia: valido, scaduto, nome errato, CA sconosciuta.
fn make_certs(dir: &Path) {
    let ec = ["-newkey", "ec", "-pkeyopt", "ec_paramgen_curve:P-256", "-nodes"];
    openssl(dir, &[&["req", "-x509"][..], &ec, &["-keyout", "ca.key", "-out", "ca.crt", "-days", "3650", "-subj", "/CN=centrifuga-test-ca"]].concat());
    openssl(dir, &[&["req"][..], &ec, &["-keyout", "leaf.key", "-out", "leaf.csr", "-subj", "/CN=localhost"]].concat());
    std::fs::write(dir.join("ok.ext"), "subjectAltName=DNS:localhost\nbasicConstraints=CA:FALSE\n").unwrap();
    std::fs::write(dir.join("other.ext"), "subjectAltName=DNS:other.example\nbasicConstraints=CA:FALSE\n").unwrap();
    let sign = |out: &str, ext: &str, serial: &str| {
        openssl(dir, &["x509", "-req", "-in", "leaf.csr", "-CA", "ca.crt", "-CAkey", "ca.key", "-set_serial", serial, "-days", "3650", "-extfile", ext, "-out", out]);
    };
    sign("good.crt", "ok.ext", "2");
    sign("other.crt", "other.ext", "3");
    std::fs::write(dir.join("ca.cnf"), "[ca]\ndefault_ca=d\n[d]\ndatabase=index.txt\nnew_certs_dir=.\nserial=serial\npolicy=p\ndefault_md=sha256\nunique_subject=no\n[p]\ncommonName=supplied\n").unwrap();
    std::fs::write(dir.join("index.txt"), "").unwrap();
    std::fs::write(dir.join("serial"), "10\n").unwrap();
    openssl(dir, &["ca", "-batch", "-config", "ca.cnf", "-cert", "ca.crt", "-keyfile", "ca.key", "-in", "leaf.csr", "-out", "expired.crt", "-startdate", "20200101000000Z", "-enddate", "20210101000000Z", "-extfile", "ok.ext", "-notext"]);
    // Foglia corretta, ma firmata da una CA che il client non conosce.
    openssl(dir, &[&["req", "-x509"][..], &ec, &["-keyout", "ca2.key", "-out", "ca2.crt", "-days", "3650", "-subj", "/CN=unknown-ca"]].concat());
    openssl(dir, &["x509", "-req", "-in", "leaf.csr", "-CA", "ca2.crt", "-CAkey", "ca2.key", "-set_serial", "4", "-days", "3650", "-extfile", "ok.ext", "-out", "foreign.crt"]);
}

fn test_tls(dir: &Path) -> Tls {
    let mut roots = rustls::RootCertStore::empty();
    roots.add(CertificateDer::from_pem_file(dir.join("ca.crt")).unwrap()).unwrap();
    tls_with(roots)
}

/// Proxy TLS di prova davanti al nodo: una richiesta per connessione, come il client.
fn tls_proxy(dir: &Path, cert: &str, port: u16) {
    let chain = vec![CertificateDer::from_pem_file(dir.join(cert)).unwrap()];
    let key = PrivateKeyDer::from_pem_file(dir.join("leaf.key")).unwrap();
    let provider = Arc::new(rustls::crypto::ring::default_provider());
    let cfg = rustls::ServerConfig::builder_with_provider(provider).with_safe_default_protocol_versions().unwrap();
    let cfg = Arc::new(cfg.with_no_client_auth().with_single_cert(chain, key).unwrap());
    let listener = TcpListener::bind(("127.0.0.1", port)).unwrap();
    std::thread::spawn(move || {
        for tcp in listener.incoming().flatten() {
            let cfg = cfg.clone();
            std::thread::spawn(move || {
                let mut tls = rustls::StreamOwned::new(rustls::ServerConnection::new(cfg).unwrap(), tcp);
                let Some(req) = read_request(&mut tls) else { return }; // handshake rifiutato dal client
                let mut up = TcpStream::connect(NODE).unwrap();
                up.write_all(&req).unwrap();
                let mut resp = vec![];
                up.read_to_end(&mut resp).unwrap();
                tls.write_all(&resp).unwrap();
                tls.conn.send_close_notify();
                let _ = tls.flush();
            });
        }
    });
}

fn read_request(s: &mut impl Read) -> Option<Vec<u8>> {
    let mut buf = vec![];
    let mut chunk = [0u8; 4096];
    loop {
        let n = s.read(&mut chunk).ok()?;
        if n == 0 {
            return None;
        }
        buf.extend_from_slice(&chunk[..n]);
        let text = String::from_utf8_lossy(&buf);
        if let Some(end) = text.find("\r\n\r\n") {
            let len = text[..end].lines().find_map(|l| l.to_lowercase().strip_prefix("content-length:").map(|v| v.trim().parse::<usize>().unwrap()));
            if buf.len() >= end + 4 + len.unwrap_or(0) {
                return Some(buf);
            }
        }
    }
}

fn args(list: &[&str]) -> Vec<String> {
    list.iter().map(|s| s.to_string()).collect()
}

#[test]
fn url_policy() {
    let ep = |tls, addr: &str, host: &str, path: &str| Endpoint { tls, addr: addr.into(), host: host.into(), path: path.into() };
    assert_eq!(endpoint("rpc.example.org").unwrap(), ep(true, "rpc.example.org:443", "rpc.example.org", "/"), "https di default");
    assert_eq!(endpoint("https://rpc.example.org:8443/x").unwrap(), ep(true, "rpc.example.org:8443", "rpc.example.org", "/x"));
    assert_eq!(endpoint("localhost:8899").unwrap(), ep(false, "localhost:8899", "localhost", "/"));
    assert_eq!(endpoint("http://127.0.0.1:8899").unwrap(), ep(false, "127.0.0.1:8899", "127.0.0.1", "/"));
    for bad in ["http://rpc.example.org", "http://127.0.0.2:8899", "http://10.0.0.1:8899", "http://[::1]:8899", "ftp://localhost", "https://"] {
        assert!(endpoint(bad).is_err(), "{bad}");
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn wallet_over_https() {
    let (op, user) = (Keypair::new(), Keypair::new());
    let mut c: Config = toml::from_str(EXAMPLE).unwrap();
    c.genesis.network = "devnet".into();
    c.genesis.chain_id = "centrifuga:devnet".into();
    c.genesis.operator = op.pubkey().to_string();
    c.genesis.timestamp_ms = node::now_ms() - 86_400_000;
    c.genesis.prefund.push(Prefund { pubkey: user.pubkey().to_string(), lamports: 10 * LAMPORTS_PER_CNFA });
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("wallet_tls");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    c.node.data_dir = dir.join("node");
    c.node.rpc_addr = NODE.into();
    c.node.stream_addr = "127.0.0.1:19182".into();
    let hash = genesis::build(&c.genesis).unwrap().0.genesis_hash.to_string();
    let running = node::start(c, Keypair::try_from(op.to_bytes().as_slice()).unwrap()).unwrap();
    tokio::spawn(running.serve_network());
    tokio::time::sleep(Duration::from_millis(300)).await;

    tokio::task::spawn_blocking(move || {
        make_certs(&dir);
        for (cert, port) in [("good.crt", 19190), ("expired.crt", 19191), ("other.crt", 19192), ("foreign.crt", 19193)] {
            tls_proxy(&dir, cert, port);
        }
        let b: Vec<String> = user.to_bytes().iter().map(|x| x.to_string()).collect();
        std::fs::write(dir.join("user.json"), format!("[{}]", b.join(","))).unwrap();
        let toml = dir.join("wallet.toml");
        let write_conf = |rpc: &str, chain_id: &str, gh: &str| {
            let key = dir.join("user.json");
            std::fs::write(&toml, format!("rpc = \"{rpc}\"\nkeypair = \"{}\"\nchain_id = \"{chain_id}\"\ngenesis_hash = \"{gh}\"\n", key.display())).unwrap();
        };
        let cfg = toml.to_str().unwrap().to_string();
        let tls = test_tls(&dir);
        let wallet = |a: &[&str]| centrifuga::wallet::run_with(&args(&[a, &["--config", &cfg]].concat()), tls.clone());

        // Certificato valido della CA di prova: saldo e invio riusciti, via TLS.
        write_conf("https://localhost:19190", "centrifuga:devnet", &hash);
        wallet(&["balance"]).unwrap();
        let to = Address::new_unique().to_string();
        wallet(&["send", &to, "1.5", "--yes"]).unwrap();
        let got = call_with(&tls, "https://localhost:19190", "getBalance", json!([to])).unwrap();
        assert_eq!(got["value"], 1_500_000_000u64);

        // HTTPS non esonera dal controllo dell'identità.
        write_conf("https://localhost:19190", "centrifuga:devnet", "11111111111111111111111111111111");
        assert!(wallet(&["balance"]).unwrap_err().contains("node rejected"));
        write_conf("https://localhost:19190", "centrifuga:mainnet", &hash);
        assert!(wallet(&["send", &to, "1", "--yes"]).unwrap_err().contains("node rejected"));

        // Scaduto, nome host errato, CA sconosciuta: rifiutati prima di qualsiasi richiesta.
        for (port, why) in [(19191, "certificate expired"), (19192, "not valid for name"), (19193, "UnknownIssuer")] {
            write_conf(&format!("https://localhost:{port}"), "centrifuga:devnet", &hash);
            let e = wallet(&["balance"]).unwrap_err();
            assert!(e.contains("invalid peer certificate") && e.contains(why), "{port}: {e}");
        }

        // Il binario usa solo le CA pubbliche: la CA di prova non vale.
        write_conf("https://localhost:19190", "centrifuga:devnet", &hash);
        let o = Command::new(env!("CARGO_BIN_EXE_centrifuga")).args(["wallet", "balance", "--config", &cfg]).output().unwrap();
        let err = String::from_utf8_lossy(&o.stderr);
        assert!(!o.status.success() && err.contains("UnknownIssuer"), "{err}");

        // http:// verso un host non locale: rifiutato senza aprire connessioni.
        let trap = TcpListener::bind("127.0.0.2:19195").unwrap();
        trap.set_nonblocking(true).unwrap();
        for rpc in ["http://127.0.0.2:19195", "http://rpc.example.org"] {
            write_conf(rpc, "centrifuga:devnet", &hash);
            let o = Command::new(env!("CARGO_BIN_EXE_centrifuga")).args(["wallet", "balance", "--config", &cfg]).output().unwrap();
            let err = String::from_utf8_lossy(&o.stderr);
            assert!(!o.status.success() && err.contains("plain http:// is allowed only to localhost"), "{rpc}: {err}");
            let o = Command::new(env!("CARGO_BIN_EXE_centrifuga")).args(["wallet", "send", &to, "1", "--yes", "--config", &cfg, "--rpc", rpc]).output().unwrap();
            assert!(!o.status.success());
        }
        assert_eq!(trap.accept().unwrap_err().kind(), ErrorKind::WouldBlock, "nessuna connessione al nodo http remoto");
    })
    .await
    .unwrap();
}
