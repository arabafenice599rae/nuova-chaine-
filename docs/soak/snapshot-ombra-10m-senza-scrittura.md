# soak stato ombra attivo senza scrittura, 10M

comando: `SNAPSHOT_EVERY=1000000 RESULTS=docs/soak/snapshot-ombra-10m-senza-scrittura.md LOAD_DIR=<dir> target/release/examples/soak 10000000 5000 6` (commit 9811e94): ombra attiva, nessuno snapshot durante il carico

macchina: 4 vCPU, Intel(R) Xeon(R) Processor @ 2.10GHz; commit 9811e94; avvio 1793976186037 ms (unix)
10000000 account, 5000 tx/s richiesti, 6 minuti, invio diretto all'ingresso (limite del nodo), rpc_recent_txs 20000, snapshot ogni 1000000 blocchi, stream_catchup_blocks 36000
CPU in % di un core: nodo = thread dei blocchi; rete = runtime tokio (RPC e stream, lato nodo); client = thread che firmano e inviano (in modalità diretta includono la verifica delle firme all'ingresso)
| min | inviate | accettate | incluse | TPS effettivi | blocco p50 / p95 / p99 ms | invio→inclusione p50 / p95 / p99 ms | CPU nodo / rete / client / altro % | RSS MB | journal MB | stati ok (290 blocchi) / scaduti ok (>300) | wallet |
| 1 | 300600 | 300600 | 300200 | 5003 | 14.8 / 27.2 / 35.6 | 79 / 133 / 236 | 16 / 0 / 31 / 16 | 4477 | 66 | 1 / 1 (errori 0 / 0) |  |
| 2 | 299900 | 299900 | 300000 | 5000 | 13.7 / 18.4 / 21.1 | 77 / 132 / 134 | 14 / 0 / 31 / 3 | 4496 | 132 | 2 / 2 (errori 0 / 0) |  |
| 3 | 299950 | 299950 | 300000 | 5000 | 13.5 / 18.4 / 33.2 | 80 / 132 / 137 | 14 / 0 / 31 / 4 | 4562 | 197 | 3 / 3 (errori 0 / 0) |  |
| 4 | 300000 | 300000 | 300050 | 5001 | 13.9 / 20.1 / 28.6 | 81 / 136 / 138 | 14 / 0 / 30 / 3 | 4581 | 263 | 4 / 4 (errori 0 / 0) |  |
| 5 | 300750 | 300750 | 299950 | 4999 | 15.7 / 35.6 / 68.0 | 87 / 137 / 179 | 18 / 0 / 33 / 4 | 4602 | 329 | 5 / 5 (errori 0 / 0) | ok (fee 0.000005 CnFA, reward 0.0000045 CnFA, refund 0 CnFA, net cost 0.0000005 CnFA) |
| 6 | 299250 | 299250 | 300001 | 5000 | 15.4 / 22.2 / 28.3 | 83 / 136 / 138 | 16 / 0 / 31 / 4 | 4623 | 395 | 6 / 6 (errori 0 / 0) |  |
totale: inviate 1800450, accettate 1800450, rifiutate all'ingresso 0, incluse 1800451, non incluse 0
TPS effettivi medi 4996 (richiesti 5000); altezza 3609
blocco (tutta la prova): p50 14.5 / p95 23.0 / p99 38.9 / max 359.1 ms
RAM: all'inizio 473 MB, prima del nodo; alla fine 4623 MB; stati in RAM 150250
invio→inclusione (1800 campioni, 1 ogni ~1000 tx, risoluzione ~40 ms): p50 81 / p95 135 / p99 139 / max 392 ms
CPU media (% di un core): nodo 15, rete 0, client 31, altro 6
RSS intorno agli snapshot (altezza: prima -> 1 min dopo): 
journal 395 MB = 219 byte/tx, 3944 MB/ora; snapshot 892 MB; status.bin 0.0 MB
stati: 6 trovati e 6 scaduti come atteso (errori 0 / 0); wallet sotto carico: 1 ok, 0 errori
snapshot durante il carico: 0 (pausa sul thread del nodo: codifica degli stati e invio allo stato ombra; copia e scrittura sono in background: ), massimo 0 ms
snapshot saltati (il precedente era ancora in scrittura): 0
FINE
