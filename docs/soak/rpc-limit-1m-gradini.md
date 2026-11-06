# soak rpc-limit-1m-gradini

comando: `scripts/soak-run.sh rpc-limit-1m-gradini 1000000 5000 24 --rpc` (commit 092e6ec)

macchina: 4 vCPU, Intel(R) Xeon(R) Processor @ 2.10GHz; commit 092e6ec; avvio 1793969153074 ms (unix)
1000000 account, 5000 tx/s richiesti, 24 minuti, invio via HTTP/JSON-RPC (end-to-end), rpc_recent_txs 20000, snapshot ogni 3000 blocchi, stream_catchup_blocks 36000
CPU in % di un core: nodo = thread dei blocchi; rete = runtime tokio (RPC e stream, lato nodo); client = thread che firmano e inviano (in modalità diretta includono la verifica delle firme all'ingresso)
| min | inviate | accettate | incluse | TPS effettivi | blocco p50 / p95 / p99 ms | invio→inclusione p50 / p95 / p99 ms | CPU nodo / rete / client / altro % | RSS MB | journal MB | stati ok (290 blocchi) / scaduti ok (>300) | wallet |
gradini: 5000, 7500, 10000, 12500, 15000, 17500, 20000, 25000 tx/s, 3 minuti ciascuno; stop al primo con p99 del blocco > 100 ms o tx accettate non incluse
| 1 | 300800 | 300800 | 300800 | 5013 | 13.5 / 25.7 / 36.7 | 64 / 128 / 137 | 57 / 47 / 46 / 0 | 723 | 66 | 1 / 1 (errori 0 / 0) |  |
| 2 | 300000 | 300000 | 300000 | 5000 | 13.4 / 20.4 / 25.5 | 62 / 129 / 135 | 56 / 47 / 47 / 0 | 743 | 132 | 2 / 2 (errori 0 / 0) |  |
| 3 | 300000 | 300000 | 300000 | 5000 | 13.5 / 21.1 / 25.7 | 67 / 126 / 135 | 56 / 47 / 46 / 0 | 762 | 198 | 3 / 3 (errori 0 / 0) |  |
gradino 5000 tx/s: | 5000 | 5004 | 5004 | 5004 | 13.4 / 22.7 / 31.5 | 65 / 128 / 136 | 0 |
