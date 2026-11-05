# `centrifuga wallet`

Wallet a riga di comando. Parla con un nodo via JSON-RPC e **rifiuta qualsiasi nodo**
il cui `chainId` o `genesisHash` (da `getChainInfo`) non sia quello fissato in
`wallet.toml`: si fissano una volta sola, da una fonte fidata (README, genesis).

```toml
# wallet.toml (default nella cartella corrente; altrimenti --config FILE)
rpc = "http://127.0.0.1:8899"
keypair = "/home/io/.config/centrifuga/id.json"
chain_id = "centrifuga:devnet"
genesis_hash = "<genesis hash della chain>"
```

```
centrifuga wallet new                               # nuova chiave (JSON di solana-keygen, permessi 600)
centrifuga wallet address
centrifuga wallet balance [indirizzo]               # CnFA e token SPL (Token e Token-2022)
centrifuga wallet send <indirizzo> <importo>        # CnFA, es. 1.5
centrifuga wallet token-send <mint> <indirizzo> <importo>
centrifuga wallet history [indirizzo]               # ultime 10 transazioni
```

Opzioni: `--rpc URL` e `--keypair FILE` (prevalgono su `wallet.toml`), `--config FILE`,
`--yes` (niente conferma, per gli script).

- `new` non sovrascrive mai un file esistente. Va bene anche una chiave già creata con
  `solana-keygen`.
- Prima di ogni invio mostra destinatario, importo e fee e chiede conferma. Dopo
  l'inclusione mostra fee pagata, premio del fee mining, rimborso della priority fee e
  costo netto.
- `token-send` crea l'account token associato del destinatario se manca (il deposito
  per la rent è mostrato prima della conferma).
- Importi in decimale esatto: niente virgola mobile, più decimali del token = errore.
- Non stampa mai la chiave privata; l'output non contiene mai "SOL".
- Limite: il client RPC parla solo `http://` (nodo locale o tunnel SSH verso il nodo).
