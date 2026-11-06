# soak final-1m-5000-direct-10min

comando: `scripts/soak-run.sh final-1m-5000-direct-10min 1000000 5000 10` (commit 90c21e5)

macchina: 4 vCPU, Intel(R) Xeon(R) Processor @ 2.10GHz; commit 90c21e5; avvio 1793957210885 ms (unix)
1000000 account, 5000 tx/s richiesti, 10 minuti, invio diretto all'ingresso (limite del nodo), rpc_recent_txs 20000, snapshot ogni 3000 blocchi, stream_catchup_blocks 36000
CPU in % di un core: nodo = thread dei blocchi; rete = runtime tokio (RPC e stream, lato nodo); client = thread che firmano e inviano (in modalità diretta includono la verifica delle firme all'ingresso)
| min | inviate | accettate | incluse | TPS effettivi | blocco p50 / p95 / p99 ms | invio→inclusione p50 / p95 / p99 ms | CPU nodo / rete / client / altro % | RSS MB | journal MB | stati ok (290 blocchi) / scaduti ok (>300) | wallet |
| 1 | 300350 | 300350 | 300150 | 5002 | 12.7 / 16.9 / 27.6 | 67 / 125 / 126 | 13 / 0 / 33 / 0 | 719 | 66 | 1 / 1 (errori 0 / 0) |  |
| 2 | 300000 | 300000 | 300000 | 5000 | 12.7 / 16.3 / 17.4 | 68 / 125 / 126 | 13 / 0 / 32 / 0 | 738 | 132 | 2 / 2 (errori 0 / 0) |  |
| 3 | 300100 | 300100 | 300000 | 5000 | 12.2 / 16.3 / 18.1 | 67 / 125 / 126 | 12 / 0 / 32 / 0 | 756 | 197 | 3 / 3 (errori 0 / 0) |  |
| 4 | 300000 | 300000 | 300000 | 5000 | 11.7 / 15.6 / 17.4 | 68 / 124 / 126 | 12 / 0 / 30 / 0 | 775 | 263 | 4 / 4 (errori 0 / 0) |  |
| 5 | 301750 | 301750 | 297000 | 4950 | 11.8 / 16.0 / 17.4 | 69 / 126 / 371 | 13 / 0 / 30 / 0 | 811 | 329 | 5 / 5 (errori 0 / 0) | ok (fee 0.000005 CnFA, reward 0.0000045 CnFA, refund 0 CnFA, net cost 0.0000005 CnFA) |
| 6 | 298250 | 298250 | 303001 | 5050 | 11.7 / 16.2 / 20.1 | 65 / 121 / 123 | 12 / 0 / 30 / 0 | 815 | 395 | 6 / 6 (errori 0 / 0) |  |
snapshot all'altezza 3000: RSS subito prima 793 MB, 1 minuto dopo 815 MB (+22 MB)
| 7 | 300050 | 300050 | 300000 | 5000 | 12.0 / 16.5 / 20.6 | 73 / 122 / 123 | 12 / 0 / 31 / 0 | 832 | 461 | 7 / 7 (errori 0 / 0) |  |
| 8 | 300050 | 300050 | 300000 | 5000 | 12.3 / 16.9 / 20.6 | 64 / 122 / 123 | 13 / 0 / 32 / 0 | 833 | 526 | 8 / 8 (errori 0 / 0) |  |
| 9 | 299900 | 299900 | 300000 | 5000 | 12.6 / 17.1 / 19.6 | 64 / 121 / 123 | 13 / 0 / 31 / 0 | 833 | 592 | 9 / 9 (errori 0 / 0) |  |
| 10 | 302000 | 302000 | 296500 | 4942 | 12.7 / 18.0 / 21.7 | 64 / 122 / 412 | 14 / 0 / 33 / 0 | 857 | 658 | 10 / 10 (errori 0 / 0) | ok (fee 0.000005 CnFA, reward 0.0000045 CnFA, refund 0 CnFA, net cost 0.0000005 CnFA) |
totale: inviate 3002500, accettate 3002500, rifiutate all'ingresso 0, incluse 3002502, non incluse 0
TPS effettivi medi 4997 (richiesti 5000); altezza 6013
blocco (tutta la prova): p50 12.2 / p95 16.7 / p99 20.2 / max 893.7 ms
RAM: all'inizio 85 MB, prima del nodo; alla fine 857 MB; stati in RAM 149851
invio→inclusione (3000 campioni, 1 ogni ~1000 tx, risoluzione ~40 ms): p50 67 / p95 123 / p99 126 / max 1011 ms
CPU media (% di un core): nodo 13, rete 0, client 32, altro 0
RSS intorno agli snapshot (altezza: prima -> 1 min dopo): 3000: 793 -> 815
journal 658 MB = 219 byte/tx, 3945 MB/ora; snapshot 91 MB; status.bin 11.4 MB
stati: 10 trovati e 10 scaduti come atteso (errori 0 / 0); wallet sotto carico: 2 ok, 0 errori
snapshot durante il carico: 2 (scrittura dello snapshot nel suo blocco: 781 ms, 876 ms), massimo 876 ms
riavvio da snapshot (altezza 6000) + journal: 6.4 s, altezza 6013, hash uguali
riesecuzione completa: stato iniziale 1.3 s, journal 111.0 s (altezza 6013)
  preparazione (fino all'altezza 1): 1 blocchi, 0 tx, 0.0 s, 0.1 ms/blocco
  carico: 5993 blocchi, 3002502 tx, 109.2 s, 18.2 ms/blocco
  tx incluse ma fallite (eseguite con errore, contate rieseguendo il journal): 0
  blocchi vuoti dopo l'inizio del carico (battito, coda): 0 blocchi, 0 tx, 0.0 s, 0.0 ms/blocco
  hash finale (blocco, stato incrementale, stato ricalcolato) uguali al nodo vivo
FINE
