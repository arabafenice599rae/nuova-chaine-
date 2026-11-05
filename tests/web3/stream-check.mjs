// Controllo dello stream: connessione, stato completo, poi blocchi live con delta.
// Uso: node stream-check.mjs <wss://host/stream>
const url = process.argv[2];
const ws = new WebSocket(url);
const seen = [];
const done = (ok, msg) => { console.log(`${ok ? "ok" : "ERRORE"} ${msg}`); ws.close(); process.exit(ok ? 0 : 1); };
setTimeout(() => done(false, `timeout, messaggi: ${JSON.stringify(seen)}`), 20000);
ws.onerror = (e) => done(false, `errore WebSocket: ${e.message ?? e}`);
ws.onmessage = (ev) => {
  const m = JSON.parse(ev.data);
  if (m.type === "chain") { seen.push(`chain(${m.chainId})`); return; }
  seen.push(m.type === "sync" ? `sync(full=${m.full}, ${m.accounts.length} account)` : `block ${m.header.height}${m.accounts ? ` +${m.accounts.length}` : ""}`);
  const live = seen.filter((s) => s.startsWith("block") && s.includes("+"));
  if (live.length >= 2) {
    const ok = seen[0].startsWith("chain(centrifuga:") && seen[1].startsWith("block") && seen[2].startsWith("sync(full=true");
    done(ok, `stream: ${seen.join(", ")}`);
  }
};
