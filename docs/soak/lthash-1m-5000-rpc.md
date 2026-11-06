# soak lthash-1m-5000-rpc

comando: `scripts/soak-run.sh lthash-1m-5000-rpc 1000000 5000 30 --rpc` (commit c3c8af9)

macchina: 4 vCPU, Intel(R) Xeon(R) Processor @ 2.10GHz; commit c3c8af9; avvio 1793953561781 ms (unix)
1000000 account, 5000 tx/s richiesti, 30 minuti, invio via HTTP/JSON-RPC (end-to-end), rpc_recent_txs 20000, snapshot ogni 3000 blocchi, stream_catchup_blocks 36000
| min | inviate | accettate | incluse | TPS effettivi | blocco p50 / p95 / p99 ms | RSS MB | journal MB | stati ok (290 blocchi) / scaduti ok (>300) | wallet |
| 1 | 301100 | 301100 | 301120 | 5019 | 13.1 / 20.3 / 26.7 | 724 | 66 | 1 / 1 (errori 0 / 0) |  |
| 2 | 300200 | 300200 | 300030 | 5000 | 13.8 / 20.7 / 31.0 | 744 | 132 | 2 / 2 (errori 0 / 0) |  |
| 3 | 299950 | 299950 | 300000 | 5000 | 13.4 / 20.8 / 25.3 | 764 | 198 | 3 / 3 (errori 0 / 0) |  |
| 4 | 299950 | 299950 | 300000 | 5000 | 13.1 / 19.2 / 21.5 | 784 | 263 | 4 / 4 (errori 0 / 0) |  |
