# soak lthash-1m-5000-direct-10min-catchup128

comando: `scripts/soak-run.sh lthash-1m-5000-direct-10min-catchup128 1000000 5000 10` (commit 63973cd)

macchina: 4 vCPU, Intel(R) Xeon(R) Processor @ 2.10GHz; commit 63973cd; avvio 1793952829289 ms (unix)
1000000 account, 5000 tx/s richiesti, 10 minuti, invio diretto all'ingresso (limite del nodo), rpc_recent_txs 20000, snapshot ogni 3000 blocchi, stream_catchup_blocks 36000
| min | inviate | accettate | incluse | TPS effettivi | blocco p50 / p95 / p99 ms | RSS MB | journal MB | stati ok (290 blocchi) / scaduti ok (>300) | wallet |
