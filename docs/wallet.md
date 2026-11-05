# `centrifuga wallet`

Wallet a riga di comando (messaggi in inglese). Parla con un nodo via JSON-RPC e
**rifiuta qualsiasi nodo** il cui `chainId` o `genesisHash` (da `getChainInfo`) non sia
quello fissato in `wallet.toml`. Il controllo è obbligatorio anche in HTTPS.

```toml
# wallet.toml (default nella cartella corrente; altrimenti --config FILE)
rpc = "https://rpc.example.org"
keypair = "/home/io/.config/centrifuga/id.json"
chain_id = "centrifuga:devnet"
genesis_hash = "<genesis hash della chain>"
```

```
centrifuga wallet new                               # nuova chiave (JSON di solana-keygen, permessi 600)
centrifuga wallet address
centrifuga wallet balance [address]                 # CnFA e token SPL (Token e Token-2022)
centrifuga wallet send <address> <amount>           # CnFA, es. 1.5
centrifuga wallet token-send <mint> <address> <amount>
centrifuga wallet history [address]                 # ultime 10 transazioni
```

Opzioni: `--rpc URL` e `--keypair FILE` (prevalgono su `wallet.toml`), `--config FILE`,
`--yes` (niente conferma, per gli script).

## Da dove prendere `chain_id` e `genesis_hash`

**Mai dal nodo a cui ci si collega**: `getChainInfo`, `/chain.json` e `getGenesisHash`
di quel nodo servono solo a *confrontare*. Un nodo falso risponderebbe con i propri
valori, e copiarli renderebbe inutile il controllo.

### Mainnet

I valori di mainnet **non esistono ancora**: nascono con il genesis definitivo. In
quel momento vengono pubblicati quattro valori in tre posti (procedura in
[release.md](release.md)):

| valore | cos'è |
|---|---|
| `chain_id` | `centrifuga:mainnet` |
| `genesis_hash` | hash del blocco 0 (base58) |
| SHA-256 del timelock | hash di `programs/timelock/centrifuga_timelock.so` |
| commit | commit Git da cui si costruisce il binario che calcola il genesis |

I tre posti sono:

1. il **tag Git firmato** `mainnet-genesis` (valori nel messaggio del tag);
2. il **README** al commit del tag (sezione "Identità di mainnet");
3. il **sito**.

Il file di configurazione del genesis (`genesis/mainnet.toml`) è committato nel
commit del tag. Contiene la sola pubkey dell'operatore, nessuna chiave privata. È
l'unico dato che serve per il ricalcolo, oltre al codice.

### Verificare che i tre valori coincidano

```sh
git fetch --tags
git tag -v mainnet-genesis          # firma valida? la chiave è quella attesa?
git show -s --format=%B mainnet-genesis   # chain_id, genesis_hash, SHA-256 timelock, commit
git rev-list -n 1 mainnet-genesis   # deve essere il commit indicato nel messaggio
git show mainnet-genesis:README.md | sed -n '/Identità di mainnet/,/^## /p'
```

Poi si confrontano gli stessi quattro valori con la pagina del sito. Basta una
differenza (o una firma non valida) per non usare nessuno dei valori e segnalarlo.

### Ricalcolarli con `centrifuga genesis`

Da un clone pulito, senza scaricare nulla dal nodo:

```sh
git checkout mainnet-genesis
cargo build --release --locked
sha256sum programs/timelock/centrifuga_timelock.so   # = SHA-256 del timelock pubblicato
target/release/centrifuga genesis genesis/mainnet.toml
#   Centrifuga (CnFA, centrifuga:mainnet), ...     -> chain_id
#   genesis hash: <base58>                          -> genesis_hash
```

Facoltativo, e serve la toolchain SBF: ricompilare il timelock (README,
"Build riproducibile") e verificare che lo SHA-256 coincida con il `.so` committato.

Se `chain_id` e `genesis_hash` ricalcolati coincidono con quelli pubblicati, vanno in
`wallet.toml`.

### Devnet

Sul server, `centrifuga genesis /etc/centrifuga/centrifuga.toml` stampa le stesse
righe. Il genesis hash cambia a ogni `--reset` della devnet.

## HTTPS

- Per i nodi remoti `https://` è il default: un URL senza schema è trattato come
  `https://`. Certificato e nome host sono sempre verificati, con le CA pubbliche di
  Mozilla (`webpki-roots`) compilate nel binario. Non esiste un'opzione per
  disattivare la verifica né per aggiungere CA.
- `http://` è accettato solo verso `localhost` o `127.0.0.1`. Verso qualsiasi altro
  host il nodo è rifiutato prima di aprire una connessione.
- Solo TLS 1.3 (configurazione minima di `rustls`, backend `ring`).

## Comportamento

- `new` non sovrascrive mai un file esistente. Va bene anche una chiave già creata con
  `solana-keygen`.
- Prima di ogni invio mostra destinatario, importo e fee e chiede `send? [y/N]`: solo
  `y` o `yes` confermano. Dopo l'inclusione mostra fee pagata, premio del fee mining
  (`reward`), rimborso della priority fee (`refund`) e costo netto (`net cost`).
- `token-send` crea l'account token associato del destinatario se manca (il deposito
  per la rent è mostrato prima della conferma).
- Importi in decimale esatto: niente virgola mobile; un importo con più decimali del
  token è un errore.
- Non stampa mai la chiave privata; l'output non contiene mai "SOL".
