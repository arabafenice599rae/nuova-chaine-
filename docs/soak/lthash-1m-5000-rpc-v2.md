# soak lthash-1m-5000-rpc-v2

comando: `scripts/soak-run.sh lthash-1m-5000-rpc-v2 1000000 5000 30 --rpc` (commit 4a1596b)

macchina: 4 vCPU, Intel(R) Xeon(R) Processor @ 2.10GHz; commit 4a1596b; avvio 1793954919304 ms (unix)
1000000 account, 5000 tx/s richiesti, 30 minuti, invio via HTTP/JSON-RPC (end-to-end), rpc_recent_txs 20000, snapshot ogni 3000 blocchi, stream_catchup_blocks 36000
CPU in % di un core: nodo = thread dei blocchi; rete = runtime tokio (RPC e stream, lato nodo); client = thread che firmano e inviano (in modalità diretta includono la verifica delle firme all'ingresso)
| min | inviate | accettate | incluse | TPS effettivi | blocco p50 / p95 / p99 ms | invio→inclusione p50 / p95 / p99 ms | CPU nodo / rete / client / altro % | RSS MB | journal MB | stati ok (290 blocchi) / scaduti ok (>300) | wallet |
| 1 | 300700 | 300700 | 300760 | 5013 | 12.7 / 19.8 / 24.9 | 66 / 128 / 137 | 55 / 46 / 46 / 0 | 723 | 66 | 1 / 1 (errori 0 / 0) |  |
