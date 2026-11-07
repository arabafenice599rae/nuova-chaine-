# RPC 10M Poisson, thread del nodo, dopo la PR (pool di verifica), preflight
macchina: 4 vCPU, Intel(R) Xeon(R) Processor @ 2.30GHz; commit ?; avvio 1794058713433 ms (unix)
avvio: snapshot iniziale su disco, Dirty 0 MB dopo 0.0 s di attesa
10000000 account, 5000 tx/s richiesti, 15 minuti, invio via HTTP/JSON-RPC (end-to-end), rpc_recent_txs 20000, snapshot ogni 1800 blocchi, stream_catchup_blocks 36000
CPU in % di un core: nodo = thread dei blocchi; rete = runtime tokio (RPC e stream, lato nodo); client = thread che firmano e inviano (in modalità diretta includono la verifica delle firme all'ingresso)
| min | inviate | accettate | incluse | TPS effettivi | blocco p50 / p95 / p99 ms | invio→inclusione p50 / p95 / p99 ms | CPU nodo / rete / client / altro % | RSS MB | journal MB | stati ok (290 blocchi) / scaduti ok (>300) | wallet |
| 1 | 300505 | 300505 | 300014 | 5000 | 7.1 / 10.9 / 13.4 | 79 / 126 / 138 | 25 / 61 / 43 / 57 | 3891 | 66 | 1 / 1 (errori 0 / 0) |  |
| 2 | 299543 | 299543 | 299624 | 4994 | 7.9 / 12.4 / 16.0 | 79 / 125 / 136 | 27 / 65 / 46 / 45 | 3912 | 132 | 2 / 2 (errori 0 / 0) |  |
| 3 | 300377 | 300377 | 300327 | 5005 | 7.7 / 10.9 / 13.0 | 79 / 127 / 137 | 26 / 64 / 45 / 45 | 3980 | 197 | 3 / 3 (errori 0 / 0) |  |
snapshot all'altezza 1800: RSS subito prima 3980 MB, 1 minuto dopo 4000 MB (+20 MB)
| 4 | 300014 | 300014 | 300085 | 5001 | 7.6 / 11.2 / 13.6 | 81 / 128 / 144 | 26 / 63 / 45 / 52 | 4000 | 263 | 4 / 4 (errori 0 / 0) |  |
| 5 | 300157 | 300157 | 299460 | 4991 | 7.2 / 10.4 / 13.8 | 77 / 126 / 137 | 25 / 62 / 43 / 44 | 4021 | 329 | 5 / 5 (errori 0 / 0) | ok (fee 0.000005 CnFA, reward 0.0000045 CnFA, refund 0 CnFA, net cost 0.0000005 CnFA) |
| 6 | 298836 | 298836 | 299545 | 4992 | 7.4 / 10.6 / 13.3 | 78 / 125 / 137 | 25 / 63 / 43 / 44 | 4046 | 395 | 6 / 6 (errori 0 / 0) |  |
snapshot all'altezza 3600: RSS subito prima 4046 MB, 1 minuto dopo 4066 MB (+20 MB)
| 7 | 299660 | 299660 | 299979 | 5000 | 7.9 / 11.3 / 14.1 | 80 / 128 / 145 | 27 / 64 / 45 / 52 | 4066 | 460 | 7 / 7 (errori 0 / 0) |  |
| 8 | 299636 | 299636 | 299322 | 4989 | 7.7 / 11.7 / 14.3 | 79 / 126 / 138 | 27 / 65 / 45 / 44 | 4067 | 526 | 8 / 8 (errori 0 / 0) |  |
| 9 | 299138 | 299138 | 299143 | 4986 | 7.5 / 10.8 / 13.1 | 77 / 125 / 136 | 26 / 63 / 44 / 44 | 4067 | 592 | 9 / 9 (errori 0 / 0) |  |
snapshot all'altezza 5400: RSS subito prima 4067 MB, 1 minuto dopo 4078 MB (+11 MB)
| 10 | 300539 | 300539 | 299732 | 4996 | 8.0 / 12.7 / 16.3 | 82 / 129 / 144 | 28 / 66 / 47 / 53 | 4078 | 658 | 10 / 10 (errori 0 / 0) | ok (fee 0.000005 CnFA, reward 0.0000045 CnFA, refund 0 CnFA, net cost 0.0000005 CnFA) |
| 11 | 298695 | 298695 | 299439 | 4991 | 7.8 / 11.9 / 14.9 | 78 / 126 / 139 | 27 / 65 / 45 / 44 | 4078 | 723 | 11 / 11 (errori 0 / 0) |  |
| 12 | 300424 | 300424 | 300354 | 5006 | 8.1 / 13.2 / 16.7 | 79 / 126 / 138 | 27 / 66 / 46 / 45 | 4088 | 789 | 12 / 12 (errori 0 / 0) |  |
snapshot all'altezza 7200: RSS subito prima 4088 MB, 1 minuto dopo 4088 MB (+0 MB)
| 13 | 300550 | 300550 | 300557 | 5009 | 8.4 / 13.2 / 15.9 | 82 / 131 / 146 | 29 / 67 / 49 / 54 | 4088 | 855 | 13 / 13 (errori 0 / 0) |  |
| 14 | 299777 | 299777 | 299904 | 4998 | 7.7 / 11.5 / 14.7 | 80 / 126 / 138 | 26 / 64 / 45 / 45 | 4088 | 921 | 14 / 14 (errori 0 / 0) |  |
| 15 | 301335 | 301335 | 300180 | 5003 | 7.5 / 10.5 / 12.9 | 77 / 125 / 137 | 26 / 63 / 44 / 44 | 4089 | 987 | 15 / 15 (errori 0 / 0) | ok (fee 0.000005 CnFA, reward 0.0000045 CnFA, refund 0 CnFA, net cost 0.0000005 CnFA) |
totale: inviate 4499202, accettate 4499202, rifiutate all'ingresso 0, incluse 4499205, non incluse 0
TPS effettivi medi 4995 (richiesti 5000); altezza 9009
blocco (tutta la prova): p50 7.7 / p95 11.8 / p99 14.9 / max 32.7 ms
RAM: all'inizio 474 MB, prima del nodo; alla fine 4089 MB; stati in RAM 150171
latenza scomposta (71956 campioni), p50 / p99 ms: invio -> eseguito 58 / 107; eseguito -> durevole 1 / 11; durevole -> pubblicato 3 / 5; pubblicato -> visto dal soak 18 / 38
istogramma invio -> eseguito (inizio del passo da 10 ms: campioni; 300 = oltre): 0:1031 10:6943 20:7352 30:7174 40:7106 50:7102 60:7206 70:7403 80:7944 90:9812 100:2735 110:148 120:0 130:0 140:0 150:0 160:0 170:0 180:0 190:0 200:0 210:0 220:0 230:0 240:0 250:0 260:0 270:0 280:0 290:0 300:0
fase dell'invio dopo l'ultimo blocco eseguito (passi da 10 ms): 0:9792 10:8086 20:7308 30:7142 40:7118 50:7100 60:7219 70:7266 80:7186 90:3696 100:43 110:0 120:0 130:0 140:0 150:0 160:0 170:0 180:0 190:0 200:0 210:0 220:0 230:0 240:0 250:0 260:0 270:0 280:0 290:0 300:0
blocchi eseguiti tra l'invio e il blocco della tx (0 = il primo utile): [69783, 2173, 0, 0, 0, 0]
thread del nodo per blocco (9005 blocchi), ms: query 14.45 (preflight 10.81, di cui simulazione 9.29); produzione 8.11 (esecuzione 5.55, hash + firma 1.66); occupato 22.56; preflight = 47.9% del thread occupato
dall'accodamento al journal alla durabilità: p50 3.7 / p99 13.1 / max 181.4 ms (il blocco sopra è il tempo sul thread del nodo, attesa per coda piena compresa)

| snapshot (altezza) | durata s | picco Dirty MB | blocchi nella finestra | sync_data p99 / max ms |
|---|---|---|---|---|
| 1800 | 9.4 | 7 | 93 | 99.0 / 161.3 |
| 3600 | 9.3 | 7 | 93 | 133.4 / 149.7 |
| 5400 | 9.4 | 7 | 94 | 74.7 / 96.9 |
| 7200 | 9.4 | 8 | 94 | 100.2 / 132.3 |

sync_data del journal: finestra reale degli snapshot (374 blocchi) p99 100.2 / max 161.3 ms; finestra fissa di 600 blocchi (2410 blocchi) p99 13.0 / max 161.3 ms; resto (6595 blocchi) p99 4.1 / max 23.5 ms; picco Dirty in tutta la prova 8 MB
invio→inclusione (71956 campioni, 1 ogni ~1000 tx, risoluzione ~40 ms): p50 79 / p95 127 / p99 139 / max 329 ms
CPU media (% di un core): nodo 26, rete 64, client 45, altro 47
RSS intorno agli snapshot (altezza: prima -> 1 min dopo): 1800: 3980 -> 4000, 3600: 4046 -> 4066, 5400: 4067 -> 4078, 7200: 4088 -> 4088
journal 987 MB = 219 byte/tx, 3944 MB/ora; snapshot 892 MB; status.bin 11.4 MB
stati: 15 trovati e 15 scaduti come atteso (errori 0 / 0); wallet sotto carico: 3 ok, 0 errori
snapshot durante il carico: 5 (pausa sul thread del nodo: codifica degli stati e invio allo stato ombra; copia e scrittura sono in background: 0 ms, 0 ms, 0 ms, 0 ms, 0 ms), massimo 0 ms
blocchi oltre 100 ms: 0 
snapshot saltati (scrittura in corso, coda piena o ombra persa): 0; ultima scrittura 9398 ms; coda massima durante una scrittura 89 blocchi
