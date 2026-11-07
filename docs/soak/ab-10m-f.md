# A/B 10M account, caso f

comando: `SNAPSHOT_IO_IDLE=1 SNAPSHOT_EVERY=1800 RESULTS=docs/soak/ab-10m-f.md LOAD_DIR=<dir> soak 10000000 5000 15` (commit ca44586, più modifiche locali se indicate nel commit successivo); sysctl: 0 0 10 20 (dirty_background_bytes dirty_bytes dirty_background_ratio dirty_ratio)

macchina: 4 vCPU, Intel(R) Xeon(R) Processor @ 2.10GHz; commit ca44586; avvio 1794010032554 ms (unix)
avvio: snapshot iniziale su disco, Dirty 0 MB dopo 0.0 s di attesa
10000000 account, 5000 tx/s richiesti, 15 minuti, invio diretto all'ingresso (limite del nodo), rpc_recent_txs 20000, snapshot ogni 1800 blocchi, stream_catchup_blocks 36000
CPU in % di un core: nodo = thread dei blocchi; rete = runtime tokio (RPC e stream, lato nodo); client = thread che firmano e inviano (in modalità diretta includono la verifica delle firme all'ingresso)
| min | inviate | accettate | incluse | TPS effettivi | blocco p50 / p95 / p99 ms | invio→inclusione p50 / p95 / p99 ms | CPU nodo / rete / client / altro % | RSS MB | journal MB | stati ok (290 blocchi) / scaduti ok (>300) | wallet |
| 1 | 300400 | 300400 | 299850 | 4998 | 8.6 / 14.1 / 18.6 | 101 / 137 / 144 | 12 / 0 / 26 / 25 | 3767 | 66 | 1 / 1 (errori 0 / 0) |  |
| 2 | 299850 | 299850 | 300000 | 5000 | 8.6 / 12.8 / 16.2 | 100 / 129 / 138 | 12 / 0 / 26 / 5 | 3780 | 132 | 2 / 2 (errori 0 / 0) |  |
| 3 | 300050 | 300050 | 300000 | 5000 | 8.8 / 13.2 / 17.9 | 101 / 138 / 143 | 12 / 0 / 27 / 6 | 3844 | 197 | 3 / 3 (errori 0 / 0) |  |
snapshot all'altezza 1800: RSS subito prima 3843 MB, 1 minuto dopo 3865 MB (+22 MB)
| 4 | 300000 | 300000 | 300000 | 5000 | 8.6 / 12.4 / 16.9 | 101 / 138 / 151 | 12 / 0 / 26 / 18 | 3865 | 263 | 4 / 4 (errori 0 / 0) |  |
| 5 | 300500 | 300500 | 300000 | 5000 | 8.6 / 12.9 / 17.4 | 101 / 138 / 142 | 12 / 0 / 26 / 5 | 3884 | 329 | 5 / 5 (errori 0 / 0) | ok (fee 0.000005 CnFA, reward 0.0000045 CnFA, refund 0 CnFA, net cost 0.0000005 CnFA) |
| 6 | 299500 | 299500 | 300001 | 5000 | 8.3 / 12.0 / 15.8 | 98 / 128 / 140 | 11 / 0 / 25 / 5 | 3904 | 395 | 6 / 6 (errori 0 / 0) |  |
snapshot all'altezza 3600: RSS subito prima 3904 MB, 1 minuto dopo 3924 MB (+20 MB)
| 7 | 299950 | 299950 | 300000 | 5000 | 8.2 / 11.6 / 14.0 | 99 / 128 / 139 | 11 / 0 / 25 / 16 | 3924 | 461 | 7 / 7 (errori 0 / 0) |  |
| 8 | 300000 | 300000 | 300000 | 5000 | 8.6 / 12.1 / 13.8 | 99 / 129 / 139 | 11 / 0 / 25 / 5 | 3924 | 526 | 8 / 8 (errori 0 / 0) |  |
| 9 | 300150 | 300150 | 300000 | 5000 | 8.5 / 12.2 / 17.4 | 99 / 129 / 141 | 11 / 0 / 25 / 5 | 3924 | 592 | 9 / 9 (errori 0 / 0) |  |
snapshot all'altezza 5400: RSS subito prima 3924 MB, 1 minuto dopo 3932 MB (+8 MB)
| 10 | 300550 | 300550 | 300000 | 5000 | 8.6 / 12.6 / 16.0 | 101 / 137 / 142 | 12 / 0 / 26 / 17 | 3933 | 658 | 10 / 10 (errori 0 / 0) | ok (fee 0.000005 CnFA, reward 0.0000045 CnFA, refund 0 CnFA, net cost 0.0000005 CnFA) |
| 11 | 299400 | 299400 | 300001 | 5000 | 8.7 / 12.5 / 13.8 | 99 / 129 / 142 | 12 / 0 / 25 / 5 | 3933 | 724 | 11 / 11 (errori 0 / 0) |  |
| 12 | 300000 | 300000 | 300000 | 5000 | 8.4 / 12.0 / 15.1 | 99 / 137 / 139 | 11 / 0 / 24 / 5 | 3933 | 789 | 12 / 12 (errori 0 / 0) |  |
snapshot all'altezza 7200: RSS subito prima 3933 MB, 1 minuto dopo 3933 MB (+0 MB)
| 13 | 300050 | 300050 | 300500 | 5008 | 8.5 / 12.5 / 15.2 | 100 / 128 / 141 | 12 / 0 / 25 / 17 | 3933 | 855 | 13 / 13 (errori 0 / 0) |  |
| 14 | 299850 | 299850 | 299500 | 4992 | 8.4 / 12.5 / 16.4 | 99 / 129 / 140 | 11 / 0 / 25 / 4 | 3933 | 921 | 14 / 14 (errori 0 / 0) |  |
| 15 | 300600 | 300600 | 300000 | 5000 | 8.5 / 12.8 / 16.5 | 99 / 128 / 138 | 12 / 0 / 25 / 5 | 3933 | 987 | 15 / 15 (errori 0 / 0) | ok (fee 0.000005 CnFA, reward 0.0000045 CnFA, refund 0 CnFA, net cost 0.0000005 CnFA) |
totale: inviate 4500850, accettate 4500850, rifiutate all'ingresso 0, incluse 4500853, non incluse 0
TPS effettivi medi 4999 (richiesti 5000); altezza 9007
blocco (tutta la prova): p50 8.5 / p95 12.6 / p99 16.5 / max 25.8 ms
RAM: all'inizio 476 MB, prima del nodo; alla fine 3933 MB; stati in RAM 150501
latenza scomposta (4498 campioni), p50 / p99 ms: invio -> eseguito 77 / 102; eseguito -> durevole 1 / 10; durevole -> pubblicato 2 / 5; pubblicato -> visto dal soak 19 / 38
dall'accodamento al journal alla durabilità: p50 3.1 / p99 12.8 / max 170.2 ms (il blocco sopra è il tempo sul thread del nodo, attesa per coda piena compresa)

| snapshot (altezza) | durata s | picco Dirty MB | blocchi nella finestra | sync_data p99 / max ms |
|---|---|---|---|---|
| 1800 | 9.7 | 8 | 96 | 124.0 / 125.7 |
| 3600 | 9.4 | 8 | 94 | 79.5 / 116.3 |
| 5400 | 9.5 | 8 | 94 | 89.4 / 124.5 |
| 7200 | 9.4 | 8 | 94 | 92.5 / 96.1 |

sync_data del journal: finestra reale degli snapshot (378 blocchi) p99 113.3 / max 125.7 ms; finestra fissa di 600 blocchi (2408 blocchi) p99 12.5 / max 125.7 ms; resto (6595 blocchi) p99 2.8 / max 167.0 ms; picco Dirty in tutta la prova 8 MB
invio→inclusione (4498 campioni, 1 ogni ~1000 tx, risoluzione ~40 ms): p50 100 / p95 136 / p99 142 / max 261 ms
CPU media (% di un core): nodo 11, rete 0, client 25, altro 10
RSS intorno agli snapshot (altezza: prima -> 1 min dopo): 1800: 3843 -> 3865, 3600: 3904 -> 3924, 5400: 3924 -> 3932, 7200: 3933 -> 3933
journal 987 MB = 219 byte/tx, 3946 MB/ora; snapshot 892 MB; status.bin 11.4 MB
stati: 15 trovati e 15 scaduti come atteso (errori 0 / 0); wallet sotto carico: 3 ok, 0 errori
snapshot durante il carico: 5 (pausa sul thread del nodo: codifica degli stati e invio allo stato ombra; copia e scrittura sono in background: 0 ms, 0 ms, 0 ms, 0 ms, 0 ms), massimo 0 ms
blocchi oltre 100 ms: 0 
snapshot saltati (scrittura in corso, coda piena o ombra persa): 0; ultima scrittura 9415 ms; coda massima durante una scrittura 91 blocchi
FINE
