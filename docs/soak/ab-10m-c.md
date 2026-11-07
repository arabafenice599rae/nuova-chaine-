# A/B 10M account, caso c

comando: `SNAPSHOT_EVERY=1800 RESULTS=docs/soak/ab-10m-c.md LOAD_DIR=<dir> soak 10000000 5000 15` (commit #13 a blocchi 29423b2); sysctl: 0 0 10 20 (dirty_background_bytes dirty_bytes dirty_background_ratio dirty_ratio)

macchina: 4 vCPU, Intel(R) Xeon(R) Processor @ 2.10GHz; commit #13 a blocchi 29423b2; avvio 1793984580133 ms (unix)
10000000 account, 5000 tx/s richiesti, 15 minuti, invio diretto all'ingresso (limite del nodo), rpc_recent_txs 20000, snapshot ogni 1800 blocchi, stream_catchup_blocks 36000
CPU in % di un core: nodo = thread dei blocchi; rete = runtime tokio (RPC e stream, lato nodo); client = thread che firmano e inviano (in modalità diretta includono la verifica delle firme all'ingresso)
| min | inviate | accettate | incluse | TPS effettivi | blocco p50 / p95 / p99 ms | invio→inclusione p50 / p95 / p99 ms | CPU nodo / rete / client / altro % | RSS MB | journal MB | stati ok (290 blocchi) / scaduti ok (>300) | wallet |
| 1 | 300550 | 300550 | 300200 | 5003 | 13.5 / 25.3 / 34.1 | 55 / 103 / 272 | 14 / 0 / 31 / 29 | 3885 | 66 | 1 / 1 (errori 0 / 0) |  |
| 2 | 299950 | 299950 | 300050 | 5001 | 13.1 / 18.2 / 20.7 | 54 / 101 / 103 | 13 / 0 / 30 / 4 | 3900 | 132 | 2 / 2 (errori 0 / 0) |  |
| 3 | 300050 | 300050 | 300000 | 5000 | 14.3 / 19.5 / 22.2 | 42 / 100 / 103 | 14 / 0 / 31 / 6 | 3965 | 197 | 3 / 3 (errori 0 / 0) |  |
snapshot all'altezza 1800: RSS subito prima 3964 MB, 1 minuto dopo 3985 MB (+21 MB)
| 4 | 299900 | 299900 | 300000 | 5000 | 13.7 / 21.9 / 27.3 | 51 / 108 / 111 | 14 / 0 / 30 / 19 | 3986 | 263 | 4 / 4 (errori 0 / 0) |  |
| 5 | 300550 | 300550 | 300000 | 5000 | 13.3 / 18.1 / 20.2 | 52 / 108 / 110 | 13 / 0 / 30 / 4 | 4006 | 329 | 5 / 5 (errori 0 / 0) | ok (fee 0.000005 CnFA, reward 0.0000045 CnFA, refund 0 CnFA, net cost 0.0000005 CnFA) |
| 6 | 299400 | 299400 | 300001 | 5000 | 14.0 / 19.5 / 24.5 | 52 / 108 / 111 | 14 / 0 / 31 / 6 | 4028 | 395 | 6 / 6 (errori 0 / 0) |  |
snapshot all'altezza 3600: RSS subito prima 4027 MB, 1 minuto dopo 4048 MB (+21 MB)
| 7 | 300150 | 300150 | 300000 | 5000 | 14.1 / 22.7 / 28.8 | 53 / 109 / 111 | 14 / 0 / 30 / 16 | 4048 | 461 | 7 / 7 (errori 0 / 0) |  |
| 8 | 299950 | 299950 | 300000 | 5000 | 13.9 / 18.6 / 20.5 | 53 / 109 / 110 | 14 / 0 / 30 / 4 | 4048 | 526 | 8 / 8 (errori 0 / 0) |  |
| 9 | 300000 | 300000 | 300000 | 5000 | 14.1 / 19.4 / 22.9 | 52 / 109 / 111 | 14 / 0 / 30 / 6 | 4048 | 592 | 9 / 9 (errori 0 / 0) |  |
snapshot all'altezza 5400: RSS subito prima 4048 MB, 1 minuto dopo 4059 MB (+11 MB)
| 10 | 300550 | 300550 | 300050 | 5001 | 15.7 / 24.8 / 29.7 | 56 / 113 / 116 | 16 / 0 / 32 / 18 | 4059 | 658 | 10 / 10 (errori 0 / 0) | ok (fee 0.000005 CnFA, reward 0.0000045 CnFA, refund 0 CnFA, net cost 0.0000005 CnFA) |
| 11 | 299350 | 299350 | 300001 | 5000 | 14.3 / 19.1 / 21.6 | 57 / 114 / 116 | 15 / 0 / 31 / 4 | 4059 | 724 | 11 / 11 (errori 0 / 0) |  |
| 12 | 300200 | 300200 | 300200 | 5003 | 14.5 / 19.5 / 24.5 | 73 / 109 / 115 | 15 / 0 / 31 / 6 | 4059 | 790 | 12 / 12 (errori 0 / 0) |  |
snapshot all'altezza 7200: RSS subito prima 4059 MB, 1 minuto dopo 4059 MB (+0 MB)
| 13 | 299950 | 299950 | 299950 | 4999 | 14.7 / 23.0 / 27.2 | 72 / 103 / 145 | 15 / 0 / 31 / 16 | 4059 | 855 | 13 / 13 (errori 0 / 0) |  |
| 14 | 299950 | 299950 | 300000 | 5000 | 14.1 / 19.4 / 23.2 | 68 / 102 / 104 | 15 / 0 / 31 / 4 | 4059 | 921 | 14 / 14 (errori 0 / 0) |  |
| 15 | 300600 | 300600 | 300000 | 5000 | 13.7 / 20.0 / 22.6 | 68 / 101 / 104 | 14 / 0 / 31 / 7 | 4059 | 987 | 15 / 15 (errori 0 / 0) | ok (fee 0.000005 CnFA, reward 0.0000045 CnFA, refund 0 CnFA, net cost 0.0000005 CnFA) |
totale: inviate 4501100, accettate 4501100, rifiutate all'ingresso 0, incluse 4501103, non incluse 0
TPS effettivi medi 4998 (richiesti 5000); altezza 9015
blocco (tutta la prova): p50 14.0 / p95 20.5 / p99 27.1 / max 334.8 ms
RAM: all'inizio 475 MB, prima del nodo; alla fine 4059 MB; stati in RAM 150151

| snapshot (altezza) | durata s | picco Dirty MB | blocchi nella finestra | sync_data p99 / max ms |
|---|---|---|---|---|
| 1800 | 10.1 | 7 | 99 | 113.2 / 148.9 |
| 3600 | 9.7 | 8 | 96 | 126.6 / 138.1 |
| 5400 | 10.0 | 8 | 98 | 147.8 / 226.8 |
| 7200 | 9.8 | 7 | 95 | 133.5 / 218.8 |

sync_data del journal: finestra reale degli snapshot (388 blocchi) p99 138.1 / max 226.8 ms; finestra fissa di 600 blocchi (2410 blocchi) p99 12.3 / max 226.8 ms; resto (6587 blocchi) p99 2.7 / max 317.3 ms; picco Dirty in tutta la prova 850 MB
invio→inclusione (4500 campioni, 1 ogni ~1000 tx, risoluzione ~40 ms): p50 60 / p95 108 / p99 114 / max 820 ms
CPU media (% di un core): nodo 14, rete 0, client 31, altro 10
RSS intorno agli snapshot (altezza: prima -> 1 min dopo): 1800: 3964 -> 3985, 3600: 4027 -> 4048, 5400: 4048 -> 4059, 7200: 4059 -> 4059
journal 987 MB = 219 byte/tx, 3946 MB/ora; snapshot 892 MB; status.bin 11.4 MB
stati: 15 trovati e 15 scaduti come atteso (errori 0 / 0); wallet sotto carico: 3 ok, 0 errori
snapshot durante il carico: 5 (pausa sul thread del nodo: codifica degli stati e invio allo stato ombra; copia e scrittura sono in background: 0 ms, 0 ms, 0 ms, 0 ms, 0 ms), massimo 0 ms
blocchi oltre 100 ms: 13 [37: 328 ms = esecuzione 15 + hash 2 + firma 0 + journal 301 + altro 10] [40: 391 ms = esecuzione 97 + hash 7 + firma 1 + journal 262 + altro 25] [139: 121 ms = esecuzione 7 + hash 2 + firma 0 + journal 2 + altro 110] [1897: 165 ms = esecuzione 9 + hash 3 + firma 0 + journal 149 + altro 4] [1899: 136 ms = esecuzione 12 + hash 4 + firma 0 + journal 114 + altro 5] [1900: 105 ms = esecuzione 12 + hash 3 + firma 0 + journal 84 + altro 6] [3694: 142 ms = esecuzione 9 + hash 2 + firma 0 + journal 127 + altro 4] [3695: 153 ms = esecuzione 8 + hash 2 + firma 0 + journal 138 + altro 4] [5496: 159 ms = esecuzione 6 + hash 1 + firma 0 + journal 148 + altro 4] [5498: 244 ms = esecuzione 10 + hash 3 + firma 0 + journal 227 + altro 5] [6780: 335 ms = esecuzione 10 + hash 3 + firma 0 + journal 318 + altro 5] [7294: 232 ms = esecuzione 7 + hash 2 + firma 0 + journal 219 + altro 4] [7296: 161 ms = esecuzione 15 + hash 4 + firma 0 + journal 134 + altro 7]
snapshot saltati (scrittura in corso, coda piena o ombra persa): 0; ultima scrittura 9774 ms; coda massima durante una scrittura 95 blocchi
FINE
