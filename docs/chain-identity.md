# Identità della chain

Ogni nodo centrifuga dichiara quale chain serve. L'identità è fissata nel genesis e
fa parte del **genesis hash**: non si può cambiare dopo il lancio senza creare
un'altra chain. Il formato degli indirizzi e delle transazioni resta quello di
Solana, senza istruzioni aggiuntive obbligatorie: l'identità si legge dal nodo, non
dalle transazioni.

## Campi

| campo | valore | note |
|---|---|---|
| `chainId` | `centrifuga:mainnet` o `centrifuga:devnet` | sempre `centrifuga:<network>`; il genesis fallisce altrimenti |
| `name` | `Centrifuga` | |
| `symbol` | `CnFA` | esattamente così, maiuscole e minuscole comprese; il genesis fallisce altrimenti |
| `decimals` | `9` | 1 CnFA = 10⁹ lamport |
| `genesisHash` | base58, 32 byte | hash del blocco 0 |
| `logoUrl` | URL o stringa vuota | **non** fa parte del genesis: è configurazione del nodo |

Il blocco 0 ha come `prev_hash` lo SHA-256 dei parametri di consenso, identità
compresa (stringhe precedute dalla lunghezza u32 little-endian, decimali come un
byte). Due chain con identità diversa hanno quindi genesis hash diversi, e così
ogni blockhash successivo.

## Come leggerla

### JSON-RPC `getChainInfo`

```json
{"jsonrpc":"2.0","id":1,"method":"getChainInfo"}
```

```json
{"jsonrpc":"2.0","id":1,"result":{
  "chainId":"centrifuga:mainnet","name":"Centrifuga","symbol":"CnFA","decimals":9,
  "genesisHash":"<base58>","logoUrl":"https://…"}}
```

### HTTP `GET /chain.json`

Stesso oggetto di `getChainInfo` (senza l'involucro JSON-RPC), sulla stessa porta del
JSON-RPC, con `Access-Control-Allow-Origin: *`.

### `getVersion`

I campi usati dagli SDK Solana restano (`solana-core`, `feature-set`); in più
`chainId`, `centrifuga` (versione del nodo) e `token: {name, symbol, decimals}`.

### Stream

Il primo messaggio di ogni connessione è
`{"type":"chain","chainId":"…","genesisHash":"…"}`. `stream::Replica::expect_chain`
rifiuta un nodo con chain_id o genesis hash diversi da quelli attesi, e ogni messaggio
ricevuto prima dell'identità.

## Cosa deve fare un client (wallet, bot, replica)

1. Tenere **fissati** nella propria configurazione `chainId` e `genesisHash` della
   chain a cui vuole parlare (non fidarsi del primo nodo che risponde).
2. Alla connessione leggere `getChainInfo` (o `/chain.json`) e confrontare
   `chainId` **e** `genesisHash` (anche con `getGenesisHash`): se uno dei due è
   diverso, rifiutare il nodo.
3. Mostrare il simbolo `CnFA` e 9 decimali; non mostrare mai "SOL".

## Perché una transazione non vale su un'altra chain

Una transazione Solana contiene il blockhash recente su cui è firmata. Genesis
diversi producono blockhash diversi, quindi un nodo di un'altra chain risponde
`BlockhashNotFound` (test `transactions_do_not_cross_chains`). Le transazioni con
durable nonce dipendono da un account nonce che esiste solo sulla chain dove è stato
creato.
