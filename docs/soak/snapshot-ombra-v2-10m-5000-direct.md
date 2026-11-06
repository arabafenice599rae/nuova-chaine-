# soak snapshot da stato ombra (v2: mappa ordinata, senza copia) 10M

comando: `SNAPSHOT_EVERY=600 RESULTS=docs/soak/snapshot-ombra-v2-10m-5000-direct.md LOAD_DIR=<dir> target/release/examples/soak 10000000 5000 6`, poi `soak --reopen <dir>` (commit 029d946)

macchina: 4 vCPU, Intel(R) Xeon(R) Processor @ 2.10GHz; commit 029d946; avvio 1793977737725 ms (unix)
10000000 account, 5000 tx/s richiesti, 6 minuti, invio diretto all'ingresso (limite del nodo), rpc_recent_txs 20000, snapshot ogni 600 blocchi, stream_catchup_blocks 36000
CPU in % di un core: nodo = thread dei blocchi; rete = runtime tokio (RPC e stream, lato nodo); client = thread che firmano e inviano (in modalità diretta includono la verifica delle firme all'ingresso)
| min | inviate | accettate | incluse | TPS effettivi | blocco p50 / p95 / p99 ms | invio→inclusione p50 / p95 / p99 ms | CPU nodo / rete / client / altro % | RSS MB | journal MB | stati ok (290 blocchi) / scaduti ok (>300) | wallet |
| 1 | 300300 | 300300 | 300000 | 5000 | 13.6 / 28.0 / 40.3 | 66 / 137 / 158 | 14 / 0 / 31 / 30 | 3811 | 66 | 1 / 1 (errori 0 / 0) |  |
snapshot all'altezza 600: RSS subito prima 3800 MB, 1 minuto dopo 3826 MB (+26 MB)
| 2 | 300000 | 300000 | 300000 | 5000 | 13.9 / 19.9 / 24.9 | 66 / 135 / 441 | 14 / 0 / 31 / 21 | 3826 | 132 | 2 / 2 (errori 0 / 0) |  |
snapshot all'altezza 1200: RSS subito prima 3826 MB, 1 minuto dopo 3855 MB (+29 MB)
| 3 | 300000 | 300000 | 300000 | 5000 | 13.5 / 18.7 / 28.6 | 65 / 130 / 271 | 14 / 0 / 30 / 18 | 3856 | 197 | 3 / 3 (errori 0 / 0) |  |
snapshot all'altezza 1800: RSS subito prima 3844 MB, 1 minuto dopo 3876 MB (+32 MB)
| 4 | 300100 | 300100 | 300200 | 5003 | 13.5 / 19.0 / 27.0 | 77 / 119 / 431 | 14 / 0 / 30 / 21 | 3878 | 263 | 4 / 4 (errori 0 / 0) |  |
snapshot all'altezza 2400: RSS subito prima 3876 MB, 1 minuto dopo 3895 MB (+19 MB)
| 5 | 300500 | 300500 | 299850 | 4998 | 13.2 / 18.0 / 25.5 | 64 / 137 / 371 | 13 / 0 / 30 / 18 | 3898 | 329 | 5 / 5 (errori 0 / 0) | ok (fee 0.000005 CnFA, reward 0.0000045 CnFA, refund 0 CnFA, net cost 0.0000005 CnFA) |
snapshot all'altezza 3000: RSS subito prima 3895 MB, 1 minuto dopo 3916 MB (+21 MB)
| 6 | 299500 | 299500 | 300051 | 5001 | 13.8 / 18.6 / 23.8 | 57 / 99 / 139 | 14 / 0 / 31 / 20 | 3919 | 395 | 6 / 6 (errori 0 / 0) |  |
totale: inviate 1800450, accettate 1800450, rifiutate all'ingresso 0, incluse 1800451, non incluse 0
TPS effettivi medi 4996 (richiesti 5000); altezza 3619
blocco (tutta la prova): p50 13.6 / p95 19.4 / p99 34.2 / max 606.5 ms
RAM: all'inizio 475 MB, prima del nodo; alla fine 3919 MB; stati in RAM 150350
invio→inclusione (1800 campioni, 1 ogni ~1000 tx, risoluzione ~40 ms): p50 69 / p95 134 / p99 441 / max 850 ms
CPU media (% di un core): nodo 14, rete 0, client 30, altro 22
RSS intorno agli snapshot (altezza: prima -> 1 min dopo): 600: 3800 -> 3826, 1200: 3826 -> 3855, 1800: 3844 -> 3876, 2400: 3876 -> 3895, 3000: 3895 -> 3916
journal 395 MB = 219 byte/tx, 3944 MB/ora; snapshot 892 MB; status.bin 11.4 MB
stati: 6 trovati e 6 scaduti come atteso (errori 0 / 0); wallet sotto carico: 1 ok, 0 errori
snapshot durante il carico: 6 (pausa sul thread del nodo: codifica degli stati e invio allo stato ombra; copia e scrittura sono in background: 0 ms, 0 ms, 0 ms, 0 ms, 0 ms, 0 ms), massimo 0 ms
blocchi oltre 100 ms: 24 [9: 559 ms = esecuzione 17 + hash 2 + firma 0 + journal 531 + altro 9] [15: 282 ms = esecuzione 101 + hash 12 + firma 1 + journal 113 + altro 55] [701: 607 ms = esecuzione 10 + hash 2 + firma 0 + journal 589 + altro 5] [707: 107 ms = esecuzione 65 + hash 13 + firma 1 + journal 8 + altro 20] [710: 229 ms = esecuzione 8 + hash 2 + firma 0 + journal 215 + altro 4] [712: 150 ms = esecuzione 20 + hash 5 + firma 0 + journal 117 + altro 9] [1280: 171 ms = esecuzione 9 + hash 2 + firma 0 + journal 155 + altro 4] [1282: 589 ms = esecuzione 11 + hash 3 + firma 0 + journal 570 + altro 5] [1288: 147 ms = esecuzione 35 + hash 11 + firma 1 + journal 85 + altro 15] [1291: 195 ms = esecuzione 5 + hash 1 + firma 0 + journal 186 + altro 3] [1293: 189 ms = esecuzione 12 + hash 4 + firma 0 + journal 166 + altro 7] [1898: 294 ms = esecuzione 14 + hash 3 + firma 0 + journal 272 + altro 5] [1901: 271 ms = esecuzione 38 + hash 8 + firma 1 + journal 214 + altro 11] [1904: 171 ms = esecuzione 28 + hash 8 + firma 1 + journal 123 + altro 11] [1909: 222 ms = esecuzione 6 + hash 1 + firma 0 + journal 211 + altro 3] [1911: 181 ms = esecuzione 19 + hash 4 + firma 0 + journal 149 + altro 8] [2480: 206 ms = esecuzione 7 + hash 2 + firma 0 + journal 193 + altro 4] [2482: 564 ms = esecuzione 16 + hash 4 + firma 0 + journal 539 + altro 6] [2488: 108 ms = esecuzione 49 + hash 11 + firma 1 + journal 33 + altro 13] [2492: 221 ms = esecuzione 7 + hash 2 + firma 0 + journal 208 + altro 4] [2494: 167 ms = esecuzione 13 + hash 4 + firma 0 + journal 141 + altro 9] [3093: 587 ms = esecuzione 7 + hash 2 + firma 0 + journal 574 + altro 5] [3102: 216 ms = esecuzione 8 + hash 2 + firma 0 + journal 203 + altro 4] [3104: 203 ms = esecuzione 18 + hash 5 + firma 0 + journal 171 + altro 9]
snapshot saltati (scrittura in corso, coda piena o ombra persa): 0; ultima scrittura 10488 ms; coda massima durante una scrittura 100 blocchi
riavvio da snapshot (altezza 3000) + journal: 39.1 s, altezza 3619, hash uguali
riesecuzione completa: stato iniziale 19.2 s, journal 71.4 s (altezza 3619)
  preparazione (fino all'altezza 1): 1 blocchi, 0 tx, 0.0 s, 0.2 ms/blocco
  carico: 3570 blocchi, 1800451 tx, 70.1 s, 19.6 ms/blocco
  tx incluse ma fallite (eseguite con errore, contate rieseguendo il journal): 0
  blocchi vuoti dopo l'inizio del carico (battito, coda): 0 blocchi, 0 tx, 0.0 s, 0.0 ms/blocco
  hash finale (blocco, stato incrementale, stato ricalcolato) uguali al nodo vivo
FINE
