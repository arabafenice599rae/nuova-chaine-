# soak final-1m-5000-direct-10min

comando: `scripts/soak-run.sh final-1m-5000-direct-10min 1000000 5000 10` (commit 90c21e5)

macchina: 4 vCPU, Intel(R) Xeon(R) Processor @ 2.10GHz; commit 90c21e5; avvio 1793957210885 ms (unix)
1000000 account, 5000 tx/s richiesti, 10 minuti, invio diretto all'ingresso (limite del nodo), rpc_recent_txs 20000, snapshot ogni 3000 blocchi, stream_catchup_blocks 36000
CPU in % di un core: nodo = thread dei blocchi; rete = runtime tokio (RPC e stream, lato nodo); client = thread che firmano e inviano (in modalità diretta includono la verifica delle firme all'ingresso)
| min | inviate | accettate | incluse | TPS effettivi | blocco p50 / p95 / p99 ms | invio→inclusione p50 / p95 / p99 ms | CPU nodo / rete / client / altro % | RSS MB | journal MB | stati ok (290 blocchi) / scaduti ok (>300) | wallet |
