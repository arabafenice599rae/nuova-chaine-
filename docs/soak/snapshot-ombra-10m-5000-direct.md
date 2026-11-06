# soak snapshot da stato ombra 10M

comando: `SNAPSHOT_EVERY=600 RESULTS=docs/soak/snapshot-ombra-10m-5000-direct.md LOAD_DIR=<dir> target/release/examples/soak 10000000 5000 6`, poi `soak --reopen <dir>` (commit 84e59b8)

macchina: 4 vCPU, Intel(R) Xeon(R) Processor @ 2.10GHz; commit 84e59b8; avvio 1793974462742 ms (unix)
10000000 account, 5000 tx/s richiesti, 6 minuti, invio diretto all'ingresso (limite del nodo), rpc_recent_txs 20000, snapshot ogni 600 blocchi, stream_catchup_blocks 36000
CPU in % di un core: nodo = thread dei blocchi; rete = runtime tokio (RPC e stream, lato nodo); client = thread che firmano e inviano (in modalità diretta includono la verifica delle firme all'ingresso)
| min | inviate | accettate | incluse | TPS effettivi | blocco p50 / p95 / p99 ms | invio→inclusione p50 / p95 / p99 ms | CPU nodo / rete / client / altro % | RSS MB | journal MB | stati ok (290 blocchi) / scaduti ok (>300) | wallet |
| 1 | 300300 | 300300 | 300150 | 5002 | 14.4 / 21.8 / 29.9 | 65 / 104 / 194 | 15 / 0 / 30 / 17 | 4542 | 66 | 1 / 1 (errori 0 / 0) |  |
snapshot all'altezza 600: RSS subito prima 4367 MB, 1 minuto dopo 4505 MB (+138 MB)
| 2 | 300150 | 300150 | 300150 | 5002 | 13.1 / 18.0 / 56.8 | 86 / 130 / 160 | 13 / 0 / 29 / 15 | 4936 | 132 | 2 / 2 (errori 0 / 0) |  |
snapshot all'altezza 1200: RSS subito prima 4398 MB, 1 minuto dopo 4680 MB (+282 MB)
| 3 | 299950 | 299950 | 299800 | 4997 | 13.0 / 17.8 / 52.9 | 76 / 130 / 173 | 13 / 0 / 29 / 13 | 4869 | 197 | 3 / 3 (errori 0 / 0) |  |
snapshot all'altezza 1800: RSS subito prima 4418 MB, 1 minuto dopo 4767 MB (+349 MB)
| 4 | 300050 | 300050 | 300150 | 5002 | 13.3 / 17.9 / 33.2 | 82 / 128 / 388 | 13 / 0 / 30 / 15 | 5233 | 263 | 4 / 4 (errori 0 / 0) |  |
snapshot all'altezza 2400: RSS subito prima 4448 MB, 1 minuto dopo 4759 MB (+311 MB)
| 5 | 300600 | 300600 | 300100 | 5002 | 13.2 / 18.7 / 28.3 | 92 / 130 / 413 | 13 / 0 / 30 / 12 | 5231 | 329 | 5 / 5 (errori 0 / 0) | ok (fee 0.000005 CnFA, reward 0.0000045 CnFA, refund 0 CnFA, net cost 0.0000005 CnFA) |
snapshot all'altezza 3000: RSS subito prima 4469 MB, 1 minuto dopo 4657 MB (+188 MB)
| 6 | 299250 | 299250 | 299901 | 4998 | 13.5 / 19.5 / 69.6 | 90 / 127 / 189 | 14 / 0 / 30 / 12 | 5429 | 395 | 6 / 6 (errori 0 / 0) |  |
totale: inviate 1800300, accettate 1800300, rifiutate all'ingresso 0, incluse 1800301, non incluse 0
TPS effettivi medi 4995 (richiesti 5000); altezza 3619
blocco (tutta la prova): p50 13.4 / p95 19.4 / p99 37.5 / max 671.0 ms
RAM: all'inizio 474 MB, prima del nodo; alla fine 5507 MB; stati in RAM 150100
invio→inclusione (1800 campioni, 1 ogni ~1000 tx, risoluzione ~40 ms): p50 82 / p95 129 / p99 362 / max 760 ms
CPU media (% di un core): nodo 14, rete 0, client 30, altro 14
RSS intorno agli snapshot (altezza: prima -> 1 min dopo): 600: 4367 -> 4505, 1200: 4398 -> 4680, 1800: 4418 -> 4767, 2400: 4448 -> 4759, 3000: 4469 -> 4657
journal 395 MB = 219 byte/tx, 3944 MB/ora; snapshot 892 MB; status.bin 11.4 MB
stati: 6 trovati e 6 scaduti come atteso (errori 0 / 0); wallet sotto carico: 1 ok, 0 errori
snapshot durante il carico: 6 (pausa sul thread del nodo: codifica degli stati e invio allo stato ombra; copia e scrittura sono in background: 0 ms, 0 ms, 0 ms, 0 ms, 0 ms, 0 ms), massimo 0 ms
snapshot saltati (il precedente era ancora in scrittura): 0
riavvio da snapshot (altezza 3000) + journal: 35.7 s, altezza 3619, hash uguali
riesecuzione completa: stato iniziale 18.4 s, journal 68.5 s (altezza 3619)
  preparazione (fino all'altezza 1): 1 blocchi, 0 tx, 0.0 s, 0.1 ms/blocco
  carico: 3576 blocchi, 1800301 tx, 67.3 s, 18.8 ms/blocco
  tx incluse ma fallite (eseguite con errore, contate rieseguendo il journal): 0
  blocchi vuoti dopo l'inizio del carico (battito, coda): 0 blocchi, 0 tx, 0.0 s, 0.0 ms/blocco
  hash finale (blocco, stato incrementale, stato ricalcolato) uguali al nodo vivo
FINE
