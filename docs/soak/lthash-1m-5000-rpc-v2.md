# soak lthash-1m-5000-rpc-v2

comando: `scripts/soak-run.sh lthash-1m-5000-rpc-v2 1000000 5000 30 --rpc` (commit 4a1596b)

macchina: 4 vCPU, Intel(R) Xeon(R) Processor @ 2.10GHz; commit 4a1596b; avvio 1793954919304 ms (unix)
1000000 account, 5000 tx/s richiesti, 30 minuti, invio via HTTP/JSON-RPC (end-to-end), rpc_recent_txs 20000, snapshot ogni 3000 blocchi, stream_catchup_blocks 36000
CPU in % di un core: nodo = thread dei blocchi; rete = runtime tokio (RPC e stream, lato nodo); client = thread che firmano e inviano (in modalità diretta includono la verifica delle firme all'ingresso)
| min | inviate | accettate | incluse | TPS effettivi | blocco p50 / p95 / p99 ms | invio→inclusione p50 / p95 / p99 ms | CPU nodo / rete / client / altro % | RSS MB | journal MB | stati ok (290 blocchi) / scaduti ok (>300) | wallet |
| 1 | 300700 | 300700 | 300760 | 5013 | 12.7 / 19.8 / 24.9 | 66 / 128 / 137 | 55 / 46 / 46 / 0 | 723 | 66 | 1 / 1 (errori 0 / 0) |  |
| 2 | 300100 | 300100 | 299454 | 4991 | 13.8 / 23.7 / 26.9 | 64 / 130 / 862 | 57 / 48 / 48 / 0 | 743 | 132 | 2 / 2 (errori 0 / 0) |  |
| 3 | 300000 | 300000 | 299967 | 4999 | 14.0 / 23.0 / 27.1 | 75 / 109 / 117 | 58 / 49 / 49 / 0 | 762 | 197 | 3 / 3 (errori 0 / 0) |  |
| 4 | 299600 | 299600 | 299995 | 5000 | 14.6 / 25.1 / 28.1 | 77 / 111 / 117 | 62 / 51 / 52 / 0 | 782 | 263 | 4 / 4 (errori 0 / 0) |  |
| 5 | 297650 | 297650 | 297188 | 4953 | 14.3 / 24.1 / 27.1 | 76 / 109 / 115 | 61 / 49 / 50 / 0 | 808 | 329 | 5 / 5 (errori 0 / 0) | ok (fee 0.000005 CnFA, reward 0.0000045 CnFA, refund 0 CnFA, net cost 0.0000005 CnFA) |
| 6 | 302750 | 302750 | 302817 | 5047 | 14.7 / 24.7 / 26.9 | 76 / 111 / 116 | 61 / 50 / 50 / 0 | 819 | 395 | 6 / 6 (errori 0 / 0) |  |
| 7 | 300000 | 300000 | 300048 | 5001 | 14.5 / 24.8 / 27.3 | 75 / 109 / 115 | 60 / 49 / 49 / 0 | 838 | 461 | 7 / 7 (errori 0 / 0) |  |
| 8 | 300000 | 300000 | 299993 | 5000 | 14.5 / 23.9 / 27.2 | 75 / 108 / 113 | 60 / 49 / 49 / 0 | 840 | 526 | 8 / 8 (errori 0 / 0) |  |
| 9 | 300000 | 300000 | 299951 | 4999 | 14.0 / 23.7 / 26.5 | 76 / 110 / 171 | 58 / 48 / 48 / 0 | 841 | 592 | 9 / 9 (errori 0 / 0) |  |
| 10 | 296800 | 296800 | 296628 | 4944 | 14.5 / 23.6 / 26.1 | 75 / 108 / 114 | 59 / 48 / 48 / 0 | 863 | 657 | 10 / 10 (errori 0 / 0) | ok (fee 0.000005 CnFA, reward 0.0000045 CnFA, refund 0 CnFA, net cost 0.0000005 CnFA) |
| 11 | 302900 | 302900 | 303822 | 5064 | 14.9 / 24.2 / 26.9 | 91 / 126 / 131 | 60 / 49 / 49 / 0 | 863 | 724 | 11 / 11 (errori 0 / 0) |  |
snapshot all'altezza 6000: RSS subito prima 841 MB, 1 minuto dopo 863 MB (+22 MB)
