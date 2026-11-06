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
