# A/B 10M account, caso a

comando: `SNAPSHOT_EVERY=1800 RESULTS=docs/soak/ab-10m-a.md LOAD_DIR=<dir> soak 10000000 5000 15` (commit main+diag 2d7a7d9); sysctl: 0 0 10 20 (dirty_background_bytes dirty_bytes dirty_background_ratio dirty_ratio)

macchina: 4 vCPU, Intel(R) Xeon(R) Processor @ 2.10GHz; commit main+diag 2d7a7d9; avvio 1793982669665 ms (unix)
10000000 account, 5000 tx/s richiesti, 15 minuti, invio diretto all'ingresso (limite del nodo), rpc_recent_txs 20000, snapshot ogni 1800 blocchi, stream_catchup_blocks 36000
CPU in % di un core: nodo = thread dei blocchi; rete = runtime tokio (RPC e stream, lato nodo); client = thread che firmano e inviano (in modalità diretta includono la verifica delle firme all'ingresso)
| min | inviate | accettate | incluse | TPS effettivi | blocco p50 / p95 / p99 ms | invio→inclusione p50 / p95 / p99 ms | CPU nodo / rete / client / altro % | RSS MB | journal MB | stati ok (290 blocchi) / scaduti ok (>300) | wallet |
| 1 | 300300 | 300300 | 299950 | 4999 | 12.5 / 17.2 / 21.6 | 67 / 122 / 140 | 13 / 0 / 31 / 0 | 2813 | 66 | 1 / 1 (errori 0 / 0) |  |
| 2 | 300250 | 300250 | 300500 | 5008 | 13.1 / 17.5 / 21.9 | 48 / 120 / 123 | 13 / 0 / 32 / 0 | 2832 | 132 | 2 / 2 (errori 0 / 0) |  |
| 3 | 343100 | 343100 | 296500 | 4942 | 12.8 / 17.6 / 20.3 | 47 / 121 / 8502 | 26 / 0 / 36 / 0 | 2893 | 200 | 3 / 3 (errori 0 / 0) |  |
| 4 | 256750 | 256750 | 303250 | 5054 | 13.6 / 18.7 / 39.4 | 90 / 5521 / 7919 | 13 / 0 / 28 / 0 | 2901 | 263 | 4 / 4 (errori 0 / 0) |  |
snapshot all'altezza 1800: RSS subito prima 2850 MB, 1 minuto dopo 2904 MB (+54 MB)
| 5 | 300400 | 300400 | 299950 | 4999 | 13.6 / 17.8 / 20.2 | 75 / 115 / 117 | 14 / 0 / 32 / 0 | 2920 | 329 | 5 / 5 (errori 0 / 0) | ok (fee 0.000005 CnFA, reward 0.0000045 CnFA, refund 0 CnFA, net cost 0.0000005 CnFA) |
| 6 | 299600 | 299600 | 300051 | 5001 | 14.9 / 18.9 / 20.5 | 76 / 116 / 117 | 15 / 0 / 32 / 0 | 2939 | 395 | 6 / 6 (errori 0 / 0) |  |
| 7 | 300000 | 300000 | 300100 | 5002 | 14.1 / 20.9 / 98.1 | 97 / 7670 / 9642 | 29 / 0 / 32 / 0 | 2962 | 461 | 7 / 7 (errori 0 / 0) |  |
snapshot all'altezza 3600: RSS subito prima 2941 MB, 1 minuto dopo 2962 MB (+21 MB)
| 8 | 300000 | 300000 | 299500 | 4992 | 12.8 / 17.3 / 19.8 | 95 / 135 / 141 | 13 / 0 / 30 / 0 | 2962 | 526 | 8 / 8 (errori 0 / 0) |  |
| 9 | 299900 | 299900 | 300000 | 5000 | 13.1 / 18.3 / 19.9 | 96 / 139 / 141 | 14 / 0 / 31 / 0 | 2962 | 592 | 9 / 9 (errori 0 / 0) |  |
| 10 | 300600 | 300600 | 300200 | 5003 | 13.9 / 20.2 / 106.1 | 94 / 6464 / 8634 | 26 / 0 / 32 / 0 | 2975 | 658 | 10 / 10 (errori 0 / 0) | ok (fee 0.000005 CnFA, reward 0.0000045 CnFA, refund 0 CnFA, net cost 0.0000005 CnFA) |
snapshot all'altezza 5400: RSS subito prima 2963 MB, 1 minuto dopo 2975 MB (+12 MB)
| 11 | 299400 | 299400 | 300001 | 5000 | 13.5 / 18.0 / 19.9 | 54 / 127 / 130 | 14 / 0 / 31 / 0 | 2975 | 724 | 11 / 11 (errori 0 / 0) |  |
| 12 | 300050 | 300050 | 300000 | 5000 | 13.0 / 18.1 / 19.9 | 56 / 128 / 129 | 13 / 0 / 31 / 0 | 2975 | 789 | 12 / 12 (errori 0 / 0) |  |
| 13 | 300050 | 300050 | 300300 | 5005 | 13.1 / 21.9 / 90.7 | 97 / 6790 / 8954 | 27 / 0 / 30 / 0 | 2975 | 855 | 13 / 13 (errori 0 / 0) |  |
snapshot all'altezza 7200: RSS subito prima 2975 MB, 1 minuto dopo 2975 MB (+0 MB)
| 14 | 299900 | 299900 | 299500 | 4992 | 12.5 / 17.1 / 19.7 | 95 / 137 / 140 | 13 / 0 / 30 / 0 | 2975 | 921 | 14 / 14 (errori 0 / 0) |  |
| 15 | 300600 | 300600 | 300500 | 5008 | 12.6 / 16.8 / 18.9 | 94 / 137 / 141 | 13 / 0 / 30 / 0 | 2975 | 987 | 15 / 15 (errori 0 / 0) | ok (fee 0.000005 CnFA, reward 0.0000045 CnFA, refund 0 CnFA, net cost 0.0000005 CnFA) |
totale: inviate 4500900, accettate 4500900, rifiutate all'ingresso 0, incluse 4500903, non incluse 0
TPS effettivi medi 4998 (richiesti 5000); altezza 8669
blocco (tutta la prova): p50 13.1 / p95 18.2 / p99 22.9 / max 10420.6 ms
RAM: all'inizio 474 MB, prima del nodo; alla fine 2976 MB; stati in RAM 150101

| snapshot (altezza) | durata s | picco Dirty MB | blocchi nella finestra | sync_data p99 / max ms |
|---|---|---|---|---|
| 1800 | 8.9 | 0 | 0 | 0.0 / 0.0 |
| 3600 | 10.4 | 0 | 0 | 0.0 / 0.0 |
| 5400 | 9.0 | 0 | 0 | 0.0 / 0.0 |
| 7200 | 9.6 | 0 | 0 | 0.0 / 0.0 |

sync_data del journal: finestra reale degli snapshot (0 blocchi) p99 0.0 / max 0.0 ms; finestra fissa di 600 blocchi (2360 blocchi) p99 2.2 / max 104.2 ms; resto (6263 blocchi) p99 2.7 / max 232.9 ms; picco Dirty in tutta la prova 850 MB

| causa dei blocchi > 100 ms | blocchi | max ms |
|---|---|---|
| altro | 28 | 219 |
| disco (sync_data) | 2 | 248 |
| snapshot | 4 | 10421 |

blocco 141 (disco (sync_data)): 248 ms = esecuzione 10 + hash 2 + firma 0 + write 0.1 + sync_data 232.9 + pubblicazione 0.0; minflt 172 majflt 0; attesa CPU 0 ms; mappa 10004289->10004289 (capacity 14680064->14680064)
blocco 143 (disco (sync_data)): 178 ms = esecuzione 14 + hash 4 + firma 0 + write 0.1 + sync_data 152.4 + pubblicazione 0.1; minflt 448 majflt 0; attesa CPU 0 ms; mappa 10004289->10004289 (capacity 14680064->14680064)
blocco 1800 (snapshot): 8935 ms = esecuzione 9 + hash 2 + firma 0 + write 0.1 + sync_data 0.9 + pubblicazione 0.2; minflt 61439 majflt 0; attesa CPU 2 ms; mappa 10004289->10004289 (capacity 14680064->14680064)
blocco 1810 (altro): 138 ms = esecuzione 81 + hash 25 + firma 2 + write 0.5 + sync_data 1.8 + pubblicazione 0.3; minflt 23 majflt 0; attesa CPU 0 ms; mappa 10004289->10004289 (capacity 14680064->14680064)
blocco 1811 (altro): 122 ms = esecuzione 73 + hash 20 + firma 1 + write 0.4 + sync_data 2.0 + pubblicazione 0.3; minflt 1 majflt 0; attesa CPU 0 ms; mappa 10004289->10004289 (capacity 14680064->14680064)
blocco 1812 (altro): 141 ms = esecuzione 86 + hash 25 + firma 2 + write 0.4 + sync_data 1.8 + pubblicazione 0.2; minflt 103 majflt 0; attesa CPU 0 ms; mappa 10004289->10004289 (capacity 14680064->14680064)
blocco 1813 (altro): 219 ms = esecuzione 63 + hash 24 + firma 2 + write 0.4 + sync_data 104.2 + pubblicazione 0.3; minflt 275 majflt 0; attesa CPU 0 ms; mappa 10004289->10004289 (capacity 14680064->14680064)
blocco 1815 (altro): 121 ms = esecuzione 65 + hash 21 + firma 1 + write 0.4 + sync_data 13.6 + pubblicazione 0.2; minflt 381 majflt 0; attesa CPU 0 ms; mappa 10004289->10004289 (capacity 14680064->14680064)
blocco 1816 (altro): 103 ms = esecuzione 59 + hash 20 + firma 1 + write 1.4 + sync_data 2.0 + pubblicazione 0.2; minflt 0 majflt 0; attesa CPU 0 ms; mappa 10004289->10004289 (capacity 14680064->14680064)
blocco 1818 (altro): 105 ms = esecuzione 58 + hash 20 + firma 1 + write 0.9 + sync_data 1.9 + pubblicazione 0.2; minflt 49 majflt 0; attesa CPU 0 ms; mappa 10004289->10004289 (capacity 14680064->14680064)
blocco 1819 (altro): 125 ms = esecuzione 79 + hash 20 + firma 1 + write 4.2 + sync_data 1.7 + pubblicazione 0.2; minflt 115 majflt 0; attesa CPU 0 ms; mappa 10004289->10004289 (capacity 14680064->14680064)
blocco 1820 (altro): 105 ms = esecuzione 59 + hash 19 + firma 1 + write 4.1 + sync_data 2.5 + pubblicazione 0.2; minflt 177 majflt 0; attesa CPU 0 ms; mappa 10004289->10004289 (capacity 14680064->14680064)
blocco 3600 (snapshot): 10421 ms = esecuzione 7 + hash 2 + firma 0 + write 0.4 + sync_data 0.8 + pubblicazione 0.1; minflt 61329 majflt 0; attesa CPU 2 ms; mappa 10004290->10004290 (capacity 14680064->14680064)
blocco 3610 (altro): 145 ms = esecuzione 87 + hash 26 + firma 2 + write 0.4 + sync_data 1.8 + pubblicazione 0.3; minflt 0 majflt 0; attesa CPU 0 ms; mappa 10004290->10004290 (capacity 14680064->14680064)
blocco 3611 (altro): 112 ms = esecuzione 70 + hash 18 + firma 1 + write 0.4 + sync_data 2.1 + pubblicazione 0.2; minflt 1 majflt 0; attesa CPU 0 ms; mappa 10004290->10004290 (capacity 14680064->14680064)
blocco 3612 (altro): 103 ms = esecuzione 61 + hash 18 + firma 1 + write 0.4 + sync_data 1.9 + pubblicazione 0.2; minflt 186 majflt 0; attesa CPU 0 ms; mappa 10004290->10004290 (capacity 14680064->14680064)
blocco 3614 (altro): 100 ms = esecuzione 57 + hash 19 + firma 1 + write 3.3 + sync_data 2.0 + pubblicazione 0.2; minflt 254 majflt 0; attesa CPU 0 ms; mappa 10004290->10004290 (capacity 14680064->14680064)
blocco 3617 (altro): 101 ms = esecuzione 55 + hash 19 + firma 1 + write 3.3 + sync_data 1.9 + pubblicazione 0.2; minflt 175 majflt 0; attesa CPU 0 ms; mappa 10004290->10004290 (capacity 14680064->14680064)
blocco 3618 (altro): 102 ms = esecuzione 58 + hash 19 + firma 1 + write 3.4 + sync_data 1.6 + pubblicazione 0.2; minflt 255 majflt 0; attesa CPU 0 ms; mappa 10004290->10004290 (capacity 14680064->14680064)
blocco 3619 (altro): 102 ms = esecuzione 56 + hash 18 + firma 1 + write 3.2 + sync_data 1.8 + pubblicazione 0.2; minflt 73 majflt 0; attesa CPU 2 ms; mappa 10004290->10004290 (capacity 14680064->14680064)
blocco 3620 (altro): 107 ms = esecuzione 62 + hash 21 + firma 1 + write 3.3 + sync_data 1.8 + pubblicazione 0.2; minflt 225 majflt 0; attesa CPU 1 ms; mappa 10004290->10004290 (capacity 14680064->14680064)
blocco 5400 (snapshot): 8974 ms = esecuzione 9 + hash 2 + firma 0 + write 0.1 + sync_data 0.8 + pubblicazione 0.1; minflt 60545 majflt 0; attesa CPU 2 ms; mappa 10004290->10004290 (capacity 14680064->14680064)
blocco 5410 (altro): 180 ms = esecuzione 73 + hash 27 + firma 2 + write 0.4 + sync_data 43.7 + pubblicazione 0.2; minflt 1 majflt 0; attesa CPU 0 ms; mappa 10004290->10004290 (capacity 14680064->14680064)
blocco 5412 (altro): 109 ms = esecuzione 65 + hash 20 + firma 1 + write 0.4 + sync_data 1.7 + pubblicazione 0.2; minflt 0 majflt 0; attesa CPU 0 ms; mappa 10004290->10004290 (capacity 14680064->14680064)
blocco 5413 (altro): 112 ms = esecuzione 60 + hash 20 + firma 1 + write 0.4 + sync_data 1.9 + pubblicazione 0.3; minflt 274 majflt 0; attesa CPU 0 ms; mappa 10004290->10004290 (capacity 14680064->14680064)
blocco 5414 (altro): 117 ms = esecuzione 72 + hash 19 + firma 1 + write 0.4 + sync_data 1.5 + pubblicazione 0.2; minflt 343 majflt 0; attesa CPU 0 ms; mappa 10004290->10004290 (capacity 14680064->14680064)
blocco 5415 (altro): 127 ms = esecuzione 74 + hash 27 + firma 1 + write 0.4 + sync_data 1.9 + pubblicazione 0.2; minflt 227 majflt 0; attesa CPU 0 ms; mappa 10004290->10004290 (capacity 14680064->14680064)
blocco 5416 (altro): 163 ms = esecuzione 100 + hash 28 + firma 1 + write 0.4 + sync_data 1.9 + pubblicazione 0.3; minflt 178 majflt 0; attesa CPU 0 ms; mappa 10004290->10004290 (capacity 14680064->14680064)
blocco 5418 (altro): 156 ms = esecuzione 91 + hash 27 + firma 2 + write 0.4 + sync_data 2.2 + pubblicazione 0.7; minflt 2 majflt 0; attesa CPU 0 ms; mappa 10004290->10004290 (capacity 14680064->14680064)
blocco 5420 (altro): 131 ms = esecuzione 81 + hash 24 + firma 1 + write 0.4 + sync_data 1.5 + pubblicazione 0.2; minflt 314 majflt 0; attesa CPU 0 ms; mappa 10004290->10004290 (capacity 14680064->14680064)
blocco 5421 (altro): 112 ms = esecuzione 67 + hash 20 + firma 1 + write 0.4 + sync_data 1.9 + pubblicazione 0.2; minflt 45 majflt 0; attesa CPU 0 ms; mappa 10004290->10004290 (capacity 14680064->14680064)
blocco 5422 (altro): 106 ms = esecuzione 63 + hash 20 + firma 1 + write 0.4 + sync_data 1.8 + pubblicazione 0.3; minflt 666 majflt 0; attesa CPU 0 ms; mappa 10004290->10004290 (capacity 14680064->14680064)
blocco 7200 (snapshot): 9655 ms = esecuzione 8 + hash 2 + firma 0 + write 0.5 + sync_data 1.0 + pubblicazione 0.1; minflt 58620 majflt 0; attesa CPU 2 ms; mappa 10004291->10004291 (capacity 14680064->14680064)
blocco 7210 (altro): 103 ms = esecuzione 59 + hash 18 + firma 1 + write 0.4 + sync_data 1.7 + pubblicazione 0.2; minflt 0 majflt 0; attesa CPU 0 ms; mappa 10004291->10004291 (capacity 14680064->14680064)
invio→inclusione (4500 campioni, 1 ogni ~1000 tx, risoluzione ~40 ms): p50 90 / p95 141 / p99 7775 / max 10243 ms
CPU media (% di un core): nodo 17, rete 0, client 31, altro 0
RSS intorno agli snapshot (altezza: prima -> 1 min dopo): 1800: 2850 -> 2904, 3600: 2941 -> 2962, 5400: 2963 -> 2975, 7200: 2975 -> 2975
journal 987 MB = 219 byte/tx, 3946 MB/ora; snapshot 892 MB; status.bin 11.4 MB
stati: 15 trovati e 15 scaduti come atteso (errori 0 / 0); wallet sotto carico: 3 ok, 0 errori
snapshot durante il carico: 4 (scrittura dello snapshot nel suo blocco: 8920 ms, 10407 ms, 8959 ms, 9639 ms), massimo 10407 ms
FINE
