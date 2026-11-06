# soak rpc-limit-1m-gradini-48client

comando: `scripts/soak-run.sh rpc-limit-1m-gradini-48client 1000000 7500 21 --rpc` (commit 8de6eb4)

macchina: 4 vCPU, Intel(R) Xeon(R) Processor @ 2.10GHz; commit 8de6eb4; avvio 1793969746490 ms (unix)
1000000 account, 7500 tx/s richiesti, 21 minuti, invio via HTTP/JSON-RPC (end-to-end) (48 thread client), rpc_recent_txs 20000, snapshot ogni 3000 blocchi, stream_catchup_blocks 36000
CPU in % di un core: nodo = thread dei blocchi; rete = runtime tokio (RPC e stream, lato nodo); client = thread che firmano e inviano (in modalità diretta includono la verifica delle firme all'ingresso)
| min | inviate | accettate | incluse | TPS effettivi | blocco p50 / p95 / p99 ms | invio→inclusione p50 / p95 / p99 ms | CPU nodo / rete / client / altro % | RSS MB | journal MB | stati ok (290 blocchi) / scaduti ok (>300) | wallet |
gradini: 7500, 10000, 12500, 15000, 17500, 20000, 25000 tx/s, 3 minuti ciascuno; stop al primo con p99 del blocco > 100 ms o tx accettate non incluse
| 1 | 451200 | 451200 | 451200 | 7520 | 18.9 / 25.9 / 30.5 | 89 / 133 / 140 | 75 / 57 / 68 / 0 | 753 | 99 | 1 / 1 (errori 0 / 0) |  |
| 2 | 449150 | 449150 | 449117 | 7485 | 19.7 / 27.2 / 32.2 | 82 / 132 / 137 | 78 / 58 / 70 / 0 | 784 | 198 | 2 / 2 (errori 0 / 0) |  |
