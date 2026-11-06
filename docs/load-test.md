# Test di carico

`examples/load.rs`: N account di sistema, blocchi da 5.000 trasferimenti reali (firmati,
verificati dall'ingresso, eseguiti da litesvm, scritti nel journal). Con `--rpc` misura
anche il percorso completo: nodo reale (`node::start`) e 16 client HTTP che inviano
`sendTransaction`.

```sh
cargo build --release --example load
LOAD_DIR=/tmp/load target/release/examples/load 10000000 20 5000 --rpc
```

Macchina: 4 vCPU, 15 GB di RAM, disco virtuale. Client e nodo sulla stessa macchina.
20 blocchi da 5.000 tx per misura; 2.000 mittenti, destinatari casuali tra gli N account.

## Risultati (hash di stato LtHash incrementale)

| account | ms per blocco (5.000 tx) | TPS esecuzione | TPS verifica firme + esecuzione | TPS via RPC (16 client) | RAM RSS (picco) | snapshot | scrittura snapshot | riavvio |
|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| 100.000 | 62–65 | 77–81 k | 32–34 k | 10,8 k | 226 MB (239) | 10 MB | < 0,1 s | 0,1–0,2 s |
| 1.000.000 | 69 | 72 k | 31 k | 11,3 k | 475 MB (496) | 91 MB | 0,6 s | 1,2 s |
| 10.000.000 | 73–75 | 67–68 k | 30 k | 11,7 k | 2.226 MB (3.766) | 892 MB | 7,4 s | 13 s |

- **ms per blocco**: `Node::tick` (esecuzione, settlement delle fee, LtHash, journal).
  Media; il p50 è 61–69 ms e il massimo 104–134 ms.
- **verifica firme**: batch ed25519 dell'ingresso, circa 90 ms ogni 5.000 tx (18 µs per
  firma, un core).
- **RPC**: tx tutte incluse subito dopo l'ultimo invio. Con preflight (il default dei
  wallet: simulazione sul thread del nodo) il campione da 2.000 tx va a 6,8–11,4 k/s.
- **riavvio**: lettura dello snapshot, ricostruzione di litesvm e LtHash completo
  (verifica). Il secondo valore è dopo i 20 blocchi.
- Il picco di RAM a 10 M arriva durante la preparazione (stato del genesis più lo
  snapshot ordinato in memoria), non durante il servizio.

### Prima: SHA-256 completo a ogni blocco

| account | ms per blocco (5.000 tx) |
|---:|---:|
| 100.000 | 69 |
| 1.000.000 | 510 (più dello slot da 100 ms) |

Con LtHash il tempo per blocco non dipende più dal numero totale di account: tra 100 mila
e 10 milioni di account passa da 62 a 75 ms, con 5.000 tx per blocco.

## Primo limite raggiunto

1. **Throughput dai client: circa 11 k TPS via RPC**, contro i 30 k della pipeline
   (verifica in batch più esecuzione) e i 50 k del limite di configurazione (5.000 tx ogni
   100 ms). `sendTransaction` viene eseguito sul thread del nodo, una tx per richiesta:
   decodifica, verifica di una sola firma (senza batch) e accodamento. È il primo limite,
   con qualsiasi numero di account.
2. **Snapshot sincrono sul thread del nodo**: 0,6 s a 1 M account, 7,4 s a 10 M. Ogni
   `snapshot_every` blocchi (3.000, cioè 5 minuti) la produzione si ferma per quel tempo
   (74 slot a 10 M, oltre il battito di 10 slot). È il primo limite che dipende dal
   numero di account.
3. RAM e riavvio non sono un limite: 2,2 GB e 13 s a 10 M account.

Nessuno dei due limiti richiede un database o stato su disco: sono lavoro sul thread
del nodo. Possibili interventi, non implementati e da decidere:

- verifica e decodifica di `sendTransaction` fuori dal thread del nodo (l'ingresso è già
  dietro un mutex), con firme verificate in batch;
- snapshot scritto da un altro thread a partire da una copia dello stato (i dati degli
  account sono condivisi con `Arc`, quindi la copia costa solo la mappa).
