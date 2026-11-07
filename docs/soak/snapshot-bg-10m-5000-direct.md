# soak snapshot-bg 10M

comando: `SNAPSHOT_EVERY=600 RESULTS=docs/soak/snapshot-bg-10m-5000-direct.md LOAD_DIR=<dir> target/release/examples/soak 10000000 5000 6`, poi `soak --reopen <dir>` (commit e32017d)

macchina: 4 vCPU, Intel(R) Xeon(R) Processor @ 2.10GHz; commit e32017d; avvio 1793971231891 ms (unix)
10000000 account, 5000 tx/s richiesti, 6 minuti, invio diretto all'ingresso (limite del nodo), rpc_recent_txs 20000, snapshot ogni 600 blocchi, stream_catchup_blocks 36000
CPU in % di un core: nodo = thread dei blocchi; rete = runtime tokio (RPC e stream, lato nodo); client = thread che firmano e inviano (in modalità diretta includono la verifica delle firme all'ingresso)
| min | inviate | accettate | incluse | TPS effettivi | blocco p50 / p95 / p99 ms | invio→inclusione p50 / p95 / p99 ms | CPU nodo / rete / client / altro % | RSS MB | journal MB | stati ok (290 blocchi) / scaduti ok (>300) | wallet |
| 1 | 309000 | 309000 | 297150 | 4952 | 12.7 / 17.7 / 25.5 | 78 / 109 / 1674 | 16 / 0 / 31 / 1 | 3734 | 68 | 1 / 1 (errori 0 / 0) |  |
| 2 | 291300 | 291300 | 302700 | 5045 | 13.1 / 18.6 / 39.2 | 74 / 119 / 683 | 13 / 0 / 30 / 307445734561825856 | 2819 | 132 | 2 / 2 (errori 0 / 0) |  |
snapshot all'altezza 600: RSS subito prima 2788 MB, 1 minuto dopo 3748 MB (+960 MB)
| 3 | 300150 | 300150 | 300400 | 5007 | 12.8 / 18.3 / 32.6 | 95 / 133 / 1166 | 16 / 0 / 30 / 0 | 2839 | 197 | 3 / 3 (errori 0 / 0) |  |
snapshot all'altezza 1200: RSS subito prima 2819 MB, 1 minuto dopo 3759 MB (+940 MB)
| 4 | 299850 | 299850 | 299600 | 4993 | 12.6 / 17.3 / 35.3 | 89 / 130 / 1204 | 16 / 0 / 29 / 0 | 2853 | 263 | 4 / 4 (errori 0 / 0) |  |
snapshot all'altezza 1800: RSS subito prima 2840 MB, 1 minuto dopo 3776 MB (+936 MB)
| 5 | 300750 | 300750 | 300450 | 5008 | 12.4 / 17.1 / 27.5 | 108 / 141 / 1183 | 15 / 0 / 29 / 0 | 2872 | 329 | 5 / 5 (errori 0 / 0) | ok (fee 0.000005 CnFA, reward 0.0000045 CnFA, refund 0 CnFA, net cost 0.0000005 CnFA) |
snapshot all'altezza 2400: RSS subito prima 2854 MB, 1 minuto dopo 3806 MB (+952 MB)
| 6 | 299400 | 299400 | 299701 | 4995 | 12.4 / 17.7 / 39.3 | 75 / 130 / 950 | 15 / 0 / 29 / 0 | 2895 | 395 | 6 / 6 (errori 0 / 0) |  |
totale: inviate 1800450, accettate 1800450, rifiutate all'ingresso 0, incluse 1800451, non incluse 0
TPS effettivi medi 4996 (richiesti 5000); altezza 3576
blocco (tutta la prova): p50 12.7 / p95 17.7 / p99 33.3 / max 2059.1 ms
RAM: all'inizio 474 MB, prima del nodo; alla fine 2895 MB; stati in RAM 150450
invio→inclusione (1799 campioni, 1 ogni ~1000 tx, risoluzione ~40 ms): p50 85 / p95 139 / p99 1285 / max 2275 ms
CPU media (% di un core): nodo 15, rete 0, client 30, altro 0
RSS intorno agli snapshot (altezza: prima -> 1 min dopo): 600: 2788 -> 3748, 1200: 2819 -> 3759, 1800: 2840 -> 3776, 2400: 2854 -> 3806
journal 395 MB = 219 byte/tx, 3944 MB/ora; snapshot 892 MB; status.bin 11.4 MB
stati: 6 trovati e 6 scaduti come atteso (errori 0 / 0); wallet sotto carico: 1 ok, 0 errori
snapshot durante il carico: 5 (pausa nel blocco: copia della mappa e degli stati; la scrittura è in background: 2047 ms, 1924 ms, 1896 ms, 1826 ms, 1596 ms), massimo 2047 ms
snapshot saltati (il precedente era ancora in scrittura): 0
riavvio da snapshot (altezza 3000) + journal: 30.9 s, altezza 3576, hash uguali
riesecuzione completa: stato iniziale 17.6 s, journal 68.2 s (altezza 3576)
  preparazione (fino all'altezza 1): 1 blocchi, 0 tx, 0.0 s, 0.1 ms/blocco
  carico: 3493 blocchi, 1800451 tx, 67.1 s, 19.2 ms/blocco
  tx incluse ma fallite (eseguite con errore, contate rieseguendo il journal): 0
  blocchi vuoti dopo l'inizio del carico (battito, coda): 0 blocchi, 0 tx, 0.0 s, 0.0 ms/blocco
  hash finale (blocco, stato incrementale, stato ricalcolato) uguali al nodo vivo
FINE
