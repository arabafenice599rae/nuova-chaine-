# soak snapshot da stato ombra (v2: mappa ordinata, senza copia) 1M

comando: `SNAPSHOT_EVERY=600 RESULTS=docs/soak/snapshot-ombra-v2-1m-5000-direct.md LOAD_DIR=<dir> target/release/examples/soak 1000000 5000 6`, poi `soak --reopen <dir>` (commit 029d946)

macchina: 4 vCPU, Intel(R) Xeon(R) Processor @ 2.10GHz; commit 029d946; avvio 1793977294916 ms (unix)
1000000 account, 5000 tx/s richiesti, 6 minuti, invio diretto all'ingresso (limite del nodo), rpc_recent_txs 20000, snapshot ogni 600 blocchi, stream_catchup_blocks 36000
CPU in % di un core: nodo = thread dei blocchi; rete = runtime tokio (RPC e stream, lato nodo); client = thread che firmano e inviano (in modalità diretta includono la verifica delle firme all'ingresso)
| min | inviate | accettate | incluse | TPS effettivi | blocco p50 / p95 / p99 ms | invio→inclusione p50 / p95 / p99 ms | CPU nodo / rete / client / altro % | RSS MB | journal MB | stati ok (290 blocchi) / scaduti ok (>300) | wallet |
| 1 | 300300 | 300300 | 299850 | 4998 | 12.4 / 17.9 / 24.2 | 79 / 119 / 130 | 12 / 0 / 30 / 5 | 874 | 66 | 1 / 1 (errori 0 / 0) |  |
snapshot all'altezza 600: RSS subito prima 862 MB, 1 minuto dopo 893 MB (+31 MB)
| 2 | 300000 | 300000 | 300000 | 5000 | 13.4 / 19.0 / 25.2 | 93 / 120 / 139 | 13 / 0 / 31 / 5 | 894 | 132 | 2 / 2 (errori 0 / 0) |  |
snapshot all'altezza 1200: RSS subito prima 893 MB, 1 minuto dopo 925 MB (+32 MB)
| 3 | 300100 | 300100 | 300450 | 5008 | 13.4 / 18.3 / 22.2 | 80 / 120 / 122 | 13 / 0 / 30 / 5 | 926 | 197 | 3 / 3 (errori 0 / 0) |  |
snapshot all'altezza 1800: RSS subito prima 925 MB, 1 minuto dopo 946 MB (+21 MB)
| 4 | 300050 | 300050 | 300000 | 5000 | 13.4 / 18.4 / 22.6 | 84 / 120 / 122 | 13 / 0 / 30 / 5 | 947 | 263 | 4 / 4 (errori 0 / 0) |  |
snapshot all'altezza 2400: RSS subito prima 946 MB, 1 minuto dopo 967 MB (+21 MB)
| 5 | 300800 | 300800 | 300000 | 5000 | 13.7 / 18.6 / 20.4 | 80 / 120 / 122 | 14 / 0 / 31 / 5 | 968 | 329 | 5 / 5 (errori 0 / 0) | ok (fee 0.000005 CnFA, reward 0.0000045 CnFA, refund 0 CnFA, net cost 0.0000005 CnFA) |
snapshot all'altezza 3000: RSS subito prima 967 MB, 1 minuto dopo 988 MB (+21 MB)
| 6 | 299200 | 299200 | 300001 | 5000 | 13.9 / 19.1 / 23.0 | 79 / 120 / 124 | 14 / 0 / 31 / 5 | 989 | 395 | 6 / 6 (errori 0 / 0) |  |
totale: inviate 1800450, accettate 1800450, rifiutate all'ingresso 0, incluse 1800451, non incluse 0
TPS effettivi medi 4995 (richiesti 5000); altezza 3607
blocco (tutta la prova): p50 13.4 / p95 18.6 / p99 23.6 / max 110.6 ms
RAM: all'inizio 85 MB, prima del nodo; alla fine 989 MB; stati in RAM 150150
invio→inclusione (1800 campioni, 1 ogni ~1000 tx, risoluzione ~40 ms): p50 80 / p95 120 / p99 132 / max 152 ms
CPU media (% di un core): nodo 13, rete 0, client 31, altro 5
RSS intorno agli snapshot (altezza: prima -> 1 min dopo): 600: 862 -> 893, 1200: 893 -> 925, 1800: 925 -> 946, 2400: 946 -> 967, 3000: 967 -> 988
journal 395 MB = 219 byte/tx, 3944 MB/ora; snapshot 91 MB; status.bin 11.4 MB
stati: 6 trovati e 6 scaduti come atteso (errori 0 / 0); wallet sotto carico: 1 ok, 0 errori
snapshot durante il carico: 6 (pausa sul thread del nodo: codifica degli stati e invio allo stato ombra; copia e scrittura sono in background: 0 ms, 0 ms, 0 ms, 0 ms, 0 ms, 0 ms), massimo 0 ms
blocchi oltre 100 ms: 1 [1208: 111 ms = esecuzione 8 + hash 2 + firma 0 + journal 96 + altro 5]
snapshot saltati (scrittura in corso, coda piena o ombra persa): 0; ultima scrittura 911 ms; coda massima durante una scrittura 7 blocchi
riavvio da snapshot (altezza 3600) + journal: 4.4 s, altezza 3607, hash uguali
riesecuzione completa: stato iniziale 1.3 s, journal 69.5 s (altezza 3607)
  preparazione (fino all'altezza 1): 1 blocchi, 0 tx, 0.0 s, 0.1 ms/blocco
  carico: 3602 blocchi, 1800451 tx, 68.2 s, 18.9 ms/blocco
  tx incluse ma fallite (eseguite con errore, contate rieseguendo il journal): 0
  blocchi vuoti dopo l'inizio del carico (battito, coda): 0 blocchi, 0 tx, 0.0 s, 0.0 ms/blocco
  hash finale (blocco, stato incrementale, stato ricalcolato) uguali al nodo vivo
FINE
