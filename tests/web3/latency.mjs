// Latenza da invio a blocco firmato: sendTransaction (con preflight) -> notifica
// signatureSubscribe, che il nodo emette dopo aver firmato il blocco e scritto il journal.
// Uso: node latency.mjs <url-rpc> <keypair.json prefinanziato> [n]
import fs from "node:fs";
import * as web3 from "@solana/web3.js";

const [url, keyPath, nArg] = process.argv.slice(2);
const n = Number(nArg ?? 200);
const payer = web3.Keypair.fromSecretKey(Uint8Array.from(JSON.parse(fs.readFileSync(keyPath))));
const conn = new web3.Connection(url, "confirmed");
const to = web3.Keypair.generate().publicKey;

async function one(i, blockhash) {
  const tx = new web3.Transaction({ feePayer: payer.publicKey, recentBlockhash: blockhash })
    .add(web3.SystemProgram.transfer({ fromPubkey: payer.publicKey, toPubkey: to, lamports: 1_000_000 + i }));
  tx.sign(payer);
  const raw = tx.serialize();
  const t0 = performance.now();
  const sig = await conn.sendRawTransaction(raw);
  await new Promise((res) => conn.onSignature(sig, res, "confirmed"));
  return performance.now() - t0;
}

function stats(name, xs) {
  xs.sort((a, b) => a - b);
  const p = (q) => xs[Math.min(xs.length - 1, Math.floor(q * xs.length))].toFixed(1);
  const avg = (xs.reduce((a, b) => a + b, 0) / xs.length).toFixed(1);
  console.log(`${name}: n=${xs.length} media=${avg} p50=${p(0.5)} p90=${p(0.9)} p99=${p(0.99)} max=${p(1)} ms`);
}

// Riscaldamento (connessione WebSocket, JIT).
await one(0, (await conn.getLatestBlockhash()).blockhash);

const seq = [];
for (let i = 1; i <= n; i++) seq.push(await one(i, (await conn.getLatestBlockhash()).blockhash));
stats("sequenziali (invio a inizio slot)", seq);

const rnd = [];
for (let i = 1; i <= n; i++) {
  const { blockhash } = await conn.getLatestBlockhash();
  await new Promise((r) => setTimeout(r, Math.random() * 200));
  rnd.push(await one(10_000 + i, blockhash));
}
stats("arrivo casuale", rnd);

// Raffica entro i limiti: 100 per IP (2 s a 50 tx/s) e 64 in coda per pagatore.
const m = Math.min(n, 60);
const { blockhash } = await conn.getLatestBlockhash();
const burst = await Promise.all(Array.from({ length: m }, (_, i) => one(n + 1 + i, blockhash)));
stats(`raffica di ${m} in parallelo`, burst);
process.exit(0);
