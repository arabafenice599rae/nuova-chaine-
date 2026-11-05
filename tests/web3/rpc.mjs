// Uso: node rpc.mjs <url-rpc> <keypair.json prefinanziato>
// Esercita ogni metodo del gateway con @solana/web3.js e @solana/spl-token.
import fs from "node:fs";
import assert from "node:assert/strict";
import * as web3 from "@solana/web3.js";
import * as spl from "@solana/spl-token";

const [url, keyPath] = process.argv.slice(2);
const payer = web3.Keypair.fromSecretKey(Uint8Array.from(JSON.parse(fs.readFileSync(keyPath))));
const conn = new web3.Connection(url, "confirmed");
const ok = (name, v) => console.log(`ok  ${name}`, v === undefined ? "" : JSON.stringify(v));

const version = await conn.getVersion();
assert.equal(version.token.symbol, "CnFA");
ok("getVersion", version);
ok("getHealth", await (await fetch(url, { method: "POST", headers: { "content-type": "application/json" },
  body: JSON.stringify({ jsonrpc: "2.0", id: 1, method: "getHealth" }) })).json());
ok("getGenesisHash", await conn.getGenesisHash());
const slot = await conn.getSlot();
assert.ok(slot > 0);
ok("getSlot", slot);
ok("getBlockHeight", await conn.getBlockHeight());
ok("getEpochInfo", await conn.getEpochInfo());
const { blockhash, lastValidBlockHeight } = await conn.getLatestBlockhash();
ok("getLatestBlockhash", { blockhash, lastValidBlockHeight });
assert.equal(await conn.isBlockhashValid(blockhash).then((r) => r.value), true);
ok("isBlockhashValid");
const rent = await conn.getMinimumBalanceForRentExemption(0);
ok("getMinimumBalanceForRentExemption(0)", rent);
ok("getRecentPrioritizationFees (#)", (await conn.getRecentPrioritizationFees()).length);
const bal0 = await conn.getBalance(payer.publicKey);
assert.ok(bal0 > 3 * web3.LAMPORTS_PER_SOL);
ok("getBalance", bal0);

// Transfer con priority fee (non cambia l'ordine), via WebSocket per la conferma.
const to = web3.Keypair.generate().publicKey;
// accountSubscribe su un WebSocket diretto: si invia la tx solo dopo la conferma
// dell'iscrizione (le notifiche riguardano solo le modifiche successive).
const wsUrl = url.replace(/^http/, "ws").replace(/:(\d+)$/, (_, p) => `:${Number(p) + 1}`);
const sock = new WebSocket(wsUrl);
await new Promise((res, rej) => { sock.onopen = res; sock.onerror = () => rej(new Error(`WebSocket ${wsUrl}`)); });
const accountNotified = new Promise((res, rej) => {
  const timer = setTimeout(() => rej(new Error("accountNotification non arrivata in 15 s")), 15000);
  sock.onmessage = (ev) => {
    const m = JSON.parse(ev.data);
    if (m.method === "accountNotification") { clearTimeout(timer); sock.close(); res(m.params.result.value.lamports); }
  };
});
await new Promise((res, rej) => {
  const prev = sock.onmessage;
  sock.onmessage = (ev) => { const m = JSON.parse(ev.data); if (m.id === 1) { sock.onmessage = prev; m.result ? res() : rej(new Error(JSON.stringify(m))); } };
  sock.send(JSON.stringify({ jsonrpc: "2.0", id: 1, method: "accountSubscribe", params: [to.toBase58(), { encoding: "base64" }] }));
});
const tx = new web3.Transaction().add(
  web3.ComputeBudgetProgram.setComputeUnitPrice({ microLamports: 1000 }),
  web3.SystemProgram.transfer({ fromPubkey: payer.publicKey, toPubkey: to, lamports: 2 * web3.LAMPORTS_PER_SOL }),
);
tx.recentBlockhash = blockhash;
tx.feePayer = payer.publicKey;
const fee = await conn.getFeeForMessage(tx.compileMessage());
assert.equal(fee.value, 5000);
ok("getFeeForMessage", fee.value);
const sim = await conn.simulateTransaction(tx, [payer]);
assert.equal(sim.value.err, null);
ok("simulateTransaction", sim.value.unitsConsumed);
const sig = await web3.sendAndConfirmTransaction(conn, tx, [payer]); // sendTransaction + signatureSubscribe
ok("sendTransaction+signatureSubscribe", sig);
assert.equal(await accountNotified, 2 * web3.LAMPORTS_PER_SOL);
ok("accountSubscribe");
const st = await conn.getSignatureStatuses([sig]);
assert.equal(st.value[0].confirmationStatus, "finalized");
ok("getSignatureStatuses");
const got = await conn.getTransaction(sig, { maxSupportedTransactionVersion: 0 });
assert.equal(got.meta.err, null);
assert.equal(got.meta.postBalances[1] - got.meta.preBalances[1], 2 * web3.LAMPORTS_PER_SOL);
ok("getTransaction", { fee: got.meta.fee, slot: got.slot });
const parsed = await conn.getParsedTransaction(sig, { maxSupportedTransactionVersion: 0 });
assert.equal(parsed.transaction.message.accountKeys[0].signer, true);
assert.equal(parsed.transaction.message.instructions[1].programId.toBase58(), web3.SystemProgram.programId.toBase58());
ok("getTransaction(jsonParsed)", parsed.transaction.message.accountKeys.length);
// Storico: con un indexer (HISTORY=required) la cronologia c'è; senza, il nodo risponde
// con l'errore -32011 "history not available".
if (process.env.HISTORY === "required") {
  let hist = [];
  for (let i = 0; i < 50 && hist[0]?.signature !== sig; i++) {
    hist = await conn.getSignaturesForAddress(payer.publicKey); // l'indexer segue il journal
    if (hist[0]?.signature !== sig) await new Promise((r) => setTimeout(r, 200));
  }
  assert.equal(hist[0].signature, sig);
  ok("getSignaturesForAddress (indexer)", hist.length);
} else {
  await assert.rejects(conn.getSignaturesForAddress(payer.publicKey), /history not available/);
  ok("getSignaturesForAddress senza indexer: errore -32011 chiaro");
}
const infos = await conn.getMultipleAccountsInfo([payer.publicKey, to, web3.Keypair.generate().publicKey]);
assert.equal(infos[2], null);
ok("getMultipleAccounts");

// Errore di preflight: trasferimento oltre il saldo.
const bad = new web3.Transaction().add(web3.SystemProgram.transfer({ fromPubkey: payer.publicKey, toPubkey: to, lamports: 10n ** 15n }));
await assert.rejects(conn.sendTransaction(bad, [payer]), /simulation failed/);
ok("preflight rifiuta tx invalida");

// SPL Token: mint, ATA, mint_to, saldi.
const mint = await spl.createMint(conn, payer, payer.publicKey, null, 6);
const ata = await spl.getOrCreateAssociatedTokenAccount(conn, payer, mint, payer.publicKey);
await spl.mintTo(conn, payer, mint, ata.address, payer, 12_500_000);
const tb = await conn.getTokenAccountBalance(ata.address);
assert.equal(tb.value.uiAmountString, "12.5");
ok("getTokenAccountBalance", tb.value);
const owned = await conn.getParsedTokenAccountsByOwner(payer.publicKey, { programId: spl.TOKEN_PROGRAM_ID });
assert.equal(owned.value[0].account.data.parsed.info.tokenAmount.uiAmountString, "12.5");
ok("getTokenAccountsByOwner(jsonParsed)", owned.value.length);
const byMint = await conn.getTokenAccountsByOwner(payer.publicKey, { mint });
assert.equal(byMint.value.length, 1);
ok("getTokenAccountsByOwner(mint, base64)");
const mintInfo = await conn.getParsedAccountInfo(mint);
assert.equal(mintInfo.value.data.parsed.info.decimals, 6);
ok("getAccountInfo(jsonParsed mint)");

console.log("TUTTO OK");
process.exit(0);
