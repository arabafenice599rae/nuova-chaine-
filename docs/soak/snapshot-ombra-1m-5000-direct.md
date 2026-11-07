# soak snapshot da stato ombra 1M

comando: `SNAPSHOT_EVERY=600 RESULTS=docs/soak/snapshot-ombra-1m-5000-direct.md LOAD_DIR=<dir> target/release/examples/soak 1000000 5000 6`, poi `soak --reopen <dir>` (commit 84e59b8)

macchina: 4 vCPU, Intel(R) Xeon(R) Processor @ 2.10GHz; commit 84e59b8; avvio 1793974009425 ms (unix)
1000000 account, 5000 tx/s richiesti, 6 minuti, invio diretto all'ingresso (limite del nodo), rpc_recent_txs 20000, snapshot ogni 600 blocchi, stream_catchup_blocks 36000
CPU in % di un core: nodo = thread dei blocchi; rete = runtime tokio (RPC e stream, lato nodo); client = thread che firmano e inviano (in modalità diretta includono la verifica delle firme all'ingresso)
| min | inviate | accettate | incluse | TPS effettivi | blocco p50 / p95 / p99 ms | invio→inclusione p50 / p95 / p99 ms | CPU nodo / rete / client / altro % | RSS MB | journal MB | stati ok (290 blocchi) / scaduti ok (>300) | wallet |
| 1 | 300450 | 300450 | 300300 | 5005 | 12.1 / 16.4 / 19.9 | 78 / 119 / 121 | 12 / 0 / 29 / 4 | 1096 | 66 | 1 / 1 (errori 0 / 0) |  |
snapshot all'altezza 600: RSS subito prima 958 MB, 1 minuto dopo 1035 MB (+77 MB)
| 2 | 300000 | 300000 | 300000 | 5000 | 11.9 / 16.9 / 20.1 | 89 / 120 / 122 | 12 / 0 / 29 / 4 | 1165 | 132 | 2 / 2 (errori 0 / 0) |  |
snapshot all'altezza 1200: RSS subito prima 990 MB, 1 minuto dopo 1028 MB (+38 MB)
| 3 | 299950 | 299950 | 300000 | 5000 | 12.6 / 17.1 / 22.2 | 78 / 119 / 121 | 12 / 0 / 30 / 4 | 1161 | 197 | 3 / 3 (errori 0 / 0) |  |
snapshot all'altezza 1800: RSS subito prima 1010 MB, 1 minuto dopo 1076 MB (+66 MB)
| 4 | 300050 | 300050 | 300000 | 5000 | 12.6 / 17.1 / 21.1 | 79 / 119 / 121 | 12 / 0 / 29 / 4 | 1204 | 263 | 4 / 4 (errori 0 / 0) |  |
snapshot all'altezza 2400: RSS subito prima 1041 MB, 1 minuto dopo 1062 MB (+21 MB)
| 5 | 300450 | 300450 | 299550 | 4992 | 12.7 / 17.6 / 20.5 | 91 / 120 / 121 | 13 / 0 / 30 / 4 | 1220 | 329 | 5 / 5 (errori 0 / 0) | ok (fee 0.000005 CnFA, reward 0.0000045 CnFA, refund 0 CnFA, net cost 0.0000005 CnFA) |
snapshot all'altezza 3000: RSS subito prima 1062 MB, 1 minuto dopo 1093 MB (+31 MB)
| 6 | 299550 | 299550 | 300451 | 5008 | 12.3 / 16.8 / 20.0 | 78 / 119 / 123 | 12 / 0 / 29 / 4 | 1223 | 395 | 6 / 6 (errori 0 / 0) |  |
totale: inviate 1800450, accettate 1800450, rifiutate all'ingresso 0, incluse 1800451, non incluse 0
TPS effettivi medi 4995 (richiesti 5000); altezza 3607
blocco (tutta la prova): p50 12.3 / p95 17.1 / p99 20.5 / max 120.5 ms
RAM: all'inizio 85 MB, prima del nodo; alla fine 1278 MB; stati in RAM 150150
invio→inclusione (1800 campioni, 1 ogni ~1000 tx, risoluzione ~40 ms): p50 79 / p95 120 / p99 122 / max 239 ms
CPU media (% di un core): nodo 12, rete 0, client 29, altro 4
RSS intorno agli snapshot (altezza: prima -> 1 min dopo): 600: 958 -> 1035, 1200: 990 -> 1028, 1800: 1010 -> 1076, 2400: 1041 -> 1062, 3000: 1062 -> 1093
journal 395 MB = 219 byte/tx, 3944 MB/ora; snapshot 91 MB; status.bin 11.4 MB
stati: 6 trovati e 6 scaduti come atteso (errori 0 / 0); wallet sotto carico: 1 ok, 0 errori
snapshot durante il carico: 6 (pausa sul thread del nodo: codifica degli stati e invio allo stato ombra; copia e scrittura sono in background: 0 ms, 0 ms, 0 ms, 0 ms, 0 ms, 0 ms), massimo 0 ms
snapshot saltati (il precedente era ancora in scrittura): 0
riavvio da snapshot (altezza 3000) + journal: 16.1 s, altezza 3607, hash uguali
riesecuzione completa: stato iniziale 1.3 s, journal 68.4 s (altezza 3607)
  preparazione (fino all'altezza 1): 1 blocchi, 0 tx, 0.0 s, 0.1 ms/blocco
  carico: 3602 blocchi, 1800451 tx, 67.2 s, 18.7 ms/blocco
  tx incluse ma fallite (eseguite con errore, contate rieseguendo il journal): 0
  blocchi vuoti dopo l'inizio del carico (battito, coda): 0 blocchi, 0 tx, 0.0 s, 0.0 ms/blocco
  hash finale (blocco, stato incrementale, stato ricalcolato) uguali al nodo vivo
FINE
