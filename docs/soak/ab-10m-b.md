# A/B 10M account, caso b

comando: `SNAPSHOT_EVERY=1800 RESULTS=docs/soak/ab-10m-b.md LOAD_DIR=<dir> soak 10000000 5000 15` (commit #13 com'è ora (c811cce + misura)); sysctl: 0 0 10 20 (dirty_background_bytes dirty_bytes dirty_background_ratio dirty_ratio)

macchina: 4 vCPU, Intel(R) Xeon(R) Processor @ 2.10GHz; commit #13 com'è ora (c811cce + misura); avvio 1793983620011 ms (unix)
10000000 account, 5000 tx/s richiesti, 15 minuti, invio diretto all'ingresso (limite del nodo), rpc_recent_txs 20000, snapshot ogni 1800 blocchi, stream_catchup_blocks 36000
CPU in % di un core: nodo = thread dei blocchi; rete = runtime tokio (RPC e stream, lato nodo); client = thread che firmano e inviano (in modalità diretta includono la verifica delle firme all'ingresso)
| min | inviate | accettate | incluse | TPS effettivi | blocco p50 / p95 / p99 ms | invio→inclusione p50 / p95 / p99 ms | CPU nodo / rete / client / altro % | RSS MB | journal MB | stati ok (290 blocchi) / scaduti ok (>300) | wallet |
| 1 | 300300 | 300300 | 300100 | 5002 | 13.0 / 18.9 / 34.1 | 53 / 97 / 357 | 14 / 0 / 30 / 27 | 3885 | 66 | 1 / 1 (errori 0 / 0) |  |
| 2 | 300100 | 300100 | 300000 | 5000 | 12.9 / 17.4 / 19.1 | 52 / 97 / 99 | 13 / 0 / 30 / 4 | 3898 | 132 | 2 / 2 (errori 0 / 0) |  |
| 3 | 299900 | 299900 | 300000 | 5000 | 13.5 / 18.8 / 21.7 | 53 / 97 / 99 | 14 / 0 / 30 / 6 | 3964 | 197 | 3 / 3 (errori 0 / 0) |  |
snapshot all'altezza 1800: RSS subito prima 3963 MB, 1 minuto dopo 3984 MB (+21 MB)
| 4 | 300150 | 300150 | 299950 | 4999 | 14.0 / 19.1 / 23.3 | 58 / 94 / 140 | 14 / 0 / 31 / 20 | 3984 | 263 | 4 / 4 (errori 0 / 0) |  |
| 5 | 300450 | 300450 | 300000 | 5000 | 14.1 / 19.5 / 21.8 | 59 / 91 / 134 | 14 / 0 / 31 / 5 | 4005 | 329 | 5 / 5 (errori 0 / 0) | ok (fee 0.000005 CnFA, reward 0.0000045 CnFA, refund 0 CnFA, net cost 0.0000005 CnFA) |
| 6 | 299400 | 299400 | 300001 | 5000 | 13.7 / 18.4 / 20.2 | 59 / 93 / 138 | 14 / 0 / 31 / 6 | 4026 | 395 | 6 / 6 (errori 0 / 0) |  |
snapshot all'altezza 3600: RSS subito prima 4025 MB, 1 minuto dopo 4047 MB (+22 MB)
| 7 | 300000 | 300000 | 299950 | 4999 | 14.2 / 19.3 / 37.6 | 58 / 128 / 204 | 14 / 0 / 31 / 15 | 4047 | 461 | 7 / 7 (errori 0 / 0) |  |
| 8 | 300050 | 300050 | 300000 | 5000 | 13.6 / 18.4 / 20.2 | 57 / 127 / 129 | 14 / 0 / 30 / 4 | 4047 | 526 | 8 / 8 (errori 0 / 0) |  |
| 9 | 299950 | 299950 | 300000 | 5000 | 13.8 / 19.1 / 20.7 | 57 / 127 / 128 | 14 / 0 / 30 / 6 | 4047 | 592 | 9 / 9 (errori 0 / 0) |  |
snapshot all'altezza 5400: RSS subito prima 4047 MB, 1 minuto dopo 4060 MB (+13 MB)
| 10 | 300600 | 300600 | 300200 | 5003 | 13.2 / 17.9 / 22.5 | 76 / 122 / 147 | 13 / 0 / 29 / 18 | 4060 | 658 | 10 / 10 (errori 0 / 0) | ok (fee 0.000005 CnFA, reward 0.0000045 CnFA, refund 0 CnFA, net cost 0.0000005 CnFA) |
| 11 | 299550 | 299550 | 300001 | 5000 | 13.1 / 17.3 / 19.8 | 78 / 121 / 123 | 13 / 0 / 29 / 4 | 4060 | 724 | 11 / 11 (errori 0 / 0) |  |
| 12 | 299900 | 299900 | 300000 | 5000 | 13.3 / 18.0 / 22.9 | 77 / 121 / 123 | 13 / 0 / 29 / 6 | 4060 | 790 | 12 / 12 (errori 0 / 0) |  |
snapshot all'altezza 7200: RSS subito prima 4060 MB, 1 minuto dopo 4060 MB (+0 MB)
| 13 | 300000 | 300000 | 299650 | 4994 | 13.7 / 19.4 / 46.6 | 88 / 129 / 152 | 14 / 0 / 30 / 14 | 4060 | 855 | 13 / 13 (errori 0 / 0) |  |
| 14 | 300000 | 300000 | 300000 | 5000 | 13.7 / 19.1 / 20.6 | 88 / 129 / 130 | 14 / 0 / 30 / 4 | 4060 | 921 | 14 / 14 (errori 0 / 0) |  |
| 15 | 300500 | 300500 | 300000 | 5000 | 14.3 / 19.6 / 29.3 | 88 / 128 / 130 | 15 / 0 / 31 / 7 | 4060 | 987 | 15 / 15 (errori 0 / 0) | ok (fee 0.000005 CnFA, reward 0.0000045 CnFA, refund 0 CnFA, net cost 0.0000005 CnFA) |
totale: inviate 4500900, accettate 4500900, rifiutate all'ingresso 0, incluse 4500903, non incluse 0
TPS effettivi medi 4998 (richiesti 5000); altezza 9016
blocco (tutta la prova): p50 13.6 / p95 18.8 / p99 22.9 / max 686.2 ms
RAM: all'inizio 474 MB, prima del nodo; alla fine 4060 MB; stati in RAM 150551

| snapshot (altezza) | durata s | picco Dirty MB | blocchi nella finestra | sync_data p99 / max ms |
|---|---|---|---|---|
| 1800 | 11.4 | 850 | 108 | 197.7 / 584.8 |
| 3600 | 8.8 | 849 | 79 | 345.6 / 366.8 |
| 5400 | 10.5 | 846 | 98 | 158.0 / 629.3 |
| 7200 | 8.3 | 848 | 78 | 180.6 / 627.0 |

sync_data del journal: finestra reale degli snapshot (363 blocchi) p99 345.6 / max 629.3 ms; finestra fissa di 600 blocchi (2388 blocchi) p99 16.3 / max 629.3 ms; resto (6588 blocchi) p99 2.1 / max 650.3 ms; picco Dirty in tutta la prova 850 MB
invio→inclusione (4498 campioni, 1 ogni ~1000 tx, risoluzione ~40 ms): p50 70 / p95 124 / p99 130 / max 958 ms
CPU media (% di un core): nodo 14, rete 0, client 30, altro 10
RSS intorno agli snapshot (altezza: prima -> 1 min dopo): 1800: 3963 -> 3984, 3600: 4025 -> 4047, 5400: 4047 -> 4060, 7200: 4060 -> 4060
journal 987 MB = 219 byte/tx, 3946 MB/ora; snapshot 892 MB; status.bin 11.4 MB
stati: 15 trovati e 15 scaduti come atteso (errori 0 / 0); wallet sotto carico: 3 ok, 0 errori
snapshot durante il carico: 5 (pausa sul thread del nodo: codifica degli stati e invio allo stato ombra; copia e scrittura sono in background: 0 ms, 0 ms, 0 ms, 0 ms, 0 ms), massimo 0 ms
blocchi oltre 100 ms: 16 [18: 686 ms = esecuzione 20 + hash 2 + firma 0 + journal 651 + altro 13] [25: 183 ms = esecuzione 102 + hash 14 + firma 1 + journal 11 + altro 55] [1902: 596 ms = esecuzione 6 + hash 2 + firma 0 + journal 585 + altro 4] [1911: 177 ms = esecuzione 7 + hash 2 + firma 0 + journal 164 + altro 4] [1913: 217 ms = esecuzione 11 + hash 3 + firma 0 + journal 198 + altro 5] [3672: 199 ms = esecuzione 7 + hash 2 + firma 0 + journal 187 + altro 4] [3674: 391 ms = esecuzione 14 + hash 4 + firma 0 + journal 368 + altro 6] [3678: 402 ms = esecuzione 32 + hash 7 + firma 1 + journal 347 + altro 15] [3685: 163 ms = esecuzione 6 + hash 2 + firma 0 + journal 150 + altro 4] [3687: 226 ms = esecuzione 11 + hash 3 + firma 0 + journal 207 + altro 5] [5492: 645 ms = esecuzione 7 + hash 2 + firma 0 + journal 629 + altro 6] [5502: 175 ms = esecuzione 9 + hash 2 + firma 0 + journal 158 + altro 6] [7269: 193 ms = esecuzione 7 + hash 2 + firma 0 + journal 181 + altro 4] [7273: 633 ms = esecuzione 3 + hash 1 + firma 0 + journal 627 + altro 3] [7282: 123 ms = esecuzione 6 + hash 2 + firma 0 + journal 112 + altro 4] [7283: 158 ms = esecuzione 11 + hash 3 + firma 0 + journal 138 + altro 5]
snapshot saltati (scrittura in corso, coda piena o ombra persa): 0; ultima scrittura 8343 ms; coda massima durante una scrittura 101 blocchi
FINE
