# soak lthash-1m-5000-direct-10min-catchup128

comando: `scripts/soak-run.sh lthash-1m-5000-direct-10min-catchup128 1000000 5000 10` (commit 63973cd)

macchina: 4 vCPU, Intel(R) Xeon(R) Processor @ 2.10GHz; commit 63973cd; avvio 1793952829289 ms (unix)
1000000 account, 5000 tx/s richiesti, 10 minuti, invio diretto all'ingresso (limite del nodo), rpc_recent_txs 20000, snapshot ogni 3000 blocchi, stream_catchup_blocks 36000
| min | inviate | accettate | incluse | TPS effettivi | blocco p50 / p95 / p99 ms | RSS MB | journal MB | stati ok (290 blocchi) / scaduti ok (>300) | wallet |
| 1 | 300300 | 300300 | 299800 | 4997 | 11.8 / 16.5 / 20.4 | 719 | 66 | 1 / 1 (errori 0 / 0) |  |
| 2 | 300100 | 300100 | 300500 | 5008 | 12.0 / 17.3 / 20.2 | 738 | 132 | 2 / 2 (errori 0 / 0) |  |
| 3 | 300000 | 300000 | 300000 | 5000 | 12.5 / 19.6 / 26.7 | 757 | 197 | 3 / 3 (errori 0 / 0) |  |
| 4 | 300050 | 300050 | 300000 | 5000 | 12.5 / 21.7 / 34.2 | 775 | 263 | 4 / 4 (errori 0 / 0) |  |
