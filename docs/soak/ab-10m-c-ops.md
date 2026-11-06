# A/B 10M account, caso c-ops

comando: `SNAPSHOT_EVERY=1800 RESULTS=docs/soak/ab-10m-c-ops.md LOAD_DIR=<dir> soak 10000000 5000 15` (commit #13 a blocchi 29423b2 + ops); sysctl: 16777216 67108864 0 0 (dirty_background_bytes dirty_bytes dirty_background_ratio dirty_ratio)

macchina: 4 vCPU, Intel(R) Xeon(R) Processor @ 2.10GHz; commit #13 a blocchi 29423b2 + ops; avvio 1793985536101 ms (unix)
10000000 account, 5000 tx/s richiesti, 15 minuti, invio diretto all'ingresso (limite del nodo), rpc_recent_txs 20000, snapshot ogni 1800 blocchi, stream_catchup_blocks 36000
CPU in % di un core: nodo = thread dei blocchi; rete = runtime tokio (RPC e stream, lato nodo); client = thread che firmano e inviano (in modalità diretta includono la verifica delle firme all'ingresso)
| min | inviate | accettate | incluse | TPS effettivi | blocco p50 / p95 / p99 ms | invio→inclusione p50 / p95 / p99 ms | CPU nodo / rete / client / altro % | RSS MB | journal MB | stati ok (290 blocchi) / scaduti ok (>300) | wallet |
| 1 | 300500 | 300500 | 300300 | 5005 | 13.1 / 21.0 / 31.4 | 84 / 120 / 122 | 14 / 0 / 30 / 26 | 3684 | 66 | 1 / 1 (errori 0 / 0) |  |
| 2 | 300000 | 300000 | 300000 | 5000 | 13.7 / 19.2 / 21.2 | 80 / 119 / 121 | 14 / 0 / 31 / 4 | 3699 | 132 | 2 / 2 (errori 0 / 0) |  |
| 3 | 299950 | 299950 | 299550 | 4992 | 14.2 / 19.3 / 21.7 | 80 / 120 / 122 | 14 / 0 / 31 / 5 | 3765 | 197 | 3 / 3 (errori 0 / 0) |  |
snapshot all'altezza 1800: RSS subito prima 3764 MB, 1 minuto dopo 3785 MB (+21 MB)
| 4 | 299950 | 299950 | 300450 | 5008 | 14.4 / 22.8 / 26.6 | 85 / 121 / 142 | 14 / 0 / 31 / 19 | 3785 | 263 | 4 / 4 (errori 0 / 0) |  |
| 5 | 300500 | 300500 | 299550 | 4992 | 14.0 / 19.3 / 21.4 | 81 / 120 / 123 | 14 / 0 / 31 / 5 | 3806 | 329 | 5 / 5 (errori 0 / 0) | ok (fee 0.000005 CnFA, reward 0.0000045 CnFA, refund 0 CnFA, net cost 0.0000005 CnFA) |
| 6 | 299550 | 299550 | 300451 | 5008 | 13.8 / 19.1 / 21.5 | 80 / 120 / 140 | 14 / 0 / 30 / 5 | 3828 | 395 | 6 / 6 (errori 0 / 0) |  |
snapshot all'altezza 3600: RSS subito prima 3827 MB, 1 minuto dopo 3848 MB (+21 MB)
| 7 | 299950 | 299950 | 299800 | 4997 | 14.0 / 21.9 / 27.7 | 66 / 99 / 122 | 14 / 0 / 30 / 16 | 3848 | 461 | 7 / 7 (errori 0 / 0) |  |
| 8 | 299950 | 299950 | 300000 | 5000 | 13.5 / 19.7 / 21.4 | 54 / 94 / 96 | 14 / 0 / 30 / 4 | 3848 | 526 | 8 / 8 (errori 0 / 0) |  |
| 9 | 300100 | 300100 | 300000 | 5000 | 13.6 / 19.8 / 23.9 | 54 / 93 / 96 | 14 / 0 / 30 / 5 | 3848 | 592 | 9 / 9 (errori 0 / 0) |  |
snapshot all'altezza 5400: RSS subito prima 3848 MB, 1 minuto dopo 3859 MB (+11 MB)
| 10 | 300600 | 300600 | 300100 | 5002 | 14.1 / 21.5 / 25.6 | 78 / 119 / 121 | 14 / 0 / 30 / 18 | 3859 | 658 | 10 / 10 (errori 0 / 0) | ok (fee 0.000005 CnFA, reward 0.0000045 CnFA, refund 0 CnFA, net cost 0.0000005 CnFA) |
| 11 | 299400 | 299400 | 300001 | 5000 | 13.4 / 19.4 / 23.4 | 80 / 119 / 122 | 14 / 0 / 29 / 4 | 3859 | 724 | 11 / 11 (errori 0 / 0) |  |
| 12 | 299950 | 299950 | 300000 | 5000 | 13.1 / 18.1 / 21.4 | 80 / 119 / 121 | 13 / 0 / 29 / 6 | 3859 | 790 | 12 / 12 (errori 0 / 0) |  |
snapshot all'altezza 7200: RSS subito prima 3859 MB, 1 minuto dopo 3859 MB (+0 MB)
| 13 | 300000 | 300000 | 299850 | 4998 | 14.4 / 24.6 / 32.0 | 64 / 138 / 141 | 14 / 0 / 30 / 17 | 3859 | 855 | 13 / 13 (errori 0 / 0) |  |
| 14 | 300000 | 300000 | 299950 | 4999 | 13.6 / 20.3 / 23.4 | 63 / 138 / 140 | 14 / 0 / 30 / 4 | 3859 | 921 | 14 / 14 (errori 0 / 0) |  |
| 15 | 300500 | 300500 | 300000 | 5000 | 14.1 / 20.4 / 30.2 | 63 / 138 / 141 | 15 / 0 / 31 / 6 | 3859 | 987 | 15 / 15 (errori 0 / 0) | ok (fee 0.000005 CnFA, reward 0.0000045 CnFA, refund 0 CnFA, net cost 0.0000005 CnFA) |
totale: inviate 4500900, accettate 4500900, rifiutate all'ingresso 0, incluse 4500903, non incluse 0
TPS effettivi medi 4998 (richiesti 5000); altezza 9012
blocco (tutta la prova): p50 13.8 / p95 20.5 / p99 25.7 / max 250.5 ms
RAM: all'inizio 475 MB, prima del nodo; alla fine 3860 MB; stati in RAM 150351

| snapshot (altezza) | durata s | picco Dirty MB | blocchi nella finestra | sync_data p99 / max ms |
|---|---|---|---|---|
| 1800 | 10.4 | 8 | 102 | 107.7 / 135.9 |
| 3600 | 9.7 | 8 | 95 | 150.3 / 158.0 |
| 5400 | 9.7 | 8 | 94 | 160.4 / 230.2 |
| 7200 | 10.0 | 8 | 98 | 91.6 / 229.4 |

sync_data del journal: finestra reale degli snapshot (389 blocchi) p99 150.3 / max 230.2 ms; finestra fissa di 600 blocchi (2406 blocchi) p99 12.0 / max 230.2 ms; resto (6595 blocchi) p99 2.4 / max 44.1 ms; picco Dirty in tutta la prova 8 MB
invio→inclusione (4499 campioni, 1 ogni ~1000 tx, risoluzione ~40 ms): p50 75 / p95 121 / p99 140 / max 244 ms
CPU media (% di un core): nodo 14, rete 0, client 30, altro 10
RSS intorno agli snapshot (altezza: prima -> 1 min dopo): 1800: 3764 -> 3785, 3600: 3827 -> 3848, 5400: 3848 -> 3859, 7200: 3859 -> 3859
journal 987 MB = 219 byte/tx, 3946 MB/ora; snapshot 892 MB; status.bin 11.4 MB
stati: 15 trovati e 15 scaduti come atteso (errori 0 / 0); wallet sotto carico: 3 ok, 0 errori
snapshot durante il carico: 5 (pausa sul thread del nodo: codifica degli stati e invio allo stato ombra; copia e scrittura sono in background: 0 ms, 0 ms, 0 ms, 0 ms, 0 ms), massimo 0 ms
blocchi oltre 100 ms: 9 [1900: 151 ms = esecuzione 9 + hash 2 + firma 0 + journal 136 + altro 4] [1902: 121 ms = esecuzione 10 + hash 3 + firma 0 + journal 103 + altro 5] [1903: 123 ms = esecuzione 8 + hash 2 + firma 0 + journal 108 + altro 5] [3694: 173 ms = esecuzione 8 + hash 2 + firma 0 + journal 158 + altro 5] [3696: 177 ms = esecuzione 16 + hash 5 + firma 0 + journal 151 + altro 5] [5493: 174 ms = esecuzione 7 + hash 2 + firma 0 + journal 161 + altro 4] [5495: 250 ms = esecuzione 11 + hash 3 + firma 0 + journal 231 + altro 5] [7296: 241 ms = esecuzione 6 + hash 2 + firma 0 + journal 230 + altro 4] [7298: 120 ms = esecuzione 15 + hash 4 + firma 0 + journal 92 + altro 9]
snapshot saltati (scrittura in corso, coda piena o ombra persa): 0; ultima scrittura 9971 ms; coda massima durante una scrittura 98 blocchi
FINE
