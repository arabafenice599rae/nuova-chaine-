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
| 4 | 449600 | 449600 | 449600 | 7493 | 17.8 / 23.9 / 27.2 | 76 / 123 / 138 | 80 / 64 / 62 / 0 | 807 | 296 | 4 / 4 (errori 0 / 0) |  |
| 5 | 445600 | 445600 | 444800 | 7413 | 18.6 / 24.7 / 30.6 | 75 / 119 / 130 | 83 / 65 / 63 / 0 | 859 | 394 | 5 / 5 (errori 0 / 0) | ok (fee 0.000005 CnFA, reward 0.0000045 CnFA, refund 0 CnFA, net cost 0.0000005 CnFA) |
| 6 | 454400 | 454400 | 454824 | 7580 | 18.4 / 23.9 / 27.1 | 74 / 120 / 139 | 81 / 66 / 63 / 0 | 860 | 493 | 6 / 6 (errori 0 / 0) |  |
gradino 7500 tx/s: | 7500 | 7498 | 7498 | 7496 | 18.3 / 24.2 / 27.5 | 75 / 121 / 136 | 376 |
snapshot all'altezza 3000: RSS subito prima 837 MB, 1 minuto dopo 860 MB (+23 MB)
| 7 | 535450 | 535450 | 535821 | 8930 | 21.6 / 27.5 / 34.8 | 65 / 102 / 126 | 93 / 70 / 67 / 0 | 876 | 611 | 7 / 7 (errori 0 / 0) |  |
| 8 | 539500 | 539500 | 539379 | 8990 | 22.1 / 26.9 / 30.8 | 64 / 101 / 126 | 93 / 71 / 67 / 0 | 876 | 729 | 8 / 8 (errori 0 / 0) |  |
| 9 | 545200 | 545200 | 545150 | 9086 | 21.6 / 27.7 / 33.4 | 65 / 101 / 125 | 93 / 70 / 67 / 0 | 877 | 848 | 9 / 9 (errori 0 / 0) |  |
gradino 10000 tx/s: | 10000 | 9001 | 9001 | 9002 | 21.8 / 27.2 / 32.7 | 65 / 102 / 126 | 176 |

INTERROTTO al gradino 10000: il client (16 thread HTTP, una tx per richiesta) arriva a ~9000 tx/s inviate mentre il nodo resta sotto i limiti (p99 del blocco 33 ms, coda 176 tx); il limite misurato qui è del client. Si ripete con più client (RPC_CLIENTS).
