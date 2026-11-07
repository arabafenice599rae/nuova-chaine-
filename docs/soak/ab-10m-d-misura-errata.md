# A/B 10M account, caso d

comando: `SNAPSHOT_EVERY=1800 RESULTS=docs/soak/ab-10m-d.md LOAD_DIR=<dir> soak 10000000 5000 15` (commit PR B su #13 cd0db31); sysctl: 0 0 10 20 (dirty_background_bytes dirty_bytes dirty_background_ratio dirty_ratio)

macchina: 4 vCPU, Intel(R) Xeon(R) Processor @ 2.10GHz; commit PR B su #13 cd0db31; avvio 1793986814298 ms (unix)
10000000 account, 5000 tx/s richiesti, 15 minuti, invio diretto all'ingresso (limite del nodo), rpc_recent_txs 20000, snapshot ogni 1800 blocchi, stream_catchup_blocks 36000
CPU in % di un core: nodo = thread dei blocchi; rete = runtime tokio (RPC e stream, lato nodo); client = thread che firmano e inviano (in modalità diretta includono la verifica delle firme all'ingresso)
| min | inviate | accettate | incluse | TPS effettivi | blocco p50 / p95 / p99 ms | invio→inclusione p50 / p95 / p99 ms | CPU nodo / rete / client / altro % | RSS MB | journal MB | stati ok (290 blocchi) / scaduti ok (>300) | wallet |
| 1 | 300450 | 300450 | 299850 | 4998 | 13.5 / 21.0 / 35.5 | 97 / 212 / 225 | 14 / 0 / 31 / 25 | 3801 | 66 | 1 / 1 (errori 0 / 0) |  |
| 2 | 300000 | 300000 | 300450 | 5008 | 13.8 / 18.8 / 21.4 | 95 / 193 / 215 | 13 / 0 / 30 / 5 | 3817 | 132 | 2 / 2 (errori 0 / 0) |  |
| 3 | 300000 | 300000 | 299550 | 4992 | 13.9 / 19.0 / 22.8 | 94 / 192 / 215 | 13 / 0 / 30 / 6 | 3882 | 197 | 3 / 3 (errori 0 / 0) |  |
snapshot all'altezza 1800: RSS subito prima 3881 MB, 1 minuto dopo 3903 MB (+22 MB)
| 4 | 300000 | 300000 | 300450 | 5008 | 15.3 / 23.2 / 29.9 | 98 / 213 / 220 | 14 / 0 / 31 / 19 | 3903 | 263 | 4 / 4 (errori 0 / 0) |  |
| 5 | 300450 | 300450 | 299550 | 4992 | 13.9 / 19.2 / 20.2 | 95 / 190 / 213 | 14 / 0 / 30 / 5 | 3923 | 329 | 5 / 5 (errori 0 / 0) | ok (fee 0.000005 CnFA, reward 0.0000045 CnFA, refund 0 CnFA, net cost 0.0000005 CnFA) |
| 6 | 299550 | 299550 | 300001 | 5000 | 13.8 / 18.3 / 22.3 | 95 / 190 / 214 | 13 / 0 / 29 / 5 | 3944 | 395 | 6 / 6 (errori 0 / 0) |  |
snapshot all'altezza 3600: RSS subito prima 3944 MB, 1 minuto dopo 3964 MB (+20 MB)
| 7 | 299850 | 299850 | 300000 | 5000 | 14.4 / 21.8 / 29.0 | 97 / 213 / 221 | 14 / 0 / 29 / 18 | 3964 | 461 | 7 / 7 (errori 0 / 0) |  |
| 8 | 300100 | 300100 | 300450 | 5008 | 13.8 / 19.0 / 21.3 | 94 / 138 / 212 | 13 / 0 / 29 / 5 | 3964 | 526 | 8 / 8 (errori 0 / 0) |  |
| 9 | 299900 | 299900 | 299550 | 4992 | 13.8 / 19.0 / 22.8 | 94 / 141 / 213 | 13 / 0 / 29 / 5 | 3964 | 592 | 9 / 9 (errori 0 / 0) |  |
snapshot all'altezza 5400: RSS subito prima 3964 MB, 1 minuto dopo 3976 MB (+12 MB)
| 10 | 300600 | 300600 | 300000 | 5000 | 14.2 / 21.8 / 28.7 | 96 / 200 / 222 | 13 / 0 / 29 / 17 | 3976 | 658 | 10 / 10 (errori 0 / 0) | ok (fee 0.000005 CnFA, reward 0.0000045 CnFA, refund 0 CnFA, net cost 0.0000005 CnFA) |
| 11 | 299400 | 299400 | 299951 | 4999 | 13.9 / 19.6 / 21.6 | 95 / 211 / 214 | 14 / 0 / 29 / 5 | 3976 | 724 | 11 / 11 (errori 0 / 0) |  |
| 12 | 300100 | 300100 | 300500 | 5008 | 13.6 / 18.8 / 21.8 | 96 / 193 / 214 | 13 / 0 / 29 / 5 | 3976 | 790 | 12 / 12 (errori 0 / 0) |  |
snapshot all'altezza 7200: RSS subito prima 3976 MB, 1 minuto dopo 3976 MB (+0 MB)
| 13 | 299900 | 299900 | 299550 | 4992 | 15.5 / 21.4 / 30.0 | 96 / 213 / 216 | 14 / 0 / 30 / 17 | 3976 | 855 | 13 / 13 (errori 0 / 0) |  |
| 14 | 300000 | 300000 | 300000 | 5000 | 14.6 / 20.1 / 22.0 | 96 / 195 / 216 | 14 / 0 / 30 / 5 | 3976 | 921 | 14 / 14 (errori 0 / 0) |  |
| 15 | 300600 | 300600 | 300000 | 5000 | 14.5 / 19.8 / 23.2 | 95 / 211 / 215 | 14 / 0 / 30 / 6 | 3976 | 987 | 15 / 15 (errori 0 / 0) | ok (fee 0.000005 CnFA, reward 0.0000045 CnFA, refund 0 CnFA, net cost 0.0000005 CnFA) |
totale: inviate 4500900, accettate 4500900, rifiutate all'ingresso 0, incluse 4500903, non incluse 0
TPS effettivi medi 4998 (richiesti 5000); altezza 9009
blocco (tutta la prova): p50 14.1 / p95 20.1 / p99 25.7 / max 649.9 ms
RAM: all'inizio 475 MB, prima del nodo; alla fine 3976 MB; stati in RAM 150101

| snapshot (altezza) | durata s | picco Dirty MB | blocchi nella finestra | sync_data p99 / max ms |
|---|---|---|---|---|
| 1800 | 10.1 | 7 | 99 | 117.6 / 227.7 |
| 3600 | 9.8 | 8 | 97 | 145.5 / 184.9 |
| 5400 | 9.9 | 8 | 96 | 160.4 / 245.4 |
| 7200 | 9.5 | 8 | 94 | 165.2 / 201.8 |

sync_data del journal: finestra reale degli snapshot (386 blocchi) p99 165.2 / max 245.4 ms; finestra fissa di 600 blocchi (2410 blocchi) p99 12.3 / max 245.4 ms; resto (6592 blocchi) p99 2.6 / max 638.8 ms; picco Dirty in tutta la prova 850 MB
invio→inclusione (4497 campioni, 1 ogni ~1000 tx, risoluzione ~40 ms): p50 96 / p95 198 / p99 216 / max 784 ms
CPU media (% di un core): nodo 14, rete 0, client 30, altro 10
RSS intorno agli snapshot (altezza: prima -> 1 min dopo): 1800: 3881 -> 3903, 3600: 3944 -> 3964, 5400: 3964 -> 3976, 7200: 3976 -> 3976
journal 987 MB = 219 byte/tx, 3946 MB/ora; snapshot 892 MB; status.bin 11.4 MB
stati: 15 trovati e 15 scaduti come atteso (errori 0 / 0); wallet sotto carico: 3 ok, 0 errori
snapshot durante il carico: 5 (pausa sul thread del nodo: codifica degli stati e invio allo stato ombra; copia e scrittura sono in background: 0 ms, 0 ms, 0 ms, 0 ms, 0 ms), massimo 0 ms
blocchi oltre 100 ms: 16 [60: 650 ms = esecuzione 6 + hash 2 + firma 0 + journal 639 + altro 4] [61: 598 ms = esecuzione 7 + hash 2 + firma 0 + journal 25 + altro 565] [62: 500 ms = esecuzione 6 + hash 2 + firma 0 + journal 25 + altro 468] [1898: 247 ms = esecuzione 11 + hash 3 + firma 0 + journal 228 + altro 5] [1899: 266 ms = esecuzione 6 + hash 2 + firma 0 + journal 118 + altro 140] [1900: 168 ms = esecuzione 6 + hash 1 + firma 0 + journal 118 + altro 43] [3695: 197 ms = esecuzione 6 + hash 2 + firma 0 + journal 185 + altro 4] [3696: 242 ms = esecuzione 7 + hash 2 + firma 0 + journal 146 + altro 88] [3697: 148 ms = esecuzione 6 + hash 1 + firma 0 + journal 6 + altro 136] [5495: 173 ms = esecuzione 7 + hash 2 + firma 0 + journal 161 + altro 4] [5496: 318 ms = esecuzione 8 + hash 2 + firma 0 + journal 246 + altro 63] [5497: 233 ms = esecuzione 9 + hash 2 + firma 0 + journal 14 + altro 206] [5498: 135 ms = esecuzione 7 + hash 2 + firma 0 + journal 14 + altro 111] [7291: 215 ms = esecuzione 7 + hash 2 + firma 0 + journal 202 + altro 5] [7292: 280 ms = esecuzione 7 + hash 2 + firma 0 + journal 166 + altro 106] [7293: 182 ms = esecuzione 7 + hash 2 + firma 0 + journal 166 + altro 8]
snapshot saltati (scrittura in corso, coda piena o ombra persa): 0; ultima scrittura 9468 ms; coda massima durante una scrittura 96 blocchi
FINE

NOTA: in questo run la latenza invio→inclusione è misurata solo quando cambia l'altezza eseguita (+~100 ms fissi con il journal su un altro thread) e "blocco" include l'attesa della durabilità. Ripetuto come ab-10m-d.md con la misura corretta.
