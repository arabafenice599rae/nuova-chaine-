# soak snapshot da stato ombra 1M

comando: `SNAPSHOT_EVERY=600 RESULTS=docs/soak/snapshot-ombra-1m-5000-direct.md LOAD_DIR=<dir> target/release/examples/soak 1000000 5000 6`, poi `soak --reopen <dir>` (commit 5045c9b)

macchina: 4 vCPU, Intel(R) Xeon(R) Processor @ 2.10GHz; commit 5045c9b; avvio 1793973409027 ms (unix)
1000000 account, 5000 tx/s richiesti, 6 minuti, invio diretto all'ingresso (limite del nodo), rpc_recent_txs 20000, snapshot ogni 600 blocchi, stream_catchup_blocks 36000
CPU in % di un core: nodo = thread dei blocchi; rete = runtime tokio (RPC e stream, lato nodo); client = thread che firmano e inviano (in modalità diretta includono la verifica delle firme all'ingresso)
| min | inviate | accettate | incluse | TPS effettivi | blocco p50 / p95 / p99 ms | invio→inclusione p50 / p95 / p99 ms | CPU nodo / rete / client / altro % | RSS MB | journal MB | stati ok (290 blocchi) / scaduti ok (>300) | wallet |
| 1 | 300450 | 300450 | 300300 | 5005 | 13.1 / 26.2 / 46.8 | 78 / 117 / 121 | 15 / 0 / 30 / 3 | 1044 | 66 | 1 / 1 (errori 0 / 0) |  |
snapshot all'altezza 600: RSS subito prima 914 MB, 1 minuto dopo 982 MB (+68 MB)
| 2 | 300000 | 300000 | 299650 | 4994 | 12.5 / 18.5 / 24.0 | 76 / 118 / 206 | 13 / 0 / 30 / 4 | 1094 | 132 | 2 / 2 (errori 0 / 0) |  |
snapshot all'altezza 1200: RSS subito prima 935 MB, 1 minuto dopo 987 MB (+52 MB)
| 3 | 299850 | 299850 | 300000 | 5000 | 11.8 / 16.2 / 21.6 | 61 / 118 / 121 | 12 / 0 / 29 / 4 | 1122 | 197 | 3 / 3 (errori 0 / 0) |  |
snapshot all'altezza 1800: RSS subito prima 955 MB, 1 minuto dopo 988 MB (+33 MB)
| 4 | 300000 | 300000 | 300000 | 5000 | 12.0 / 16.1 / 18.7 | 60 / 119 / 122 | 12 / 0 / 29 / 3 | 1122 | 263 | 4 / 4 (errori 0 / 0) |  |
snapshot all'altezza 2400: RSS subito prima 975 MB, 1 minuto dopo 1011 MB (+36 MB)
| 5 | 300750 | 300750 | 300000 | 5000 | 12.1 / 17.0 / 22.8 | 60 / 120 / 140 | 12 / 0 / 29 / 4 | 1160 | 329 | 5 / 5 (errori 0 / 0) | ok (fee 0.000005 CnFA, reward 0.0000045 CnFA, refund 0 CnFA, net cost 0.0000005 CnFA) |
snapshot all'altezza 3000: RSS subito prima 995 MB, 1 minuto dopo 1032 MB (+37 MB)
| 6 | 299400 | 299400 | 300001 | 5000 | 12.5 / 17.2 / 20.4 | 63 / 120 / 141 | 13 / 0 / 30 / 4 | 1168 | 395 | 6 / 6 (errori 0 / 0) |  |
totale: inviate 1800450, accettate 1800450, rifiutate all'ingresso 0, incluse 1800451, non incluse 0
TPS effettivi medi 4996 (richiesti 5000); altezza 3607
blocco (tutta la prova): p50 12.3 / p95 18.1 / p99 30.5 / max 315.7 ms
RAM: all'inizio 85 MB, prima del nodo; alla fine 1224 MB; stati in RAM 150500
invio→inclusione (1799 campioni, 1 ogni ~1000 tx, risoluzione ~40 ms): p50 66 / p95 119 / p99 140 / max 427 ms
CPU media (% di un core): nodo 13, rete 0, client 29, altro 4
RSS intorno agli snapshot (altezza: prima -> 1 min dopo): 600: 914 -> 982, 1200: 935 -> 987, 1800: 955 -> 988, 2400: 975 -> 1011, 3000: 995 -> 1032
journal 395 MB = 219 byte/tx, 3944 MB/ora; snapshot 91 MB; status.bin 11.4 MB
stati: 6 trovati e 6 scaduti come atteso (errori 0 / 0); wallet sotto carico: 1 ok, 0 errori
snapshot durante il carico: 6 (pausa sul thread del nodo: codifica degli stati e invio allo stato ombra; copia e scrittura sono in background: 95 ms, 97 ms, 56 ms, 85 ms, 93 ms, 93 ms), massimo 97 ms
snapshot saltati (il precedente era ancora in scrittura): 0
riavvio da snapshot (altezza 3000) + journal: 16.6 s, altezza 3607, hash uguali
