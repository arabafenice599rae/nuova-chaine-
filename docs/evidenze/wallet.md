# Wallet con nodo personalizzato: esito dei test con le estensioni vere

Test eseguiti in Chromium (Playwright, headless) con le estensioni scaricate dal Chrome
Web Store, wallet nuovi e password usa e getta, contro un nodo centrifuga devnet locale.

## Solflare 2.41.2: non accetta un nodo personalizzato

- Onboarding automatizzato: wallet creato, impostazioni raggiunte.
- Impostazioni → Network offre solo **Mainnet, Testnet, Devnet**
  (`solflare-2.41.2-reti.png`); nessun campo per un URL RPC.
- Nel pacchetto (`static/wallet.e2ae3a23.js`) le tre reti hanno URL fissi
  (`solflare-app.mainnet.rpcpool.com`, `api.testnet.solana.com`, `api.devnet.solana.com`)
  più failover e invio tramite backend Solflare (`/v2/tx/rpc-extended/<rete>`).

## Backpack 0.10.219: accetta l'RPC personalizzato, ma il saldo viene dal suo backend

- Onboarding automatizzato: wallet nuovo, Impostazioni → Solana → RPC Connection →
  Custom → `http://127.0.0.1:8899`: accettato (`backpack-0.10.219-rpc-custom.png`).
- Con Developer Mode anche la rete **Solana Devnet** (dichiarata `read-and-write` per
  l'RPC personalizzato nel pacchetto) con lo stesso RPC.
- Il wallet riceve 5 CnFA dal faucet (confermato dal nodo: saldo 5.000.000.000 lamport),
  ma Backpack mostra **0 SOL** (`backpack-0.10.219-saldo-devnet.png`).
- Richieste registrate (`backpack-0.10.219-richieste.txt`): al nodo arriva solo
  `getAccountInfo`; i saldi sono chiesti a `https://backpack-api.xnfts.dev/v3/graphql`
  (`GetTokenBalances`, `GetWalletsBalanceForToken`) con `caip2` = genesis di Solana
  mainnet (`5eykt4Us…`) e devnet (`EtWTRABZ…`): il backend interroga le reti Solana, non
  il nostro nodo. Saldo, ricezione e token non sono quindi verificabili; invio non
  provato (senza saldo mostrato il wallet non permette di inviare).
