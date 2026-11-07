# A/B 10M account, caso e

comando: `SNAPSHOT_CHUNK_KB=1024 SNAPSHOT_EVERY=1800 RESULTS=docs/soak/ab-10m-e.md LOAD_DIR=<dir> soak 10000000 5000 15` (commit ca44586, più modifiche locali se indicate nel commit successivo); sysctl: 0 0 10 20 (dirty_background_bytes dirty_bytes dirty_background_ratio dirty_ratio)

macchina: 4 vCPU, Intel(R) Xeon(R) Processor @ 2.10GHz; commit ca44586; avvio 1794009068142 ms (unix)
avvio: snapshot iniziale su disco, Dirty 0 MB dopo 0.0 s di attesa
10000000 account, 5000 tx/s richiesti, 15 minuti, invio diretto all'ingresso (limite del nodo), rpc_recent_txs 20000, snapshot ogni 1800 blocchi, stream_catchup_blocks 36000
CPU in % di un core: nodo = thread dei blocchi; rete = runtime tokio (RPC e stream, lato nodo); client = thread che firmano e inviano (in modalità diretta includono la verifica delle firme all'ingresso)
| min | inviate | accettate | incluse | TPS effettivi | blocco p50 / p95 / p99 ms | invio→inclusione p50 / p95 / p99 ms | CPU nodo / rete / client / altro % | RSS MB | journal MB | stati ok (290 blocchi) / scaduti ok (>300) | wallet |
| 1 | 300400 | 300400 | 299850 | 4998 | 7.9 / 14.3 / 21.2 | 100 / 135 / 146 | 12 / 0 / 25 / 25 | 3884 | 66 | 1 / 1 (errori 0 / 0) |  |
| 2 | 300000 | 300000 | 300000 | 5000 | 7.9 / 11.3 / 16.3 | 97 / 126 / 136 | 10 / 0 / 24 / 5 | 3898 | 132 | 2 / 2 (errori 0 / 0) |  |
| 3 | 299850 | 299850 | 300000 | 5000 | 8.5 / 12.2 / 15.2 | 99 / 126 / 138 | 11 / 0 / 26 / 6 | 3961 | 197 | 3 / 3 (errori 0 / 0) |  |
snapshot all'altezza 1800: RSS subito prima 3961 MB, 1 minuto dopo 3982 MB (+21 MB)
| 4 | 300000 | 300000 | 300000 | 5000 | 8.5 / 12.5 / 15.9 | 101 / 136 / 142 | 12 / 0 / 26 / 18 | 3982 | 263 | 4 / 4 (errori 0 / 0) |  |
| 5 | 300650 | 300650 | 300000 | 5000 | 8.3 / 12.0 / 13.9 | 99 / 127 / 139 | 11 / 0 / 25 / 4 | 4002 | 329 | 5 / 5 (errori 0 / 0) | ok (fee 0.000005 CnFA, reward 0.0000045 CnFA, refund 0 CnFA, net cost 0.0000005 CnFA) |
| 6 | 299500 | 299500 | 300001 | 5000 | 8.5 / 12.2 / 15.7 | 98 / 126 / 139 | 11 / 0 / 26 / 5 | 4021 | 395 | 6 / 6 (errori 0 / 0) |  |
snapshot all'altezza 3600: RSS subito prima 4020 MB, 1 minuto dopo 4040 MB (+20 MB)
| 7 | 299900 | 299900 | 300000 | 5000 | 8.8 / 13.6 / 16.7 | 100 / 137 / 183 | 12 / 0 / 26 / 18 | 4040 | 461 | 7 / 7 (errori 0 / 0) |  |
| 8 | 299950 | 299950 | 300000 | 5000 | 8.6 / 12.4 / 14.0 | 100 / 127 / 140 | 12 / 0 / 26 / 5 | 4040 | 526 | 8 / 8 (errori 0 / 0) |  |
| 9 | 300100 | 300100 | 300000 | 5000 | 9.1 / 13.3 / 16.5 | 100 / 127 / 139 | 12 / 0 / 27 / 5 | 4040 | 592 | 9 / 9 (errori 0 / 0) |  |
snapshot all'altezza 5400: RSS subito prima 4040 MB, 1 minuto dopo 4050 MB (+10 MB)
| 10 | 300400 | 300400 | 300000 | 5000 | 8.7 / 12.6 / 15.1 | 100 / 136 / 140 | 12 / 0 / 26 / 17 | 4050 | 658 | 10 / 10 (errori 0 / 0) | ok (fee 0.000005 CnFA, reward 0.0000045 CnFA, refund 0 CnFA, net cost 0.0000005 CnFA) |
| 11 | 299600 | 299600 | 300001 | 5000 | 8.3 / 11.7 / 16.7 | 98 / 134 / 140 | 11 / 0 / 25 / 5 | 4050 | 724 | 11 / 11 (errori 0 / 0) |  |
| 12 | 299950 | 299950 | 300000 | 5000 | 8.6 / 12.5 / 15.5 | 100 / 136 / 139 | 12 / 0 / 26 / 5 | 4050 | 789 | 12 / 12 (errori 0 / 0) |  |
snapshot all'altezza 7200: RSS subito prima 4050 MB, 1 minuto dopo 4050 MB (+0 MB)
| 13 | 300000 | 300000 | 300000 | 5000 | 8.2 / 12.0 / 15.3 | 99 / 135 / 142 | 11 / 0 / 25 / 17 | 4050 | 855 | 13 / 13 (errori 0 / 0) |  |
| 14 | 300050 | 300050 | 300000 | 5000 | 8.1 / 11.6 / 12.9 | 97 / 126 / 137 | 11 / 0 / 24 / 4 | 4050 | 921 | 14 / 14 (errori 0 / 0) |  |
| 15 | 300550 | 300550 | 300000 | 5000 | 8.3 / 11.9 / 15.1 | 99 / 135 / 141 | 11 / 0 / 25 / 5 | 4050 | 987 | 15 / 15 (errori 0 / 0) | ok (fee 0.000005 CnFA, reward 0.0000045 CnFA, refund 0 CnFA, net cost 0.0000005 CnFA) |
totale: inviate 4500950, accettate 4500950, rifiutate all'ingresso 0, incluse 4500953, non incluse 0
TPS effettivi medi 4998 (richiesti 5000); altezza 9008
blocco (tutta la prova): p50 8.4 / p95 12.5 / p99 16.1 / max 109.1 ms
RAM: all'inizio 476 MB, prima del nodo; alla fine 4051 MB; stati in RAM 150101
latenza scomposta (4498 campioni), p50 / p99 ms: invio -> eseguito 76 / 101; eseguito -> durevole 1 / 8; durevole -> pubblicato 2 / 4; pubblicato -> visto dal soak 19 / 38
dall'accodamento al journal alla durabilità: p50 3.1 / p99 10.4 / max 384.2 ms (il blocco sopra è il tempo sul thread del nodo, attesa per coda piena compresa)

| snapshot (altezza) | durata s | picco Dirty MB | blocchi nella finestra | sync_data p99 / max ms |
|---|---|---|---|---|
| 1800 | 9.8 | 4 | 97 | 95.4 / 168.6 |
| 3600 | 9.7 | 0 | 97 | 114.5 / 224.6 |
| 5400 | 9.4 | 1 | 93 | 159.6 / 171.0 |
| 7200 | 9.5 | 8 | 94 | 133.7 / 150.4 |

sync_data del journal: finestra reale degli snapshot (381 blocchi) p99 150.4 / max 224.6 ms; finestra fissa di 600 blocchi (2409 blocchi) p99 8.6 / max 224.6 ms; resto (6595 blocchi) p99 2.6 / max 380.7 ms; picco Dirty in tutta la prova 8 MB
invio→inclusione (4498 campioni, 1 ogni ~1000 tx, risoluzione ~40 ms): p50 99 / p95 134 / p99 140 / max 284 ms
CPU media (% di un core): nodo 11, rete 0, client 25, altro 10
RSS intorno agli snapshot (altezza: prima -> 1 min dopo): 1800: 3961 -> 3982, 3600: 4020 -> 4040, 5400: 4040 -> 4050, 7200: 4050 -> 4050
journal 987 MB = 219 byte/tx, 3946 MB/ora; snapshot 892 MB; status.bin 11.4 MB
stati: 15 trovati e 15 scaduti come atteso (errori 0 / 0); wallet sotto carico: 3 ok, 0 errori
snapshot durante il carico: 5 (pausa sul thread del nodo: codifica degli stati e invio allo stato ombra; copia e scrittura sono in background: 0 ms, 0 ms, 0 ms, 0 ms, 0 ms), massimo 0 ms
blocchi oltre 100 ms: 1 [1156: 109 ms = esecuzione 13 + hash 3 + firma 0 + journal 0 + altro 92]
snapshot saltati (scrittura in corso, coda piena o ombra persa): 0; ultima scrittura 9510 ms; coda massima durante una scrittura 93 blocchi
FINE
