# A/B 10M account, caso d

comando: `SNAPSHOT_EVERY=1800 RESULTS=docs/soak/ab-10m-d.md LOAD_DIR=<dir> soak 10000000 5000 15` (commit PR B su #13 5d16356); sysctl: 0 0 10 20 (dirty_background_bytes dirty_bytes dirty_background_ratio dirty_ratio)

macchina: 4 vCPU, Intel(R) Xeon(R) Processor @ 2.10GHz; commit PR B su #13 5d16356; avvio 1793987982602 ms (unix)
10000000 account, 5000 tx/s richiesti, 15 minuti, invio diretto all'ingresso (limite del nodo), rpc_recent_txs 20000, snapshot ogni 1800 blocchi, stream_catchup_blocks 36000
CPU in % di un core: nodo = thread dei blocchi; rete = runtime tokio (RPC e stream, lato nodo); client = thread che firmano e inviano (in modalità diretta includono la verifica delle firme all'ingresso)
| min | inviate | accettate | incluse | TPS effettivi | blocco p50 / p95 / p99 ms | invio→inclusione p50 / p95 / p99 ms | CPU nodo / rete / client / altro % | RSS MB | journal MB | stati ok (290 blocchi) / scaduti ok (>300) | wallet |
| 1 | 300300 | 300300 | 299900 | 4998 | 10.3 / 16.1 / 22.2 | 109 / 150 / 450 | 14 / 0 / 30 / 29 | 3829 | 66 | 1 / 1 (errori 0 / 0) |  |
| 2 | 300050 | 300050 | 300000 | 5000 | 9.8 / 14.7 / 17.7 | 109 / 148 / 152 | 13 / 0 / 30 / 5 | 3842 | 132 | 2 / 2 (errori 0 / 0) |  |
| 3 | 300100 | 300100 | 300000 | 5000 | 9.9 / 14.2 / 16.2 | 110 / 138 / 151 | 13 / 0 / 30 / 6 | 3906 | 197 | 3 / 3 (errori 0 / 0) |  |
snapshot all'altezza 1800: RSS subito prima 3906 MB, 1 minuto dopo 3927 MB (+21 MB)
| 4 | 299850 | 299850 | 300000 | 5000 | 9.9 / 14.3 / 16.2 | 111 / 148 / 154 | 13 / 0 / 30 / 19 | 3927 | 263 | 4 / 4 (errori 0 / 0) |  |
| 5 | 300700 | 300700 | 300000 | 5000 | 9.8 / 14.0 / 16.1 | 110 / 139 / 154 | 13 / 0 / 29 / 5 | 3946 | 329 | 5 / 5 (errori 0 / 0) | ok (fee 0.000005 CnFA, reward 0.0000045 CnFA, refund 0 CnFA, net cost 0.0000005 CnFA) |
| 6 | 299350 | 299350 | 300001 | 5000 | 9.8 / 14.0 / 15.6 | 109 / 148 / 152 | 13 / 0 / 29 / 5 | 3967 | 395 | 6 / 6 (errori 0 / 0) |  |
snapshot all'altezza 3600: RSS subito prima 3966 MB, 1 minuto dopo 3986 MB (+20 MB)
| 7 | 299950 | 299950 | 300000 | 5000 | 9.8 / 14.3 / 17.9 | 110 / 139 / 153 | 13 / 0 / 29 / 16 | 3986 | 461 | 7 / 7 (errori 0 / 0) |  |
| 8 | 300000 | 300000 | 300000 | 5000 | 10.2 / 14.7 / 17.5 | 110 / 139 / 155 | 14 / 0 / 30 / 5 | 3987 | 526 | 8 / 8 (errori 0 / 0) |  |
| 9 | 300000 | 300000 | 300000 | 5000 | 10.1 / 14.5 / 15.9 | 110 / 140 / 153 | 14 / 0 / 30 / 6 | 3987 | 592 | 9 / 9 (errori 0 / 0) |  |
snapshot all'altezza 5400: RSS subito prima 3987 MB, 1 minuto dopo 3998 MB (+11 MB)
| 10 | 300550 | 300550 | 300000 | 5000 | 10.4 / 14.8 / 16.8 | 111 / 151 / 181 | 14 / 0 / 30 / 19 | 3998 | 658 | 10 / 10 (errori 0 / 0) | ok (fee 0.000005 CnFA, reward 0.0000045 CnFA, refund 0 CnFA, net cost 0.0000005 CnFA) |
| 11 | 299550 | 299550 | 300001 | 5000 | 10.2 / 14.4 / 16.1 | 111 / 149 / 155 | 14 / 0 / 30 / 5 | 3998 | 724 | 11 / 11 (errori 0 / 0) |  |
| 12 | 300000 | 300000 | 300000 | 5000 | 10.0 / 14.7 / 16.7 | 109 / 139 / 153 | 14 / 0 / 29 / 6 | 3998 | 789 | 12 / 12 (errori 0 / 0) |  |
snapshot all'altezza 7200: RSS subito prima 3998 MB, 1 minuto dopo 3998 MB (+0 MB)
| 13 | 300000 | 300000 | 300000 | 5000 | 10.1 / 14.4 / 18.1 | 111 / 152 / 176 | 14 / 0 / 29 / 17 | 3998 | 855 | 13 / 13 (errori 0 / 0) |  |
| 14 | 299900 | 299900 | 300000 | 5000 | 9.6 / 14.1 / 17.4 | 109 / 147 / 154 | 13 / 0 / 28 / 5 | 3998 | 921 | 14 / 14 (errori 0 / 0) |  |
| 15 | 301200 | 301200 | 300000 | 5000 | 9.7 / 13.9 / 15.1 | 111 / 139 / 153 | 13 / 0 / 29 / 6 | 3998 | 987 | 15 / 15 (errori 0 / 0) | ok (fee 0.000005 CnFA, reward 0.0000045 CnFA, refund 0 CnFA, net cost 0.0000005 CnFA) |
totale: inviate 4501500, accettate 4501500, rifiutate all'ingresso 0, incluse 4501503, non incluse 0
TPS effettivi medi 4998 (richiesti 5000); altezza 9010
blocco (tutta la prova): p50 9.9 / p95 14.5 / p99 17.3 / max 613.6 ms
RAM: all'inizio 475 MB, prima del nodo; alla fine 3999 MB; stati in RAM 150101
dall'accodamento al journal alla durabilità: p50 3.6 / p99 12.9 / max 803.7 ms (il blocco sopra è il tempo sul thread del nodo, attesa per coda piena compresa)

| snapshot (altezza) | durata s | picco Dirty MB | blocchi nella finestra | sync_data p99 / max ms |
|---|---|---|---|---|
| 1800 | 10.3 | 7 | 100 | 177.9 / 201.3 |
| 3600 | 9.7 | 8 | 96 | 150.4 / 208.6 |
| 5400 | 9.9 | 7 | 99 | 119.7 / 222.2 |
| 7200 | 9.9 | 8 | 98 | 152.8 / 208.3 |

sync_data del journal: finestra reale degli snapshot (393 blocchi) p99 177.9 / max 222.2 ms; finestra fissa di 600 blocchi (2411 blocchi) p99 12.8 / max 222.2 ms; resto (6590 blocchi) p99 2.4 / max 799.9 ms; picco Dirty in tutta la prova 850 MB
invio→inclusione (4498 campioni, 1 ogni ~1000 tx, risoluzione ~40 ms): p50 110 / p95 148 / p99 155 / max 1009 ms
CPU media (% di un core): nodo 13, rete 0, client 30, altro 10
RSS intorno agli snapshot (altezza: prima -> 1 min dopo): 1800: 3906 -> 3927, 3600: 3966 -> 3986, 5400: 3987 -> 3998, 7200: 3998 -> 3998
journal 987 MB = 219 byte/tx, 3946 MB/ora; snapshot 892 MB; status.bin 11.4 MB
stati: 15 trovati e 15 scaduti come atteso (errori 0 / 0); wallet sotto carico: 3 ok, 0 errori
snapshot durante il carico: 5 (pausa sul thread del nodo: codifica degli stati e invio allo stato ombra; copia e scrittura sono in background: 0 ms, 0 ms, 0 ms, 0 ms, 0 ms), massimo 0 ms
blocchi oltre 100 ms: 1 [57: 614 ms = esecuzione 71 + hash 24 + firma 1 + journal 0 + altro 513]
snapshot saltati (scrittura in corso, coda piena o ombra persa): 0; ultima scrittura 9865 ms; coda massima durante una scrittura 97 blocchi
FINE
