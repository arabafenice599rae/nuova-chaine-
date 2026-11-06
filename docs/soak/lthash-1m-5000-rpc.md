# soak lthash-1m-5000-rpc

comando: `scripts/soak-run.sh lthash-1m-5000-rpc 1000000 5000 30 --rpc` (commit c3c8af9)

macchina: 4 vCPU, Intel(R) Xeon(R) Processor @ 2.10GHz; commit c3c8af9; avvio 1793953561781 ms (unix)
1000000 account, 5000 tx/s richiesti, 30 minuti, invio via HTTP/JSON-RPC (end-to-end), rpc_recent_txs 20000, snapshot ogni 3000 blocchi, stream_catchup_blocks 36000
| min | inviate | accettate | incluse | TPS effettivi | blocco p50 / p95 / p99 ms | RSS MB | journal MB | stati ok (290 blocchi) / scaduti ok (>300) | wallet |
| 1 | 301100 | 301100 | 301120 | 5019 | 13.1 / 20.3 / 26.7 | 724 | 66 | 1 / 1 (errori 0 / 0) |  |
| 2 | 300200 | 300200 | 300030 | 5000 | 13.8 / 20.7 / 31.0 | 744 | 132 | 2 / 2 (errori 0 / 0) |  |
| 3 | 299950 | 299950 | 300000 | 5000 | 13.4 / 20.8 / 25.3 | 764 | 198 | 3 / 3 (errori 0 / 0) |  |
| 4 | 299950 | 299950 | 300000 | 5000 | 13.1 / 19.2 / 21.5 | 784 | 263 | 4 / 4 (errori 0 / 0) |  |
| 5 | 296750 | 296750 | 296001 | 4933 | 13.0 / 17.5 / 20.1 | 812 | 329 | 5 / 5 (errori 0 / 0) | ok (fee 0.000005 CnFA, reward 0.0000045 CnFA, refund 0 CnFA, net cost 0.0000005 CnFA) |
| 6 | 303400 | 303400 | 303992 | 5067 | 13.4 / 20.0 / 22.8 | 825 | 395 | 6 / 6 (errori 0 / 0) |  |
| 7 | 300000 | 300000 | 300008 | 5000 | 12.8 / 17.9 / 20.1 | 845 | 461 | 7 / 7 (errori 0 / 0) |  |
| 8 | 299850 | 299850 | 300000 | 5000 | 13.3 / 19.7 / 27.1 | 846 | 527 | 8 / 8 (errori 0 / 0) |  |
| 9 | 300000 | 300000 | 299984 | 5000 | 13.1 / 19.2 / 30.7 | 846 | 592 | 9 / 9 (errori 0 / 0) |  |
| 10 | 297550 | 297550 | 296018 | 4934 | 13.3 / 18.7 / 21.3 | 868 | 658 | 10 / 10 (errori 0 / 0) | ok (fee 0.000005 CnFA, reward 0.0000045 CnFA, refund 0 CnFA, net cost 0.0000005 CnFA) |
| 11 | 302450 | 302450 | 304023 | 5067 | 13.6 / 19.5 / 25.0 | 868 | 724 | 11 / 11 (errori 0 / 0) |  |
| 12 | 300000 | 300000 | 300015 | 5000 | 13.6 / 18.9 / 21.7 | 868 | 790 | 12 / 12 (errori 0 / 0) |  |
| 13 | 300100 | 300100 | 299999 | 5000 | 13.8 / 19.4 / 21.8 | 868 | 856 | 13 / 13 (errori 0 / 0) |  |
| 14 | 300000 | 300000 | 299996 | 5000 | 14.4 / 21.0 / 23.5 | 869 | 921 | 14 / 14 (errori 0 / 0) |  |
| 15 | 297100 | 297100 | 295541 | 4926 | 14.1 / 19.5 / 24.5 | 869 | 987 | 15 / 15 (errori 0 / 0) | ok (fee 0.000005 CnFA, reward 0.0000045 CnFA, refund 0 CnFA, net cost 0.0000005 CnFA) |
| 16 | 302950 | 302950 | 304311 | 5072 | 14.8 / 20.7 / 23.2 | 869 | 1053 | 16 / 16 (errori 0 / 0) |  |

INTERROTTO deliberatamente al minuto 16: il rapporto richiesto aggiunge RSS intorno agli snapshot, tx fallite, latenza invio->inclusione e CPU per nodo e client; il run si ripete come lthash-1m-5000-rpc-v2.
