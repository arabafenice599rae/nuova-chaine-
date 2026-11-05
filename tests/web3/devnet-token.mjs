// Crea un token SPL di prova e ne invia 1.000 al wallet indicato (per il test con Solflare).
// Uso: node devnet-token.mjs <url-rpc> <keypair-prefinanziato.json> <pubkey-wallet>
import fs from "node:fs";
import * as web3 from "@solana/web3.js";
import * as spl from "@solana/spl-token";

const [url, keyPath, wallet] = process.argv.slice(2);
const payer = web3.Keypair.fromSecretKey(Uint8Array.from(JSON.parse(fs.readFileSync(keyPath))));
const conn = new web3.Connection(url, "confirmed");
const owner = new web3.PublicKey(wallet);
const mint = await spl.createMint(conn, payer, payer.publicKey, null, 6);
const ata = await spl.getOrCreateAssociatedTokenAccount(conn, payer, mint, owner);
await spl.mintTo(conn, payer, mint, ata.address, payer, 1_000_000_000n);
console.log(`mint ${mint.toBase58()}\naccount ${ata.address.toBase58()} di ${wallet}: 1000 token (6 decimali)`);
process.exit(0);
