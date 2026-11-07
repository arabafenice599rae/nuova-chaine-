# soak snapshot-bg 1M

comando: `SNAPSHOT_EVERY=600 RESULTS=docs/soak/snapshot-bg-1m-5000-direct.md LOAD_DIR=<dir> target/release/examples/soak 1000000 5000 6`, poi `soak --reopen <dir>` (commit e32017d)

macchina: 4 vCPU, Intel(R) Xeon(R) Processor @ 2.10GHz; commit e32017d; avvio 1793970790809 ms (unix)
1000000 account, 5000 tx/s richiesti, 6 minuti, invio diretto all'ingresso (limite del nodo), rpc_recent_txs 20000, snapshot ogni 600 blocchi, stream_catchup_blocks 36000
CPU in % di un core: nodo = thread dei blocchi; rete = runtime tokio (RPC e stream, lato nodo); client = thread che firmano e inviano (in modalità diretta includono la verifica delle firme all'ingresso)
| min | inviate | accettate | incluse | TPS effettivi | blocco p50 / p95 / p99 ms | invio→inclusione p50 / p95 / p99 ms | CPU nodo / rete / client / altro % | RSS MB | journal MB | stati ok (290 blocchi) / scaduti ok (>300) | wallet |
| 1 | 300300 | 300300 | 300150 | 5002 | 12.0 / 17.0 / 24.1 | 78 / 119 / 120 | 13 / 0 / 29 / 0 | 829 | 66 | 1 / 1 (errori 0 / 0) |  |
snapshot all'altezza 600: RSS subito prima 721 MB, 1 minuto dopo 838 MB (+117 MB)
| 2 | 300000 | 300000 | 300000 | 5000 | 12.1 / 16.2 / 19.7 | 71 / 102 / 106 | 12 / 0 / 29 / 1 | 838 | 132 | 2 / 2 (errori 0 / 0) |  |
snapshot all'altezza 1200: RSS subito prima 743 MB, 1 minuto dopo 867 MB (+124 MB)
| 3 | 300050 | 300050 | 300050 | 5001 | 12.8 / 16.7 / 19.3 | 74 / 111 / 113 | 13 / 0 / 30 / 0 | 867 | 197 | 3 / 3 (errori 0 / 0) |  |
snapshot all'altezza 1800: RSS subito prima 761 MB, 1 minuto dopo 886 MB (+125 MB)
| 4 | 300000 | 300000 | 299850 | 4998 | 12.6 / 17.6 / 22.6 | 75 / 116 / 118 | 13 / 0 / 30 / 0 | 886 | 263 | 4 / 4 (errori 0 / 0) |  |
snapshot all'altezza 2400: RSS subito prima 781 MB, 1 minuto dopo 899 MB (+118 MB)
| 5 | 300550 | 300550 | 299900 | 4998 | 12.4 / 16.9 / 20.4 | 62 / 135 / 139 | 13 / 0 / 30 / 0 | 807 | 329 | 5 / 5 (errori 0 / 0) | ok (fee 0.000005 CnFA, reward 0.0000045 CnFA, refund 0 CnFA, net cost 0.0000005 CnFA) |
snapshot all'altezza 3000: RSS subito prima 799 MB, 1 minuto dopo 923 MB (+124 MB)
| 6 | 299550 | 299550 | 300401 | 5007 | 12.3 / 16.8 / 24.8 | 51 / 122 / 125 | 13 / 0 / 30 / 307445734561825856 | 831 | 395 | 6 / 6 (errori 0 / 0) |  |
totale: inviate 1800450, accettate 1800450, rifiutate all'ingresso 0, incluse 1800451, non incluse 0
TPS effettivi medi 4995 (richiesti 5000); altezza 3614
blocco (tutta la prova): p50 12.4 / p95 16.9 / p99 22.1 / max 458.7 ms
RAM: all'inizio 85 MB, prima del nodo; alla fine 831 MB; stati in RAM 149500
invio→inclusione (1800 campioni, 1 ogni ~1000 tx, risoluzione ~40 ms): p50 69 / p95 119 / p99 139 / max 600 ms
CPU media (% di un core): nodo 13, rete 0, client 30, altro 0
RSS intorno agli snapshot (altezza: prima -> 1 min dopo): 600: 721 -> 838, 1200: 743 -> 867, 1800: 761 -> 886, 2400: 781 -> 899, 3000: 799 -> 923
journal 395 MB = 219 byte/tx, 3944 MB/ora; snapshot 91 MB; status.bin 11.4 MB
stati: 6 trovati e 6 scaduti come atteso (errori 0 / 0); wallet sotto carico: 1 ok, 0 errori
snapshot durante il carico: 6 (pausa nel blocco: copia della mappa e degli stati; la scrittura è in background: 449 ms, 297 ms, 393 ms, 339 ms, 364 ms, 367 ms), massimo 449 ms
snapshot saltati (il precedente era ancora in scrittura): 0
riavvio da snapshot (altezza 3600) + journal: 4.1 s, altezza 3614, hash uguali
riesecuzione completa: stato iniziale 1.3 s, journal 67.7 s (altezza 3614)
  preparazione (fino all'altezza 1): 1 blocchi, 0 tx, 0.0 s, 0.1 ms/blocco
  carico: 3591 blocchi, 1800451 tx, 66.6 s, 18.5 ms/blocco
  tx incluse ma fallite (eseguite con errore, contate rieseguendo il journal): 0
  blocchi vuoti dopo l'inizio del carico (battito, coda): 0 blocchi, 0 tx, 0.0 s, 0.0 ms/blocco
  hash finale (blocco, stato incrementale, stato ricalcolato) uguali al nodo vivo
FINE
