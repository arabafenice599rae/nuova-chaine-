# RPC 10M Poisson, misura del thread del nodo (prima)
macchina: 4 vCPU, Intel(R) Xeon(R) Processor @ 2.30GHz; commit ?; avvio 1794046265810 ms (unix)
avvio: snapshot iniziale su disco, Dirty 0 MB dopo 0.0 s di attesa
10000000 account, 5000 tx/s richiesti, 15 minuti, invio via HTTP/JSON-RPC (end-to-end), rpc_recent_txs 20000, snapshot ogni 1800 blocchi, stream_catchup_blocks 36000
CPU in % di un core: nodo = thread dei blocchi; rete = runtime tokio (RPC e stream, lato nodo); client = thread che firmano e inviano (in modalità diretta includono la verifica delle firme all'ingresso)
| min | inviate | accettate | incluse | TPS effettivi | blocco p50 / p95 / p99 ms | invio→inclusione p50 / p95 / p99 ms | CPU nodo / rete / client / altro % | RSS MB | journal MB | stati ok (290 blocchi) / scaduti ok (>300) | wallet |
| 1 | 300453 | 300453 | 299976 | 5000 | 8.6 / 13.8 / 21.4 | 79 / 125 / 137 | 51 / 53 / 45 / 22 | 3637 | 66 | 1 / 1 (errori 0 / 0) |  |
| 2 | 299863 | 299863 | 300135 | 5002 | 8.2 / 12.4 / 14.0 | 78 / 124 / 136 | 50 / 54 / 44 / 4 | 3657 | 132 | 2 / 2 (errori 0 / 0) |  |
| 3 | 300246 | 300246 | 300309 | 5005 | 8.4 / 12.9 / 14.8 | 79 / 124 / 137 | 51 / 54 / 44 / 5 | 3724 | 197 | 3 / 3 (errori 0 / 0) |  |
snapshot all'altezza 1800: RSS subito prima 3724 MB, 1 minuto dopo 3744 MB (+20 MB)
| 4 | 299958 | 299958 | 299584 | 4993 | 8.9 / 13.5 / 18.2 | 80 / 127 / 141 | 52 / 53 / 44 / 15 | 3744 | 263 | 4 / 4 (errori 0 / 0) |  |
| 5 | 300135 | 300135 | 299993 | 5000 | 8.7 / 13.1 / 14.1 | 80 / 127 / 139 | 52 / 55 / 46 / 5 | 3765 | 329 | 5 / 5 (errori 0 / 0) | ok (fee 0.000005 CnFA, reward 0.0000045 CnFA, refund 0 CnFA, net cost 0.0000005 CnFA) |
| 6 | 298908 | 298908 | 299013 | 4984 | 8.9 / 13.4 / 14.3 | 79 / 125 / 138 | 52 / 56 / 46 / 5 | 3790 | 395 | 6 / 6 (errori 0 / 0) |  |
snapshot all'altezza 3600: RSS subito prima 3789 MB, 1 minuto dopo 3810 MB (+21 MB)
| 7 | 299515 | 299515 | 299985 | 5000 | 8.8 / 13.3 / 15.9 | 81 / 127 / 142 | 51 / 54 / 44 / 13 | 3810 | 460 | 7 / 7 (errori 0 / 0) |  |
| 8 | 299773 | 299773 | 299320 | 4989 | 8.5 / 13.0 / 15.8 | 77 / 123 / 136 | 50 / 54 / 44 / 4 | 3811 | 526 | 8 / 8 (errori 0 / 0) |  |
| 9 | 299145 | 299145 | 299130 | 4986 | 8.4 / 12.3 / 13.7 | 77 / 123 / 134 | 50 / 54 / 43 / 5 | 3812 | 592 | 9 / 9 (errori 0 / 0) |  |
snapshot all'altezza 5400: RSS subito prima 3812 MB, 1 minuto dopo 3821 MB (+9 MB)
| 10 | 300365 | 300365 | 300292 | 5005 | 8.5 / 13.1 / 17.7 | 80 / 127 / 144 | 50 / 52 / 42 / 13 | 3821 | 658 | 10 / 10 (errori 0 / 0) | ok (fee 0.000005 CnFA, reward 0.0000045 CnFA, refund 0 CnFA, net cost 0.0000005 CnFA) |
| 11 | 298789 | 298789 | 298890 | 4982 | 8.3 / 12.4 / 14.5 | 77 / 125 / 137 | 49 / 54 / 43 / 4 | 3821 | 723 | 11 / 11 (errori 0 / 0) |  |
| 12 | 300449 | 300449 | 300867 | 5014 | 8.5 / 12.9 / 14.6 | 77 / 124 / 136 | 50 / 54 / 43 / 5 | 3831 | 789 | 12 / 12 (errori 0 / 0) |  |
snapshot all'altezza 7200: RSS subito prima 3831 MB, 1 minuto dopo 3831 MB (+0 MB)
| 13 | 300497 | 300497 | 300039 | 5001 | 8.4 / 12.8 / 17.2 | 79 / 126 / 143 | 50 / 52 / 42 / 13 | 3831 | 855 | 13 / 13 (errori 0 / 0) |  |
| 14 | 299930 | 299930 | 300413 | 5007 | 8.4 / 12.4 / 14.2 | 79 / 124 / 136 | 50 / 53 / 42 / 4 | 3832 | 921 | 14 / 14 (errori 0 / 0) |  |
| 15 | 300731 | 300731 | 300139 | 5002 | 8.4 / 12.4 / 14.4 | 75 / 123 / 135 | 50 / 54 / 43 / 5 | 3832 | 987 | 15 / 15 (errori 0 / 0) | ok (fee 0.000005 CnFA, reward 0.0000045 CnFA, refund 0 CnFA, net cost 0.0000005 CnFA) |
totale: inviate 4498779, accettate 4498779, rifiutate all'ingresso 0, incluse 4498782, non incluse 0
TPS effettivi medi 4996 (richiesti 5000); altezza 9009
blocco (tutta la prova): p50 8.5 / p95 13.0 / p99 15.6 / max 57.3 ms
RAM: all'inizio 474 MB, prima del nodo; alla fine 3832 MB; stati in RAM 150288
latenza scomposta (71961 campioni), p50 / p99 ms: invio -> eseguito 60 / 108; eseguito -> durevole 1 / 11; durevole -> pubblicato 3 / 5; pubblicato -> visto dal soak 10 / 38
istogramma invio -> eseguito (inizio del passo da 10 ms: campioni; 300 = oltre): 0:539 10:6719 20:7266 30:7041 40:7151 50:7123 60:7305 70:7163 80:8562 90:9869 100:2889 110:323 120:6 130:0 140:3 150:1 160:0 170:1 180:0 190:0 200:0 210:0 220:0 230:0 240:0 250:0 260:0 270:0 280:0 290:0 300:0
fase dell'invio dopo l'ultimo blocco eseguito (passi da 10 ms): 0:10366 10:8406 20:7203 30:7278 40:7224 50:7080 60:6986 70:7265 80:7107 90:3018 100:28 110:0 120:0 130:0 140:0 150:0 160:0 170:0 180:0 190:0 200:0 210:0 220:0 230:0 240:0 250:0 260:0 270:0 280:0 290:0 300:0
blocchi eseguiti tra l'invio e il blocco della tx (0 = il primo utile): [69803, 2158, 0, 0, 0, 0]
thread del nodo per blocco (9004 blocchi), ms: query 38.30 (decodifica tx 0.98, firme + coda 31.17); produzione 9.23 (esecuzione 6.68, hash + firma 1.81); decodifica + firme = 67.6% di query + produzione
dall'accodamento al journal alla durabilità: p50 3.6 / p99 13.5 / max 294.6 ms (il blocco sopra è il tempo sul thread del nodo, attesa per coda piena compresa)

| snapshot (altezza) | durata s | picco Dirty MB | blocchi nella finestra | sync_data p99 / max ms |
|---|---|---|---|---|
| 1800 | 9.2 | 8 | 91 | 17.2 / 52.4 |
| 3600 | 9.4 | 7 | 93 | 55.7 / 107.8 |
| 5400 | 9.4 | 7 | 94 | 61.2 / 123.3 |
| 7200 | 9.4 | 7 | 94 | 157.4 / 233.4 |

sync_data del journal: finestra reale degli snapshot (372 blocchi) p99 107.8 / max 233.4 ms; finestra fissa di 600 blocchi (2410 blocchi) p99 12.7 / max 233.4 ms; resto (6594 blocchi) p99 4.0 / max 69.1 ms; picco Dirty in tutta la prova 8 MB
invio→inclusione (71961 campioni, 1 ogni ~1000 tx, risoluzione ~40 ms): p50 79 / p95 125 / p99 138 / max 405 ms
CPU media (% di un core): nodo 51, rete 54, client 44, altro 8
RSS intorno agli snapshot (altezza: prima -> 1 min dopo): 1800: 3724 -> 3744, 3600: 3789 -> 3810, 5400: 3812 -> 3821, 7200: 3831 -> 3831
journal 987 MB = 219 byte/tx, 3944 MB/ora; snapshot 892 MB; status.bin 11.4 MB
stati: 15 trovati e 15 scaduti come atteso (errori 0 / 0); wallet sotto carico: 3 ok, 0 errori
snapshot durante il carico: 5 (pausa sul thread del nodo: codifica degli stati e invio allo stato ombra; copia e scrittura sono in background: 0 ms, 0 ms, 0 ms, 0 ms, 0 ms), massimo 0 ms
blocchi oltre 100 ms: 0 
snapshot saltati (scrittura in corso, coda piena o ombra persa): 0; ultima scrittura 9421 ms; coda massima durante una scrittura 89 blocchi
