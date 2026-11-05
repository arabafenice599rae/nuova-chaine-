# centrifuga

Chain pubblica compatibile con Solana (SVM) a **validatore unico**, in un solo binario.
Nessun consenso, nessun voto, nessun relay: chi costruisce il blocco è chi lo firma.

Token nativo: **CENTRIFUGA**, simbolo **CnFA** (forma canonica), 9 decimali. È il lamport
dell'SVM: i wallet esterni (Solflare, Phantom) lo mostreranno comunque come "SOL".
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
**Due header con firma valida e la stessa altezza ma hash diverso** sono una prova
crittografica che l'operatore ha creato un fork: chiunque li possieda può pubblicarli.
Un blocco il cui hash di stato non corrisponde alla riesecuzione è un'altra prova.

## Blocchi

- Slot ogni 100 ms (`slot_ms`), anche vuoti: le altezze misurano il tempo
  (sblocchi dei vault, fee mining).
- Header: altezza, hash precedente, timestamp (ms, monotono:
  `max(ora, precedente + 1)`), hash di stato, firma dell'operatore. Corpo: le tx
  nell'ordine eseguito.
- A ogni blocco la sysvar Clock riceve `slot = altezza` e `unix_timestamp`.
- Hash di stato: SHA-256 su tutti gli account ordinati per pubkey (dietro
  l'interfaccia `StateHasher`, sostituibile da un hasher incrementale).
- Blockhash = hash del blocco. Una tx è valida se il suo blockhash è tra gli ultimi
  N blocchi (`blockhash_window`, default 300 ≈ 30 s), oppure se usa un durable nonce
  valido. Filtro anti-duplicati sulle firme degli ultimi N blocchi.
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
   (6,350609621 CnFA di sysvar, builtin, feature e programmi), = 100.000.000 CnFA esatti;
2. **determinismo**: costruisce due volte, produce 20 blocchi di test su una copia,
   li riesegue sull'altra e confronta gli hash.

Allocazione (vault del programma timelock):

| Vault      | Importo                    | Sblocco                                     |
|------------|----------------------------|---------------------------------------------|
| `users`    | 50M CnFA                   | 20M airdrop + 30M fee mining                |
| `operator` | 20M CnFA                   | lineare per blocco in 48 mesi               |
| `reserve`  | 30M − account di sistema   | bloccata fino al blocco 315.360.000 (~12 mesi) |

Programmi nel genesis: System, SPL Token, Token-2022, Associated Token Account,
Compute Budget, Memo, precompile ed25519/secp256k1 (più quelli che litesvm carica
di default: Address Lookup Table, Stake).

## Stack

- Esecuzione: [`litesvm`](https://crates.io/crates/litesvm) `=0.18.2`; tutte le chiamate
  in `src/exec/litesvm_internal.rs`.
- Firme: verifica ed25519 in batch (`ed25519-dalek`) all'ingresso e in riesecuzione.
- Nessun database: journal + snapshot.

## Struttura

```
src/main.rs        CLI (genesis, run)
src/node.rs        apertura (snapshot + journal), ciclo di slot
src/exec/          esecuzione blocchi, finestra blockhash, nonce, StateHasher
src/ledger/        blocco, journal, snapshot
src/ingress/       verifica firme batch, seq, coda FIFO
src/genesis/       config TOML, stato iniziale, verifiche
programs/timelock/ programma on-chain dei vault (passo 3)
```

## Stato

- [x] Passo 1: genesis, transfer, blocco firmato, riavvio dal journal, stesso hash.
- [ ] Passo 2: fee mining e rimborso priority fee.
- [ ] Passo 3: programma timelock (withdraw lineare, claim_airdrop).
- [ ] Passo 4: RPC minimo, stream, test con Solflare.

Licenza: MIT.
