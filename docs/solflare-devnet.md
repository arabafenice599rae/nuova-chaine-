# Test con Solflare su un nodo devnet

Obiettivo: verificare che Solflare funzioni con un nodo centrifuga e scoprire, dal
log, quali metodi RPC chiede davvero. **Non si aggiungono metodi in anticipo**: solo
quelli che il log mostra come `NON_SUPPORTATO` e che servono a una funzione del wallet.

## 1. Chiavi e configurazione

Servono: un server con IP pubblico, un dominio (es. `rpc.example.org`) che punta al
server, Rust (la versione la fissa `rust-toolchain.toml`) e Node.js 22 per gli script.

```sh
git clone <repo> centrifuga && cd centrifuga
cargo build --release --bins --examples
./target/release/examples/keygen > operator.json   # stampa la pubkey dell'operatore
./target/release/examples/keygen > faucet.json     # conto per creare il token di prova
cp centrifuga.example.toml centrifuga.toml
```

In `centrifuga.toml`:

```toml
[node]
rpc_addr = "127.0.0.1:8899"    # resta locale: davanti c'è il proxy HTTPS (WebSocket su 8900)
rpc_log = true                 # registra ogni metodo ricevuto in data/rpc.log

[genesis]
network = "devnet"             # obbligatorio per usare prefund
operator = "<pubkey di operator.json>"

[[genesis.prefund]]            # il wallet Solflare da testare (indirizzo copiato dall'app)
pubkey = "<indirizzo Solflare>"
lamports = 100000000000        # 100 CnFA

[[genesis.prefund]]            # conto per lo script del token SPL
pubkey = "<pubkey di faucet.json>"
lamports = 10000000000         # 10 CnFA
```

I conti prefinanziati entrano solo nel genesis: per cambiarli, fermare il nodo e
cancellare `data/` (su devnet si riparte da zero).

```sh
./target/release/centrifuga genesis centrifuga.toml   # controlli di conservazione e determinismo
./target/release/centrifuga run centrifuga.toml
```

## 2. Endpoint HTTPS pubblico

I wallet richiedono HTTPS. Con l'URL `https://rpc.example.org` (senza porta) i client
basati su web3.js aprono il WebSocket su `wss://rpc.example.org`, stessa porta: il
proxy deve mandare le richieste di upgrade a 8900 e il resto a 8899. Esempio con
[Caddy](https://caddyserver.com) (certificato Let's Encrypt automatico, e passa
l'IP reale in `X-Forwarded-For`, che il nodo usa per i limiti per IP):

```
rpc.example.org {
    @ws header Connection *Upgrade*
    reverse_proxy @ws 127.0.0.1:8900
    reverse_proxy 127.0.0.1:8899
}
```

Controllo dall'esterno:

```sh
curl -s https://rpc.example.org -H 'content-type: application/json' \
  -d '{"jsonrpc":"2.0","id":1,"method":"getVersion"}'
./target/release/centrifuga balance <indirizzo Solflare> http://127.0.0.1:8899   # sul server: "100 CnFA"
```

## 3. Token SPL di prova

```sh
cd tests/web3 && npm ci
node devnet-token.mjs http://127.0.0.1:8899 ../../faucet.json <indirizzo Solflare>
```

## 4. Checklist

Annotare per ogni voce: esito, cosa mostra Solflare, eventuali errori.

- [ ] **Aggiunta del nodo**: nelle impostazioni di rete di Solflare, RPC personalizzato
      `https://rpc.example.org`. Solflare lo accetta? Mostra errori di rete?
- [ ] **Saldo**: il wallet prefinanziato mostra 100. Con quale nome e simbolo?
- [ ] **Ricezione**: da un secondo wallet (o con lo script del faucet) inviare CnFA
      al wallet. Il saldo si aggiorna da solo (WebSocket) o serve ricaricare?
- [ ] **Invio**: inviare 1 CnFA a un indirizzo nuovo. Fee mostrata prima della
      conferma? Conferma rapida? La tx compare nello storico?
- [ ] **Token SPL**: il token di prova compare con saldo 1.000? Invio di 10 token a un
      altro indirizzo: riesce? (Un token senza metadati sarà mostrato come sconosciuto.)
- [ ] **Nome mostrato per il token nativo**: ci aspettiamo "SOL" (i wallet non leggono
      il campo `token` di `getVersion`). Annotare esattamente cosa compare.

## 5. Raccolta del log

```sh
cut -d' ' -f2- data/rpc.log | sort | uniq -c | sort -rn   # metodi ricevuti, con esito
grep NON_SUPPORTATO data/rpc.log | cut -d' ' -f2,3 | sort -u
```

Riportare i due output insieme alla checklist. Si aggiungono solo i metodi
`NON_SUPPORTATO` legati a una voce della checklist che non funziona.
