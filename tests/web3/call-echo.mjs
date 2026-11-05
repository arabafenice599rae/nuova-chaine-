// Uso: node call-echo.mjs <url-rpc> <keypair.json> <program id>
// Chiama il programma di prova echo e stampa la versione (dai return data).
import fs from "node:fs";
import * as web3 from "@solana/web3.js";
const [url, keyPath, programId] = process.argv.slice(2);
const payer = web3.Keypair.fromSecretKey(Uint8Array.from(JSON.parse(fs.readFileSync(keyPath))));
const conn = new web3.Connection(url, "confirmed");
const tx = new web3.Transaction().add(new web3.TransactionInstruction({ programId: new web3.PublicKey(programId), keys: [], data: Buffer.alloc(0) }));
const sig = await web3.sendAndConfirmTransaction(conn, tx, [payer]);
const t = await conn.getTransaction(sig, { commitment: "confirmed", maxSupportedTransactionVersion: 0 });
const ret = t.meta.returnData ? Buffer.from(t.meta.returnData.data[0], "base64").toString() : null;
console.log(ret ?? `no return data; logs: ${JSON.stringify(t.meta.logMessages)}`);
process.exit(ret ? 0 : 1);
