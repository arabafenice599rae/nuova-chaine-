# Nodo devnet su un VPS

Come installare un nodo centrifuga devnet con endpoint HTTPS pubblico e conti
prefinanziati, e come misurare la latenza da una rete esterna.

## Requisiti minimi del VPS

| | minimo | note |
|---|---|---|
| Sistema | Debian 12, Ubuntu 22.04 o 24.04, x86_64 | servono systemd e apt |
| CPU | 2 vCPU | la compilazione dura 10–20 minuti |
| RAM | 2 GB | sotto 3,5 GB lo script crea 4 GB di swap per la compilazione; il nodo poi usa poche centinaia di MB |
| Disco | 20 GB SSD | sorgenti e compilazione ~6 GB; il journal cresce di ~130 MB al giorno (blocchi vuoti ogni 100 ms) |
| Rete | IPv4 pubblico, porte 80 e 443 aperte in ingresso | la 80 serve a Let's Encrypt |
| DNS | record A del dominio (es. `rpc.example.org`) verso l'IP del VPS, già propagato | |

Sul tuo computer: Node.js 22 e una copia del repository, per la misura di latenza e
per lo script del token di prova.

## 1. Sul tuo computer: chiave per la misura di latenza

```sh
git clone https://github.com/arabafenice599rae/nuova-chaine-.git centrifuga
cd centrifuga/tests/web3 && npm ci
node latency.mjs https://rpc.example.org bench.json
```

Il comando crea `bench.json` (resta sul tuo computer, permessi 600) e stampa la
pubkey da prefinanziare.

## 2. Sul VPS: installazione con un solo comando

```sh
ssh root@<ip-del-vps>
git clone https://github.com/arabafenice599rae/nuova-chaine-.git /opt/centrifuga
cd /opt/centrifuga
deploy/devnet-install.sh --domain rpc.example.org --email tu@example.org \
  --prefund <tuo indirizzo>:100 --prefund <pubkey di bench.json>:10
```

(Con un utente non root: `sudo deploy/devnet-install.sh ...`.)

Lo script, in ordine:
1. installa i pacchetti di sistema e Caddy (repository ufficiale), crea l'utente
   `centrifuga` e, se la RAM è poca, lo swap;
2. installa Rust alla versione di `rust-toolchain.toml` e compila il nodo;
3. genera **sul server** la chiave dell'operatore e quella del faucet in
   `/var/lib/centrifuga/` (permessi 600, proprietario `centrifuga`). Le chiavi non
   vengono mai stampate: lo script mostra solo le pubkey;
4. scrive `/etc/centrifuga/centrifuga.toml`, con `network = "devnet"`, i conti
   prefinanziati (più 10 CnFA al faucet), `rpc_log = true` e RPC e stream in
   ascolto solo su `127.0.0.1`;
5. verifica il genesis (conservazione e determinismo);
6. installa e avvia i servizi systemd `centrifuga` (nodo) e `centrifuga-indexer`
   (storico: legge il journal del nodo; il nodo con `indexer_rpc` gli inoltra
   `getSignaturesForAddress` e le tx vecchie, vedi docs/history.md);
7. configura Caddy: HTTPS con Let's Encrypt; WebSocket sulla stessa porta (upgrade
   instradati al nodo); stream su `/stream`; l'IP reale passa in `X-Forwarded-For`;
8. controlla che `https://rpc.example.org/health` risponda.

Rieseguire lo script (es. dopo `git pull`) ricompila e riavvia, senza toccare
chiavi, config e dati. Per cambiare i conti prefinanziati serve rifare il genesis:
`--reset` cancella config e dati della chain (non le chiavi).

Comandi utili sul VPS:

```sh
systemctl status centrifuga caddy
journalctl -u centrifuga -f                      # blocchi con transazioni, errori
journalctl -u centrifuga-indexer -f              # storico: altezza raggiunta, errori
centrifuga balance <tuo indirizzo>               # "100 CnFA"
```

## 3. Latenza dalla tua rete

```sh
cd centrifuga/tests/web3
node latency.mjs https://rpc.example.org bench.json      # 200 tx, ~1 minuto
```

Output: media, p50, p95 e p99 di quattro misure:
- **RTT HTTP**: `getHealth`, solo rete (TLS, Caddy, nodo che risponde subito);
- **totale**: da `sendTransaction` (con preflight) alla notifica `signatureSubscribe`
  del blocco firmato e scritto;
- **nodo**: dall'arrivo della richiesta al nodo al blocco firmato e scritto
  (misurato dal nodo, campo `centrifugaNodeLatencyUs` di `getTransaction`);
- **rete**: totale − nodo (andata della richiesta + ritorno della notifica).

Il tempo nel nodo è in media metà slot (~50 ms con slot da 100 ms). La parte di rete
è quella su cui QUIC potrebbe incidere: confrontarla con l'RTT.

## 4. Token SPL di prova

Sul VPS, con la chiave del faucet generata dallo script:

```sh
cd /opt/centrifuga/tests/web3 && npm ci
sudo -u centrifuga node devnet-token.mjs http://127.0.0.1:8899 /var/lib/centrifuga/faucet.json <tuo indirizzo>
```

(Node.js sul VPS: `apt-get install -y nodejs npm`, oppure lanciare lo script dal tuo
computer con una chiave prefinanziata anche lì.)

## 5. Metodi RPC ricevuti

Sul VPS:

```sh
cut -d' ' -f2- /var/lib/centrifuga/data/rpc.log | sort | uniq -c | sort -rn   # metodi ricevuti, con esito
grep UNSUPPORTED /var/lib/centrifuga/data/rpc.log | cut -d' ' -f2,3 | sort -u
```

## Disco: snapshot e journal

Il journal fa un `sync_data` a ogni blocco; lo snapshot scrive in background ~100 byte
per account (1 GB a 10 milioni). Perché il `sync_data` del journal non aspetti le
scritture dello snapshot:
- lo snapshot scrive a blocchi da 8 MB e li manda su disco man mano (`sync_file_range`):
  non restano mai più di ~16 MB in sospeso;
- `snapshot_mb_s` (default 100) limita la banda della scrittura;
- sul sistema, limiti bassi alle pagine sporche in attesa:

```sh
sudo sysctl -w vm.dirty_background_bytes=16777216   # 16 MB: la scrittura parte subito
sudo sysctl -w vm.dirty_bytes=67108864              # 64 MB: oltre, chi scrive aspetta
# permanenti: le stesse due righe (senza sudo sysctl -w) in /etc/sysctl.d/90-centrifuga.conf
```

Meglio ancora: snapshot e journal su dischi separati, così le scritture dello snapshot
non passano dalla stessa coda del disco del journal.
