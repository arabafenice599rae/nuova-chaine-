# A/B 10M account, caso d2

comando: `X=1 SNAPSHOT_EVERY=1800 RESULTS=docs/soak/ab-10m-d2.md LOAD_DIR=<dir> soak 10000000 5000 15` (commit fdc7aae, più modifiche locali se indicate nel commit successivo); sysctl: 0 0 10 20 (dirty_background_bytes dirty_bytes dirty_background_ratio dirty_ratio)

macchina: 4 vCPU, Intel(R) Xeon(R) Processor @ 2.10GHz; commit fdc7aae; avvio 1794007904839 ms (unix)
avvio: snapshot iniziale su disco, Dirty 0 MB dopo 0.0 s di attesa
10000000 account, 5000 tx/s richiesti, 15 minuti, invio diretto all'ingresso (limite del nodo), rpc_recent_txs 20000, snapshot ogni 1800 blocchi, stream_catchup_blocks 36000
CPU in % di un core: nodo = thread dei blocchi; rete = runtime tokio (RPC e stream, lato nodo); client = thread che firmano e inviano (in modalità diretta includono la verifica delle firme all'ingresso)
| min | inviate | accettate | incluse | TPS effettivi | blocco p50 / p95 / p99 ms | invio→inclusione p50 / p95 / p99 ms | CPU nodo / rete / client / altro % | RSS MB | journal MB | stati ok (290 blocchi) / scaduti ok (>300) | wallet |
| 1 | 300300 | 300300 | 300300 | 5005 | 8.7 / 13.8 / 20.6 | 103 / 145 / 151 | 12 / 0 / 25 / 26 | 3883 | 66 | 1 / 1 (errori 0 / 0) |  |
| 2 | 300150 | 300150 | 300000 | 5000 | 8.8 / 12.8 / 16.5 | 103 / 146 / 151 | 11 / 0 / 25 / 5 | 3898 | 132 | 2 / 2 (errori 0 / 0) |  |
| 3 | 299900 | 299900 | 299700 | 4995 | 8.9 / 13.6 / 21.3 | 88 / 147 / 614 | 12 / 0 / 25 / 6 | 3967 | 197 | 3 / 3 (errori 0 / 0) |  |
snapshot all'altezza 1800: RSS subito prima 3966 MB, 1 minuto dopo 3987 MB (+21 MB)
| 4 | 300000 | 300000 | 300000 | 5000 | 9.2 / 13.4 / 19.3 | 73 / 130 / 136 | 12 / 0 / 26 / 18 | 3988 | 263 | 4 / 4 (errori 0 / 0) |  |
| 5 | 300700 | 300700 | 300000 | 5000 | 9.2 / 13.5 / 15.4 | 71 / 123 / 133 | 12 / 0 / 26 / 5 | 4008 | 329 | 5 / 5 (errori 0 / 0) | ok (fee 0.000005 CnFA, reward 0.0000045 CnFA, refund 0 CnFA, net cost 0.0000005 CnFA) |
| 6 | 299250 | 299250 | 300001 | 5000 | 9.5 / 14.1 / 16.7 | 71 / 122 / 134 | 12 / 0 / 26 / 6 | 4029 | 395 | 6 / 6 (errori 0 / 0) |  |
snapshot all'altezza 3600: RSS subito prima 4029 MB, 1 minuto dopo 4049 MB (+20 MB)
| 7 | 300200 | 300200 | 300000 | 5000 | 9.2 / 13.6 / 15.9 | 73 / 131 / 136 | 12 / 0 / 26 / 18 | 4049 | 461 | 7 / 7 (errori 0 / 0) |  |
| 8 | 299800 | 299800 | 300000 | 5000 | 8.9 / 12.9 / 17.5 | 71 / 122 / 133 | 12 / 0 / 25 / 4 | 4049 | 526 | 8 / 8 (errori 0 / 0) |  |
| 9 | 300000 | 300000 | 300000 | 5000 | 8.7 / 12.8 / 15.8 | 70 / 121 / 131 | 12 / 0 / 25 / 5 | 4049 | 592 | 9 / 9 (errori 0 / 0) |  |
snapshot all'altezza 5400: RSS subito prima 4049 MB, 1 minuto dopo 4059 MB (+10 MB)
| 10 | 300750 | 300750 | 300000 | 5000 | 8.7 / 12.5 / 15.6 | 70 / 122 / 133 | 11 / 0 / 25 / 17 | 4059 | 658 | 10 / 10 (errori 0 / 0) | ok (fee 0.000005 CnFA, reward 0.0000045 CnFA, refund 0 CnFA, net cost 0.0000005 CnFA) |
| 11 | 299400 | 299400 | 300001 | 5000 | 8.8 / 12.7 / 14.3 | 70 / 122 / 133 | 12 / 0 / 25 / 4 | 4059 | 724 | 11 / 11 (errori 0 / 0) |  |
| 12 | 299850 | 299850 | 300000 | 5000 | 8.9 / 12.4 / 14.4 | 70 / 122 / 133 | 12 / 0 / 25 / 5 | 4059 | 789 | 12 / 12 (errori 0 / 0) |  |
snapshot all'altezza 7200: RSS subito prima 4059 MB, 1 minuto dopo 4059 MB (+0 MB)
| 13 | 300000 | 300000 | 300000 | 5000 | 9.3 / 13.4 / 14.8 | 72 / 122 / 136 | 12 / 0 / 26 / 17 | 4059 | 855 | 13 / 13 (errori 0 / 0) |  |
| 14 | 300250 | 300250 | 300000 | 5000 | 9.5 / 15.0 / 18.2 | 72 / 122 / 133 | 13 / 0 / 27 / 4 | 4060 | 921 | 14 / 14 (errori 0 / 0) |  |
| 15 | 300500 | 300500 | 300000 | 5000 | 10.1 / 14.4 / 17.4 | 72 / 130 / 135 | 13 / 0 / 27 / 5 | 4060 | 987 | 15 / 15 (errori 0 / 0) | ok (fee 0.000005 CnFA, reward 0.0000045 CnFA, refund 0 CnFA, net cost 0.0000005 CnFA) |
totale: inviate 4501050, accettate 4501050, rifiutate all'ingresso 0, incluse 4501053, non incluse 0
TPS effettivi medi 4998 (richiesti 5000); altezza 9008
blocco (tutta la prova): p50 9.0 / p95 13.5 / p99 17.4 / max 313.4 ms
RAM: all'inizio 475 MB, prima del nodo; alla fine 4060 MB; stati in RAM 150151
latenza scomposta (4499 campioni), p50 / p99 ms: invio -> eseguito 49 / 108; eseguito -> durevole 1 / 10; durevole -> pubblicato 3 / 5; pubblicato -> visto dal soak 19 / 38
dall'accodamento al journal alla durabilità: p50 3.6 / p99 13.0 / max 618.6 ms (il blocco sopra è il tempo sul thread del nodo, attesa per coda piena compresa)

| snapshot (altezza) | durata s | picco Dirty MB | blocchi nella finestra | sync_data p99 / max ms |
|---|---|---|---|---|
| 1800 | 9.4 | 7 | 93 | 145.3 / 167.5 |
| 3600 | 9.8 | 8 | 95 | 147.0 / 252.1 |
| 5400 | 9.4 | 10 | 93 | 113.5 / 145.2 |
| 7200 | 9.4 | 7 | 94 | 109.7 / 141.4 |

sync_data del journal: finestra reale degli snapshot (375 blocchi) p99 145.2 / max 252.1 ms; finestra fissa di 600 blocchi (2409 blocchi) p99 12.0 / max 252.1 ms; resto (6587 blocchi) p99 3.2 / max 456.8 ms; picco Dirty in tutta la prova 10 MB
invio→inclusione (4499 campioni, 1 ogni ~1000 tx, risoluzione ~40 ms): p50 76 / p95 131 / p99 147 / max 986 ms
CPU media (% di un core): nodo 12, rete 0, client 26, altro 10
RSS intorno agli snapshot (altezza: prima -> 1 min dopo): 1800: 3966 -> 3987, 3600: 4029 -> 4049, 5400: 4049 -> 4059, 7200: 4059 -> 4059
journal 987 MB = 219 byte/tx, 3946 MB/ora; snapshot 892 MB; status.bin 11.4 MB
stati: 15 trovati e 15 scaduti come atteso (errori 0 / 0); wallet sotto carico: 3 ok, 0 errori
snapshot durante il carico: 5 (pausa sul thread del nodo: codifica degli stati e invio allo stato ombra; copia e scrittura sono in background: 0 ms, 0 ms, 0 ms, 0 ms, 0 ms), massimo 0 ms
blocchi oltre 100 ms: 9 [1457: 188 ms = esecuzione 14 + hash 4 + firma 0 + journal 0 + altro 168] [1459: 143 ms = esecuzione 9 + hash 2 + firma 0 + journal 0 + altro 131] [1461: 231 ms = esecuzione 14 + hash 4 + firma 0 + journal 0 + altro 212] [1466: 212 ms = esecuzione 19 + hash 5 + firma 0 + journal 0 + altro 186] [1468: 140 ms = esecuzione 10 + hash 3 + firma 0 + journal 0 + altro 126] [1470: 313 ms = esecuzione 29 + hash 8 + firma 1 + journal 0 + altro 272] [1473: 207 ms = esecuzione 17 + hash 4 + firma 0 + journal 0 + altro 183] [1476: 172 ms = esecuzione 17 + hash 4 + firma 0 + journal 0 + altro 149] [1478: 105 ms = esecuzione 9 + hash 2 + firma 0 + journal 0 + altro 92]
snapshot saltati (scrittura in corso, coda piena o ombra persa): 0; ultima scrittura 9433 ms; coda massima durante una scrittura 92 blocchi
FINE
