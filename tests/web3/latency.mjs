// Latenza da invio a blocco firmato, separando rete e nodo.
//
//   node latency.mjs <url-rpc> <keypair.json> [n=200]
//
// Se <keypair.json> non esiste lo crea (permessi 600) e stampa solo la pubkey da
// prefinanziare nel genesis devnet. Per ogni tx (arrivo casuale, 0-200 ms):
// - totale: sendTransaction (con preflight) -> notifica signatureSubscribe;
// - nodo: dall'arrivo della richiesta HTTP al nodo al blocco firmato e scritto
//   (campo centrifugaNodeLatencyUs di getTransaction);
// - rete: totale - nodo (andata della richiesta + ritorno della notifica, TLS e
//   proxy inclusi). Per confronto: RTT di getHealth sulla stessa connessione.
import fs from "node:fs";
import * as web3 from "@solana/web3.js";

const [url, keyPath, nArg] = process.argv.slice(2);
if (!url || !keyPath) {
  console.error("uso: node latency.mjs <url-rpc> <keypair.json> [n]");
  process.exit(1);
}
if (!fs.existsSync(keyPath)) {
  const kp = web3.Keypair.generate();
  fs.writeFileSync(keyPath, JSON.stringify(Array.from(kp.secretKey)), { mode: 0o600 });
  console.log(`creata ${keyPath}: prefinanziare ${kp.publicKey.toBase58()} nel genesis devnet, poi rilanciare`);
  process.exit(2);
}
const n = Number(nArg ?? 200);
const payer = web3.Keypair.fromSecretKey(Uint8Array.from(JSON.parse(fs.readFileSync(keyPath))));
const conn = new web3.Connection(url, "confirmed");
const to = web3.Keypair.generate().publicKey;

const rpc = async (method, params = []) => {
  const r = await fetch(url, { method: "POST", headers: { "content-type": "application/json" }, body: JSON.stringify({ jsonrpc: "2.0", id: 1, method, params }) });
  return (await r.json()).result;
};

function stats(name, xs) {
  xs = [...xs].sort((a, b) => a - b);
  const p = (q) => xs[Math.min(xs.length - 1, Math.ceil(q * xs.length) - 1)].toFixed(1);
  const avg = (xs.reduce((a, b) => a + b, 0) / xs.length).toFixed(1);
  console.log(`${name.padEnd(34)} n=${String(xs.length).padEnd(4)} media=${avg.padStart(6)}  p50=${p(0.5).padStart(6)}  p95=${p(0.95).padStart(6)}  p99=${p(0.99).padStart(6)} ms`);
}

async function send(i, blockhash) {
  const tx = new web3.Transaction({ feePayer: payer.publicKey, recentBlockhash: blockhash })
    .add(web3.SystemProgram.transfer({ fromPubkey: payer.publicKey, toPubkey: to, lamports: 1_000_000 + i }));
  tx.sign(payer);
  const raw = tx.serialize();
  const t0 = performance.now();
  const sig = await conn.sendRawTransaction(raw);
  await new Promise((res) => conn.onSignature(sig, res, "confirmed"));
  return { sig, total: performance.now() - t0 };
}

const balance = await conn.getBalance(payer.publicKey);
if (balance < n * 10_000) {
  console.error(`saldo insufficiente (${balance} lamport) per ${payer.publicKey.toBase58()}`);
  process.exit(1);
}
await send(0, (await conn.getLatestBlockhash()).blockhash); // riscaldamento (TLS, WebSocket)

const rtt = [];
for (let i = 0; i < 50; i++) {
  const t0 = performance.now();
  await rpc("getHealth");
  rtt.push(performance.now() - t0);
}

const runs = [];
for (let i = 1; i <= n; i++) {
  const { blockhash } = await conn.getLatestBlockhash();
  await new Promise((r) => setTimeout(r, Math.random() * 200));
  runs.push(await send(i, blockhash));
}
const total = [], node = [], net = [];
for (const r of runs) {
  const tx = await rpc("getTransaction", [r.sig, { encoding: "json", maxSupportedTransactionVersion: 0 }]);
  const us = tx?.centrifugaNodeLatencyUs;
  if (us === undefined) continue; // es. nodo riavviato nel mezzo
  total.push(r.total);
  node.push(us / 1000);
  net.push(r.total - us / 1000);
}
console.log(`${url}, ${new Date().toISOString()}`);
stats("RTT HTTP (getHealth)", rtt);
stats("totale: invio -> blocco notificato", total);
stats("nodo: arrivo -> blocco firmato", node);
stats("rete: totale - nodo", net);
process.exit(0);
