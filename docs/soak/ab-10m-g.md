# A/B 10M account, caso g

comando: `SNAPSHOT_CHUNK_KB=1024 SNAPSHOT_IO_IDLE=1 SNAPSHOT_EVERY=1800 RESULTS=docs/soak/ab-10m-g.md LOAD_DIR=<dir> soak 10000000 5000 15` (commit ca44586, più modifiche locali se indicate nel commit successivo); sysctl: 0 0 10 20 (dirty_background_bytes dirty_bytes dirty_background_ratio dirty_ratio)

macchina: 4 vCPU, Intel(R) Xeon(R) Processor @ 2.10GHz; commit ca44586; avvio 1794010988053 ms (unix)
avvio: snapshot iniziale su disco, Dirty 0 MB dopo 0.0 s di attesa
10000000 account, 5000 tx/s richiesti, 15 minuti, invio diretto all'ingresso (limite del nodo), rpc_recent_txs 20000, snapshot ogni 1800 blocchi, stream_catchup_blocks 36000
CPU in % di un core: nodo = thread dei blocchi; rete = runtime tokio (RPC e stream, lato nodo); client = thread che firmano e inviano (in modalità diretta includono la verifica delle firme all'ingresso)
| min | inviate | accettate | incluse | TPS effettivi | blocco p50 / p95 / p99 ms | invio→inclusione p50 / p95 / p99 ms | CPU nodo / rete / client / altro % | RSS MB | journal MB | stati ok (290 blocchi) / scaduti ok (>300) | wallet |
| 1 | 300300 | 300300 | 299800 | 4997 | 8.1 / 14.9 / 22.4 | 99 / 140 / 151 | 11 / 0 / 24 / 26 | 3885 | 66 | 1 / 1 (errori 0 / 0) |  |
| 2 | 300050 | 300050 | 300500 | 5008 | 8.2 / 11.8 / 13.6 | 99 / 140 / 149 | 11 / 0 / 24 / 5 | 3900 | 132 | 2 / 2 (errori 0 / 0) |  |
| 3 | 300000 | 300000 | 299550 | 4992 | 8.4 / 12.4 / 14.9 | 99 / 141 / 150 | 11 / 0 / 24 / 6 | 3966 | 197 | 3 / 3 (errori 0 / 0) |  |
snapshot all'altezza 1800: RSS subito prima 3965 MB, 1 minuto dopo 3986 MB (+21 MB)
| 4 | 299950 | 299950 | 299950 | 4999 | 8.7 / 12.5 / 14.5 | 98 / 140 / 152 | 11 / 0 / 25 / 17 | 3986 | 263 | 4 / 4 (errori 0 / 0) |  |
| 5 | 300600 | 300600 | 300000 | 5000 | 8.6 / 12.6 / 14.3 | 96 / 139 / 150 | 11 / 0 / 24 / 4 | 4007 | 329 | 5 / 5 (errori 0 / 0) | ok (fee 0.000005 CnFA, reward 0.0000045 CnFA, refund 0 CnFA, net cost 0.0000005 CnFA) |
| 6 | 299550 | 299550 | 300501 | 5008 | 9.2 / 15.0 / 19.5 | 96 / 139 / 165 | 12 / 0 / 25 / 5 | 4029 | 395 | 6 / 6 (errori 0 / 0) |  |
snapshot all'altezza 3600: RSS subito prima 4028 MB, 1 minuto dopo 4049 MB (+21 MB)
| 7 | 299900 | 299900 | 300000 | 5000 | 8.8 / 12.9 / 15.4 | 99 / 149 / 157 | 11 / 0 / 25 / 16 | 4049 | 461 | 7 / 7 (errori 0 / 0) |  |
| 8 | 299950 | 299950 | 299500 | 4992 | 8.8 / 12.6 / 15.0 | 95 / 139 / 150 | 11 / 0 / 25 / 4 | 4049 | 526 | 8 / 8 (errori 0 / 0) |  |
| 9 | 300100 | 300100 | 300500 | 5008 | 8.5 / 12.7 / 15.4 | 96 / 139 / 149 | 11 / 0 / 24 / 5 | 4049 | 592 | 9 / 9 (errori 0 / 0) |  |
snapshot all'altezza 5400: RSS subito prima 4049 MB, 1 minuto dopo 4059 MB (+10 MB)
| 10 | 300500 | 300500 | 299550 | 4992 | 8.5 / 12.1 / 13.5 | 98 / 139 / 151 | 11 / 0 / 24 / 17 | 4059 | 658 | 10 / 10 (errori 0 / 0) | ok (fee 0.000005 CnFA, reward 0.0000045 CnFA, refund 0 CnFA, net cost 0.0000005 CnFA) |
| 11 | 299450 | 299450 | 300451 | 5008 | 8.6 / 12.4 / 14.5 | 97 / 140 / 154 | 11 / 0 / 25 / 4 | 4060 | 724 | 11 / 11 (errori 0 / 0) |  |
| 12 | 299950 | 299950 | 299500 | 4992 | 8.6 / 12.0 / 13.7 | 97 / 139 / 150 | 11 / 0 / 24 / 5 | 4060 | 789 | 12 / 12 (errori 0 / 0) |  |
snapshot all'altezza 7200: RSS subito prima 4060 MB, 1 minuto dopo 4060 MB (+0 MB)
| 13 | 300150 | 300150 | 300500 | 5008 | 8.8 / 14.4 / 18.3 | 103 / 140 / 155 | 12 / 0 / 25 / 16 | 4060 | 855 | 13 / 13 (errori 0 / 0) |  |
| 14 | 300000 | 300000 | 300000 | 5000 | 8.5 / 11.9 / 14.0 | 96 / 139 / 150 | 11 / 0 / 24 / 4 | 4060 | 921 | 14 / 14 (errori 0 / 0) |  |
| 15 | 300900 | 300900 | 299550 | 4992 | 8.8 / 14.1 / 18.4 | 99 / 148 / 153 | 12 / 0 / 25 / 5 | 4060 | 987 | 15 / 15 (errori 0 / 0) | ok (fee 0.000005 CnFA, reward 0.0000045 CnFA, refund 0 CnFA, net cost 0.0000005 CnFA) |
totale: inviate 4501450, accettate 4501450, rifiutate all'ingresso 0, incluse 4501453, non incluse 0
TPS effettivi medi 4998 (richiesti 5000); altezza 9009
blocco (tutta la prova): p50 8.6 / p95 12.9 / p99 16.6 / max 43.4 ms
RAM: all'inizio 476 MB, prima del nodo; alla fine 4060 MB; stati in RAM 150101
latenza scomposta (4500 campioni), p50 / p99 ms: invio -> eseguito 83 / 114; eseguito -> durevole 1 / 7; durevole -> pubblicato 2 / 5; pubblicato -> visto dal soak 19 / 38
dall'accodamento al journal alla durabilità: p50 3.2 / p99 10.3 / max 138.2 ms (il blocco sopra è il tempo sul thread del nodo, attesa per coda piena compresa)

| snapshot (altezza) | durata s | picco Dirty MB | blocchi nella finestra | sync_data p99 / max ms |
|---|---|---|---|---|
| 1800 | 9.4 | 1 | 92 | 74.8 / 135.5 |
| 3600 | 9.6 | 6 | 95 | 69.2 / 125.3 |
| 5400 | 9.4 | 10 | 94 | 71.2 / 99.6 |
| 7200 | 9.5 | 0 | 94 | 95.8 / 122.6 |

sync_data del journal: finestra reale degli snapshot (375 blocchi) p99 95.8 / max 135.5 ms; finestra fissa di 600 blocchi (2410 blocchi) p99 9.5 / max 135.5 ms; resto (6595 blocchi) p99 2.5 / max 72.9 ms; picco Dirty in tutta la prova 10 MB
invio→inclusione (4500 campioni, 1 ogni ~1000 tx, risoluzione ~40 ms): p50 97 / p95 140 / p99 153 / max 255 ms
CPU media (% di un core): nodo 11, rete 0, client 25, altro 9
RSS intorno agli snapshot (altezza: prima -> 1 min dopo): 1800: 3965 -> 3986, 3600: 4028 -> 4049, 5400: 4049 -> 4059, 7200: 4060 -> 4060
journal 987 MB = 219 byte/tx, 3946 MB/ora; snapshot 892 MB; status.bin 11.4 MB
stati: 15 trovati e 15 scaduti come atteso (errori 0 / 0); wallet sotto carico: 3 ok, 0 errori
snapshot durante il carico: 5 (pausa sul thread del nodo: codifica degli stati e invio allo stato ombra; copia e scrittura sono in background: 0 ms, 0 ms, 0 ms, 0 ms, 0 ms), massimo 0 ms
blocchi oltre 100 ms: 0 
snapshot saltati (scrittura in corso, coda piena o ombra persa): 0; ultima scrittura 9469 ms; coda massima durante una scrittura 90 blocchi
FINE
