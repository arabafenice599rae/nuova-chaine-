# centrifuga

Chain pubblica compatibile con Solana (SVM) a **validatore unico**, in un solo binario.
Nessun consenso, nessun voto, nessun relay: chi costruisce il blocco è chi lo firma.

Chain **Centrifuga** (`centrifuga:mainnet`, `centrifuga:devnet`), token nativo con simbolo **CnFA** (forma canonica), 9 decimali. È il lamport
dell'SVM: i wallet esterni lo mostreranno comunque come "SOL".
Supply fissa di 100.000.000 CnFA, zero inflazione, zero staking.

## Modello di fiducia (onesto)

L'operatore **può**:
- censurare transazioni (non includerle);
- scegliere l'ordine di arrivo, cioè assegnare i numeri progressivi (seq);
- fermare la chain.

L'operatore **non può** (senza essere scoperto):
- eseguire le transazioni in un ordine diverso da quello che pubblica, o con regole
  diverse: chiunque riesegue il journal e confronta l'hash di stato di ogni header;
- spendere fondi altrui: ogni transazione porta le firme ed25519 dei mittenti;
- creare CnFA: la somma dei lamport è verificabile e costante (100M esatti);
- riscrivere la storia: vedi *prova di frode*.

Tutto tranne censura e ordine di arrivo è verificabile da chiunque rieseguendo il journal.

### Prova di frode

Ogni blocco è firmato dall'operatore. La firma copre
`sha256(altezza ‖ hash_precedente ‖ timestamp ‖ hash_stato ‖ sha256(corpo))`.
**Due header diversi con firma valida e la stessa altezza, oppure lo stesso
blocco precedente** (un fork, anche ad altezze diverse) sono una prova
crittografica che l'operatore ha creato un fork: chiunque li possieda può pubblicarli.
Un blocco il cui hash di stato non corrisponde alla riesecuzione è un'altra prova.

## Blocchi

- Slot ogni 100 ms (`slot_ms`). Un blocco esce solo se ci sono transazioni in coda,
  oppure ogni `heartbeat_slots` slot (default 10 = 1 s) come segnale di presenza.
- **Regole di validità** (parametri del genesis, inclusi nel suo hash), controllate da
  `Chain::apply`, dalla riesecuzione del journal e da `stream::Replica`:
  - salto = altezza − altezza precedente, con **1 ≤ salto ≤ `heartbeat_slots`**;
  - **timestamp − precedente ≥ salto × `slot_ms` / 2** (e almeno 1 ms);
  - la replica in tempo reale rifiuta anche i timestamp oltre 5 s nel suo futuro.
- Il produttore usa come salto gli slot trascorsi dall'ultimo blocco (al massimo
  `heartbeat_slots`) e come timestamp max(ora, precedente + salto × slot_ms / 2,
  precedente + 1). Dopo un riavvio riparte da ultima altezza + 1: il tempo di fermo non
  viene recuperato, quindi l'altezza misura il tempo di attività della chain.
- Un blocco vuoto occupa 152 byte: con l'heartbeat a 1 s il journal cresce di ~13 MB
  al giorno senza traffico (contro ~130 MB con un blocco a ogni slot).
- Header: altezza, hash precedente, timestamp (ms, monotono:
  `max(ora, precedente + 1)`), hash di stato, firma dell'operatore. Corpo: le tx
  nell'ordine eseguito.
- A ogni blocco la sysvar Clock riceve `slot = altezza` e `unix_timestamp`.
- Genesis hash: il blocco 0 ha come `prev_hash` lo SHA-256 dei parametri di consenso
  (operatore, fee, finestra, fee mining, `slot_ms`, `heartbeat_slots`, vault): chi
  verifica con parametri diversi ottiene un altro genesis.
- Hash di stato: LtHash omomorfo come in Agave (`solana-lattice-hash`, schema
  `lthash-blake3-v1`, che entra nei parametri di consenso e quindi nel genesis hash). È
  la somma degli LtHash degli account; a ogni blocco si toglie il contributo precedente
  degli account cambiati e si aggiunge il nuovo, quindi il costo dipende dagli account
  cambiati e non da quelli totali. Nell'header va il checksum blake3 (32 byte).
  Ricalcolo completo solo all'avvio, come verifica dello snapshot. Misure in
  [docs/load-test.md](docs/load-test.md).
- Blockhash = hash del blocco. Il blockhash del blocco all'altezza h vale nei blocchi
  fino all'altezza h + N (`blockhash_window`, default 300 slot = 30 s), oppure la tx
  usa un durable nonce valido. Anti-duplicati: le firme restano note per lo stesso
  intervallo. `getLatestBlockhash` riporta `lastValidBlockHeight` = h + N.
- Ordine: FIFO rigoroso per seq di arrivo. La priority fee **non** cambia l'ordine.

## Persistenza

`data_dir/journal.bin`: append-only, record `[len u32][blocco]`, fsync per blocco.
`data_dir/snapshot.bin`: ogni `snapshot_every` blocchi, scritto su file temporaneo e
rinominato atomicamente. Al riavvio: carica lo snapshot, verifica il suo hash di stato
contro l'header del journal, riesegue il resto del journal.

## Genesis

```
cargo run --release --example keygen > operator.json   # stampa la pubkey su stderr
cp centrifuga.example.toml centrifuga.toml               # inserire la pubkey operatore
centrifuga genesis centrifuga.toml
centrifuga run centrifuga.toml
```

`centrifuga genesis` costruisce lo stato iniziale e verifica:
1. **conservazione**: somma di tutti i lamport, account di sistema inclusi
   (5,4523437 CnFA di sysvar, builtin, feature, programmi e vault), = 100.000.000 CnFA esatti;
2. **determinismo**: costruisce due volte, produce 20 blocchi di test su una copia,
   li riesegue sull'altra e confronta gli hash.

Allocazione (vault del programma timelock):

| Vault      | Importo                    | Uso / sblocco                                         |
|------------|----------------------------|-------------------------------------------------------|
| `airdrop`  | 20M CnFA                   | claim: 100 CnFA fissi ai primi 200.000 aventi diritto; poi il residuo del fee mining; claim fino al blocco 708.480.000 |
| `mining`   | 30M CnFA                   | premi del fee mining (lo muove solo il nodo)          |
| `operator` | 5M CnFA                    | withdraw lineare per blocco in 48 mesi                |
| `reserve`  | 45M − account di sistema   | solo claim a ondate dal blocco 315.360.000 (~12 mesi); riceve il 90% delle fee base durante il fee mining |

Con `network = "mainnet"` gli importi sono fissi: `centrifuga genesis` rifiuta valori
diversi da 20M / 30M / 5M, uno sblocco dell'operatore diverso da 48 mesi e una riserva
diversa da 45M meno gli account di sistema. Su devnet gli importi sono configurabili;
la conservazione a 100M vale per entrambe le reti.

**Lista airdrop**: solo pubkey da uno snapshot dello storico di Solana mainnet. Niente
inviti, dati personali, collegamenti social o compiti da svolgere.

**Operatore**: il vault `operator` esce solo con `withdraw` verso il beneficiario
registrato. Né il genesis né il timelock contengono programmi o istruzioni per vendere
CnFA dell'operatore (test: `operator_vault_has_no_other_exit` e l'elenco chiuso dei
programmi del genesis).

`network = "mainnet" | "devnet"`. I conti `[[genesis.prefund]]` (sottratti alla
riserva) sono ammessi solo su devnet: su mainnet `centrifuga genesis` fallisce.

## Fee

- Fee base: 5000 lamport per firma (incluse le firme verificate dalle precompile),
  burn 0: la supply resta esattamente 100M.
- **Priority fee**: rimborsata al pagatore nello stesso blocco ed esclusa da ogni
  calcolo. Non cambia l'ordine (FIFO per seq): pagarla non serve a nulla.
- **Fee mining** (finché altezza < `fee_mining_end_height`, 24 mesi, e il vault
  `mining` ha saldo), a ogni blocco:
  - 10% della fee base all'operatore, 90% al vault `reserve`;
  - premio = min(`budget_blocco`, 90% della fee base, saldo di `mining`), pagato
    dal vault `mining` ai pagatori in proporzione alla fee base pagata (il resto
    della divisione intera resta nel vault);
  - `budget_blocco` parte da 0,01 CnFA e dimezza ogni `halving_blocks` (6 mesi).
  - Un operatore che fa transazioni a se stesso riceve al massimo 10% + 90% di
    quanto paga: guadagno netto ≤ 0.
- **Fine del fee mining** (altezza raggiunta o vault `mining` vuoto): 100% della fee
  base all'operatore; il residuo di `mining` passa ad `airdrop` nel primo blocco
  successivo. Il nodo legge solo il saldo di `mining` e l'altezza di fine.
- Il residuo si distribuisce con claim agli aventi diritto (vedi *Programma
  timelock*).
- `preBalances`/`postBalances` di getTransaction riflettono la tx; rimborso e
  premio sono accreditati a fine blocco.

Il nodo conosce solo budget, regola 90/10, rimborso della priority fee e data di
fine; sblocchi e claim sono nel programma timelock.

## Programma timelock

`programs/timelock/` (Pinocchio 0.9.2, zero dipendenze transitive, < 400 righe),
deployato nel genesis come programma **immutabile** (loader upgradeable senza
autorità di upgrade). Il nodo include il `.so` nel binario.

| Istruzione | Chi | Cosa |
|---|---|---|
| `withdraw` | chiunque | solo vault `operator`: sblocco lineare per altezza; paga sempre il beneficiario registrato |
| `init_wave(importo)` | autorità (operatore) | apre un'ondata a importo fisso su `airdrop` o `reserve`, con bitmap anti-doppio-claim |
| `claim(indice)` | chiunque | paga l'importo dell'ondata se l'istruzione precedente è la precompile ed25519 con la firma dell'operatore su (ondata, destinatario, importo, indice) |
| `to_reserve` | chiunque | dal blocco `claims_end_height` (708.480.000 = 24 mesi + 90 giorni) sposta tutto il saldo di `airdrop` in `reserve` |

- `reserve` esce **solo** tramite claim, dal blocco 315.360.000: nessun withdraw
  verso l'operatore o altri.
- `airdrop`: claim rifiutati dal blocco `claims_end_height`.
- `mining`: nessuna istruzione.
- Ogni uscita è limitata dal **saldo reale** del vault (meno il minimo per la rent
  exemption), non da un totale fisso.

### Build riproducibile

```
cargo install cargo-build-sbf --version 4.5.0 --locked
cd programs/timelock && cargo build-sbf     # platform-tools fissate: v1.57 (Cargo.toml)
sha256sum target/deploy/centrifuga_timelock.so
```

SHA-256 di `programs/timelock/centrifuga_timelock.so` (SBPF v3):

```
1cd1e548557ed8ed4445b22f57b514e242cffe3e0bb1ebc6fe9ac8c37324ea93
```

Verificato con due compilazioni pulite (`cargo clean` prima di ciascuna) e una terza
in un'altra cartella: stesso hash.

### Residuo del fee mining

- Ha diritto chi è nella lista airdrop (snapshot dello storico Solana) e, **come
  pagatore**, ha almeno 10 tx riuscite in almeno 5 giorni UTC diversi (timestamp
  dei blocchi del periodo di fee mining).
- Quota uguale per tutti: min(1.000 CnFA, residuo / numero aventi diritto).
  La parte non assegnata resta in `airdrop` e passa a `reserve` con `to_reserve`
  alla scadenza dei claim.
- Lista deterministica, rieseguibile da chiunque con journal e config:

```
centrifuga eligible centrifuga.toml lista_airdrop.txt residuo.txt   # stampa lo sha256
```

Hash della lista: *da pubblicare a fine fee mining* (blocco 630.720.000).

Programmi nel genesis: System, SPL Token, Token-2022, Associated Token Account,
Compute Budget, Memo, precompile ed25519/secp256k1 (più quelli che litesvm carica
di default: Address Lookup Table, Memo v1, precompile secp256r1, ZK ElGamal proof).
L'elenco è chiuso: un test fallisce se nel genesis compare un altro programma.
**Vote e Stake non ci sono**: litesvm li carica di default, il genesis li chiude con l'API
pubblica (account vuoto: programma chiuso e account cancellato) e il nodo li richiude dopo
ogni riavvio da snapshot. Non c'è consenso a stake né ricompense; una tx che li invoca
fallisce (test `vote_and_stake_programs_are_absent`). Restano due account creati da
litesvm con le sysvar, **dati inerti senza programma**: la configurazione di Stake
(`StakeConfig11111111111111111111111111111111`) e la sysvar StakeHistory, vuota. Nessun
programma li legge o li modifica, e non hanno effetti su fee, ricompense o consenso..

## Deploy di programmi

`solana program deploy` (CLI Agave 4.3.2) funziona contro il nodo con `--use-rpc`:

```
solana -u https://<nodo> program deploy programma.so --use-rpc
```

Senza `--use-rpc` la CLI invia le scritture via TPU/QUIC, ricavando i leader con
`getClusterNodes` e `getEpochSchedule`. Qui c'è un solo validatore e niente QUIC, quindi
il deploy si ferma con "Failed to initialize LeaderTpuCacheService". Gli aggiornamenti
usano lo stesso comando con lo stesso `--program-id`. La CI lo verifica con
`scripts/cli-deploy.sh`: deploy, chiamata, upgrade, chiamata, riavvio del nodo e
chiamata. `tests/step9_deploy.rs` verifica la stessa sequenza e la riesecuzione dal
journal.

## JSON-RPC per i wallet

**Durabilità.** Il journal si scrive su un thread separato: il nodo esegue il blocco
successivo mentre il precedente va su disco (al massimo `journal_queue_blocks` blocchi
eseguiti e non ancora durevoli, default 10 = 1 s; oltre, il nodo aspetta il disco). Blocchi, conferme e stream escono solo dopo
che il blocco è durevole:
- `getSlot`, `getBlockHeight`, `getEpochInfo`, `getLatestBlockhash`: l'ultimo blocco
  durevole, qualunque commitment;
- `getSignatureStatuses`, `getTransaction`: solo tx di blocchi durevoli (prima, nessuno
  stato);
- stream: blocchi e recupero solo durevoli;
- `getBalance`, `getAccountInfo`, `simulateTransaction`: lo stato eseguito, al massimo
  `journal_queue_blocks` blocchi avanti. **Dopo un crash queste letture possono tornare
  indietro di al massimo `journal_queue_blocks` blocchi; conferme e blocchi mai.**

Il gateway (`src/rpc/`) è solo un adattatore per i wallet: HTTP su `rpc_addr`
(default `127.0.0.1:8899`), WebSocket sulla porta successiva (8900), come si
aspettano i client Solana. I metodi girano sul thread del nodo tra un blocco e
l'altro: niente lock, risposte sempre coerenti con l'ultimo blocco.

Metodi HTTP: `getBalance`, `getAccountInfo`, `getMultipleAccounts`,
`getTokenAccountsByOwner`, `getTokenAccountBalance`, `getLatestBlockhash`,
`isBlockhashValid`, `getBlockHeight`, `getSlot`, `getEpochInfo`, `getGenesisHash`,
`getFeeForMessage`, `getMinimumBalanceForRentExemption`,
`getRecentPrioritizationFees`, `simulateTransaction`, `sendTransaction`,
`getSignatureStatuses`, `getSignaturesForAddress`, `getTransaction`, `getVersion`,
`getHealth`, `getChainInfo` (identità della chain, anche su `GET /chain.json`: vedi
[docs/chain-identity.md](docs/chain-identity.md)). WebSocket: `accountSubscribe`, `signatureSubscribe` (e unsubscribe).

Differenze da Solana, per scelta:
- tutto è subito `finalized` (un solo firmatario, nessun fork); `commitment` è ignorato;
- nessuna epoca: `getEpochInfo` riporta `epoch: 0`;
- `getRecentPrioritizationFees` è sempre 0: la priority fee non compra posizione;
- `getFeeForMessage` riporta solo la fee base (la priority fee è rimborsata);
- `getVersion` aggiunge `centrifuga`, `chainId` e `token: {name, symbol, decimals}`;
- `jsonParsed`: token account e mint SPL interpretati; transazioni in forma
  parzialmente decodificata (istruzioni con `programId`, `data` in base58);
  `preTokenBalances`/`postTokenBalances` e `innerInstructions` vuoti;
- getTransaction/getSignaturesForAddress coprono le tx dall'ultimo snapshot in poi
  (indice in memoria ricostruito dal journal al riavvio);
- ogni metodo non supportato risponde `-32601`. Con `rpc_log = true` il nodo scrive
  in `data_dir/rpc.log` una riga per richiesta (`unix_ms canale metodo esito`, con
  `UNSUPPORTED` per i metodi mancanti): è così che si scopre cosa chiede un wallet.

Test con un client reale (`@solana/web3.js` + `@solana/spl-token`), che avvia da solo
un nodo devnet con un conto prefinanziato:

```
cd tests/web3 && npm ci && cd ../..
scripts/web3-node.sh rpc.mjs
```

Nodo devnet su VPS: vedi [docs/devnet.md](docs/devnet.md) (nodo devnet,
endpoint HTTPS pubblico, prefund, latenza).

## Stream

L'API principale per bot e repliche (`src/stream/`): WebSocket su `stream_addr`
(default `127.0.0.1:8901`), un messaggio JSON per frame.

- A ogni blocco: `{"type":"block", "header":{height, prev_hash, timestamp_ms,
  state_hash, signature}, "txs":[base64], "accounts":[...]}` con il valore, dopo il
  blocco, di ogni account cambiato (`lamports: 0` = cancellato).
- `ws://host:8901/?from_height=h`: il client riceve dal **journal** i blocchi da h
  in poi (solo header e corpo) e poi `{"type":"sync","full":false,"accounts":[...]}`
  con il valore attuale di ogni account cambiato da h, se h è nella finestra del
  recupero: ultimi `stream_catchup_blocks` blocchi e al massimo `stream_catchup_bytes`
  (default 128 MB) di indirizzi in RAM, vale il primo limite raggiunto. Senza
  `from_height` riceve lo stato completo (`"full":true`) ancorato all'ultimo blocco.
- `from_height` fuori dalla finestra: `{"type":"error","code":"from_height_out_of_window",
  "oldest":H,...}` e chiusura 1008. Il client si riconnette senza `from_height` e
  riparte dallo stato completo; `Replica::resume_height()` restituisce già 0 dopo
  l'errore (test `replica_in_and_out_of_byte_window`).
- Il client verifica la firma dell'operatore, la continuità `prev_hash` e l'hash di
  stato (a ogni blocco live e dopo ogni sync). `stream::Replica` fa tutto questo: il
  test `replica_rebuilds_state_from_stream_only` ricostruisce lo stato solo dallo
  stream, con due riconnessioni, e ottiene lo stesso hash degli header del nodo.
- Il nodo non aspetta mai i client: un canale broadcast con capacità
  `stream_max_lag_blocks`. Chi resta indietro di più (o ha un invio bloccato per più
  di `stream_max_lag_blocks × slot_ms`) viene disconnesso e si riconnette con
  `from_height`. Il messaggio di un blocco si costruisce solo se c'è almeno un client.

## Ingresso delle transazioni

Per ora HTTP (`sendTransaction`); QUIC si valuterà con i numeri qui sotto.
`ingress::Ingress::submit(mittente, txs)` è l'interfaccia comune: verifica le firme in
batch, applica i limiti e assegna il seq. Un futuro server QUIC chiamerà la stessa.

- Limite per IP: `ingress_tps_per_ip` tx/s (burst di 2 s). Dietro un proxy locale
  vale l'IP in `X-Forwarded-For`.
- Limite per pagatore: al massimo `ingress_pending_per_payer` tx in coda non ancora
  eseguite (un pagatore non può riempire la coda FIFO).
- Rifiuti: errore `-32003` con il motivo.

Latenza da invio a blocco firmato (`tests/web3/latency.mjs`, 200 tx ad arrivo casuale):
**totale** = `sendTransaction` con preflight → notifica `signatureSubscribe`, emessa
dopo firma e scrittura del journal; **nodo** = dall'arrivo della richiesta al nodo al
blocco firmato, misurato dal nodo (`centrifugaNodeLatencyUs` in `getTransaction`);
**rete** = totale − nodo. Nodo e client sulla stessa macchina (loopback), slot 100 ms, ms:

| misura | media | p50 | p95 | p99 |
|---|---|---|---|---|
| RTT HTTP (`getHealth`) | 1,1 | 0,9 | 2,2 | 2,9 |
| totale | 51,5 | 49,8 | 98,4 | 101,3 |
| nodo | 49,4 | 48,3 | 96,5 | 99,6 |
| rete | 2,1 | 1,8 | 3,7 | 7,7 |

- Produzione di un blocco vuoto (esecuzione, hash di stato, firma, fsync del journal):
  circa 1,05 ms (9.514 blocchi in 10 s con `slot_ms = 1`).
- Il tempo nel nodo è l'attesa dello slot (in media metà slot). In una raffica pesa
  anche il preflight, eseguito una tx alla volta sul thread del nodo (60 tx in
  parallelo: p50 160 ms con slot 100 ms).
- Su loopback la rete è trascurabile: QUIC si valuta con la stessa misura fatta da
  una rete reale verso un VPS (vedi [docs/devnet.md](docs/devnet.md)).

Riprodurre in locale: `scripts/web3-node.sh latency.mjs 100` (slot in ms come secondo
argomento); verso un nodo remoto: `node latency.mjs https://<dominio> bench.json`.

## Deploy devnet

`deploy/devnet-install.sh` installa su un VPS Linux pulito un nodo devnet come servizio
systemd, con HTTPS (Caddy + Let's Encrypt), chiavi generate sul server e conti
prefinanziati. Passi e requisiti in [docs/devnet.md](docs/devnet.md).

## CLI

```
centrifuga genesis [config]                 # costruisce e verifica il genesis
centrifuga run [config]                     # nodo: blocchi, RPC, WebSocket, stream
centrifuga balance <pubkey> [rpc_url]       # es. "12.5 CnFA" (simbolo da getVersion)
centrifuga eligible <config> <lista> <out>  # aventi diritto al residuo del fee mining
centrifuga wallet <new|address|balance|send|token-send|history>  # vedi docs/wallet.md
```

## CI

`.github/workflows/ci.yml`: build, `cargo test`, test web3.js contro un nodo devnet,
ricompilazione del programma timelock e confronto del suo SHA-256 con il README e con
il `.so` committato (`scripts/check-timelock-hash.sh`).

## Stack

- Esecuzione: [`litesvm`](https://crates.io/crates/litesvm) `=0.18.2`; tutte le chiamate
  in `src/exec/litesvm_internal.rs`.
- Firme: verifica ed25519 in batch (`ed25519-dalek`) all'ingresso e in riesecuzione.
- Nessun database: journal + snapshot.
- Rete: tokio, hyper (HTTP/1), tokio-tungstenite (WebSocket).
- Client CLI (wallet, balance): HTTPS con `rustls` (TLS 1.3, `ring`) e CA pubbliche
  `webpki-roots`; `http://` solo verso localhost.

## Struttura

```
src/main.rs        CLI (genesis, run, balance, eligible, wallet)
src/wallet.rs      wallet a riga di comando (verifica l'identità del nodo)
deploy/            installazione devnet su VPS (systemd, HTTPS)
src/stream/        stream WebSocket per bot e repliche, `Replica` che verifica tutto
src/rpc/           gateway JSON-RPC + WebSocket per i wallet
src/node.rs        apertura (snapshot + journal), ciclo di slot
src/exec/          esecuzione blocchi, finestra blockhash, nonce, StateHasher
src/ledger/        blocco, journal, snapshot
src/ingress/       verifica firme batch, seq, coda FIFO
src/genesis/       config TOML, stato iniziale, verifiche
programs/timelock/ programma on-chain dei vault (+ .so compilato)
src/claims.rs      aventi diritto al residuo del fee mining (off-chain)
```

## Stato

- [x] Passo 1: genesis, transfer, blocco firmato, riavvio dal journal, stesso hash.
- [x] Passo 2: fee mining 10/90, premio dal vault `mining`, rimborso priority fee, transizione.
- [x] Passo 3: programma timelock (withdraw lineare, ondate di claim, to_reserve), aventi diritto dal journal.
- [x] Passo 4: RPC, WebSocket, stream con recupero, limiti di ingresso, CI, `balance`.
- [ ] QUIC: da valutare con le latenze misurate da una rete esterna.

Licenza: MIT.
