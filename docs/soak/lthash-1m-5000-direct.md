# soak lthash-1m-5000-direct

comando: `scripts/soak-run.sh lthash-1m-5000-direct 1000000 5000 30` (commit 4b3128a)

macchina: 4 vCPU, Intel(R) Xeon(R) Processor @ 2.10GHz; commit 4b3128a; avvio 1793949900065 ms (unix)
1000000 account, 5000 tx/s richiesti, 30 minuti, invio diretto all'ingresso (limite del nodo), rpc_recent_txs 20000, snapshot ogni 3000 blocchi
| min | inviate | accettate | incluse | TPS effettivi | blocco p50 / p95 / p99 ms | RSS MB | journal MB | stati ok (290 blocchi) / scaduti ok (>300) | wallet |
| 1 | 300300 | 300300 | 299850 | 4998 | 11.5 / 15.2 / 18.0 | 722 | 66 | 1 / 1 (errori 0 / 0) |  |
| 2 | 300000 | 300000 | 300000 | 5000 | 11.3 / 15.3 / 17.7 | 741 | 132 | 2 / 2 (errori 0 / 0) |  |
| 3 | 300000 | 300000 | 300000 | 5000 | 11.6 / 15.7 / 18.5 | 760 | 197 | 3 / 3 (errori 0 / 0) |  |
| 4 | 300000 | 300000 | 300000 | 5000 | 11.4 / 15.5 / 19.4 | 779 | 263 | 4 / 4 (errori 0 / 0) |  |
| 5 | 304000 | 304000 | 297450 | 4958 | 12.5 / 17.2 / 22.2 | 810 | 330 | 5 / 5 (errori 0 / 0) | ok (fee 0.000005 CnFA, reward 0.0000045 CnFA, refund 0 CnFA, net cost 0.0000005 CnFA) |
| 6 | 296100 | 296100 | 302751 | 5046 | 12.3 / 16.2 / 23.1 | 821 | 395 | 6 / 6 (errori 0 / 0) |  |
| 7 | 299900 | 299900 | 300000 | 5000 | 12.2 / 16.3 / 19.0 | 841 | 461 | 7 / 7 (errori 0 / 0) |  |
| 8 | 300000 | 300000 | 300000 | 5000 | 12.4 / 17.0 / 20.0 | 861 | 526 | 8 / 8 (errori 0 / 0) |  |
| 9 | 299950 | 299950 | 300050 | 5001 | 12.7 / 17.2 / 19.7 | 881 | 592 | 9 / 9 (errori 0 / 0) |  |
| 10 | 304500 | 304500 | 297500 | 4958 | 13.3 / 18.3 / 22.0 | 923 | 659 | 10 / 10 (errori 0 / 0) | ok (fee 0.000005 CnFA, reward 0.0000045 CnFA, refund 0 CnFA, net cost 0.0000005 CnFA) |
| 11 | 295550 | 295550 | 302401 | 5040 | 13.0 / 17.7 / 20.9 | 923 | 724 | 11 / 11 (errori 0 / 0) |  |
| 12 | 300150 | 300150 | 300000 | 5000 | 13.2 / 18.4 / 29.5 | 940 | 789 | 12 / 12 (errori 0 / 0) |  |
| 13 | 299900 | 299900 | 300150 | 5002 | 13.2 / 18.3 / 22.2 | 960 | 855 | 13 / 13 (errori 0 / 0) |  |
| 14 | 300050 | 300050 | 300000 | 5000 | 12.9 / 17.8 / 21.9 | 980 | 921 | 14 / 14 (errori 0 / 0) |  |
| 15 | 304850 | 304850 | 298050 | 4968 | 13.5 / 18.5 / 21.1 | 1006 | 988 | 15 / 15 (errori 0 / 0) | ok (fee 0.000005 CnFA, reward 0.0000045 CnFA, refund 0 CnFA, net cost 0.0000005 CnFA) |
| 16 | 295150 | 295150 | 301801 | 5030 | 13.4 / 18.8 / 21.8 | 1020 | 1053 | 16 / 16 (errori 0 / 0) |  |
| 17 | 299900 | 299900 | 300000 | 5000 | 13.6 / 19.3 / 22.6 | 1039 | 1118 | 17 / 17 (errori 0 / 0) |  |
| 18 | 300000 | 300000 | 300000 | 5000 | 13.3 / 17.8 / 20.5 | 1059 | 1184 | 18 / 18 (errori 0 / 0) |  |
| 19 | 299950 | 299950 | 300000 | 5000 | 13.3 / 18.0 / 21.6 | 1079 | 1250 | 19 / 19 (errori 0 / 0) |  |
| 20 | 304700 | 304700 | 298500 | 4975 | 12.7 / 17.0 / 25.2 | 1121 | 1317 | 20 / 20 (errori 0 / 0) | ok (fee 0.000005 CnFA, reward 0.0000045 CnFA, refund 0 CnFA, net cost 0.0000005 CnFA) |
| 21 | 295350 | 295350 | 301351 | 5023 | 13.0 / 17.9 / 23.3 | 1121 | 1382 | 21 / 21 (errori 0 / 0) |  |
| 22 | 300000 | 300000 | 300000 | 5000 | 13.3 / 18.2 / 22.6 | 1138 | 1447 | 22 / 22 (errori 0 / 0) |  |
| 23 | 300150 | 300150 | 300450 | 5008 | 13.2 / 18.4 / 22.0 | 1158 | 1513 | 23 / 23 (errori 0 / 0) |  |
| 24 | 299850 | 299850 | 299550 | 4992 | 12.6 / 16.5 / 21.0 | 1178 | 1579 | 24 / 24 (errori 0 / 0) |  |
| 25 | 304800 | 304800 | 298500 | 4975 | 13.3 / 17.9 / 20.2 | 1205 | 1646 | 25 / 25 (errori 0 / 0) | ok (fee 0.000005 CnFA, reward 0.0000045 CnFA, refund 0 CnFA, net cost 0.0000005 CnFA) |
| 26 | 295200 | 295200 | 301601 | 5027 | 13.5 / 18.6 / 25.7 | 1217 | 1711 | 26 / 26 (errori 0 / 0) |  |
| 27 | 299950 | 299950 | 300000 | 5000 | 13.9 / 18.9 / 23.6 | 1236 | 1776 | 27 / 27 (errori 0 / 0) |  |
| 28 | 300200 | 300200 | 300000 | 5000 | 13.8 / 19.7 / 26.0 | 1255 | 1842 | 28 / 28 (errori 0 / 0) |  |
| 29 | 299850 | 299850 | 300000 | 5000 | 14.2 / 19.2 / 23.9 | 1275 | 1908 | 29 / 29 (errori 0 / 0) |  |
| 30 | 304950 | 304950 | 297950 | 4966 | 13.6 / 19.2 / 43.9 | 1317 | 1975 | 30 / 30 (errori 0 / 0) | ok (fee 0.000005 CnFA, reward 0.0000045 CnFA, refund 0 CnFA, net cost 0.0000005 CnFA) |
totale: inviate 9005250, accettate 9005250, rifiutate all'ingresso 0, incluse 9005256, non incluse 0
TPS effettivi medi 4999 (richiesti 5000); altezza 18014
blocco (tutta la prova): p50 12.9 / p95 18.0 / p99 22.5 / max 1189.6 ms
RAM: all'inizio 85 MB, prima del nodo; alla fine 1318 MB; stati in RAM 150801
journal 1975 MB = 219 byte/tx, 3947 MB/ora; snapshot 91 MB; status.bin 11.4 MB
stati: 30 trovati e 30 scaduti come atteso (errori 0 / 0); wallet sotto carico: 6 ok, 0 errori
snapshot durante il carico: 6 (blocchi con snapshot: 1144 ms, 1177 ms, 1136 ms, 1066 ms, 1100 ms, 1190 ms), pausa massima 1190 ms
riavvio da snapshot (altezza 18000) + journal: 14.6 s, altezza 18014, hash uguali
