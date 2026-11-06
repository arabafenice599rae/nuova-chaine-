# soak lthash-1m-5000-direct

comando: `scripts/soak-run.sh lthash-1m-5000-direct 1000000 5000 30` (commit 4b3128a)

macchina: 4 vCPU, Intel(R) Xeon(R) Processor @ 2.10GHz; commit 4b3128a; avvio 1793949900065 ms (unix)
1000000 account, 5000 tx/s richiesti, 30 minuti, invio diretto all'ingresso (limite del nodo), rpc_recent_txs 20000, snapshot ogni 3000 blocchi
| min | inviate | accettate | incluse | TPS effettivi | blocco p50 / p95 / p99 ms | RSS MB | journal MB | stati ok (290 blocchi) / scaduti ok (>300) | wallet |
