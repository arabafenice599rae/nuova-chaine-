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

Fonti valide, da una fonte diversa dal nodo:

- **Mainnet**: i valori ufficiali vanno pubblicati in questo repository (README) al
  momento della generazione del genesis definitivo. Non esistono ancora. Chiunque
  può ricalcolarli dalla configurazione del genesis pubblicata:
  `centrifuga genesis centrifuga.toml` stampa la riga `genesis hash: …`, e
  `chain_id` è nella sezione `[genesis]`.
- **Devnet**: sul server, `centrifuga genesis /etc/centrifuga/centrifuga.toml` (stesse
  righe). Il genesis hash cambia a ogni `--reset` della devnet.

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
