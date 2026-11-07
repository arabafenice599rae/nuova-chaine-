# RPC 10M Poisson, thread del nodo, dopo la PR (pool di verifica), skip
macchina: 4 vCPU, Intel(R) Xeon(R) Processor @ 2.30GHz; commit ?; avvio 1794057775848 ms (unix)
avvio: snapshot iniziale su disco, Dirty 0 MB dopo 0.0 s di attesa
10000000 account, 5000 tx/s richiesti, 15 minuti, invio via HTTP/JSON-RPC (end-to-end), rpc_recent_txs 20000, snapshot ogni 1800 blocchi, stream_catchup_blocks 36000
CPU in % di un core: nodo = thread dei blocchi; rete = runtime tokio (RPC e stream, lato nodo); client = thread che firmano e inviano (in modalità diretta includono la verifica delle firme all'ingresso)
| min | inviate | accettate | incluse | TPS effettivi | blocco p50 / p95 / p99 ms | invio→inclusione p50 / p95 / p99 ms | CPU nodo / rete / client / altro % | RSS MB | journal MB | stati ok (290 blocchi) / scaduti ok (>300) | wallet |
| 1 | 300629 | 300629 | 300519 | 5009 | 8.8 / 14.2 / 17.8 | 81 / 132 / 145 | 12 / 62 / 50 / 64 | 3795 | 66 | 1 / 1 (errori 0 / 0) |  |
| 2 | 299552 | 299552 | 299139 | 4986 | 8.8 / 13.7 / 16.3 | 81 / 132 / 145 | 12 / 64 / 51 / 49 | 3817 | 132 | 2 / 2 (errori 0 / 0) |  |
| 3 | 300364 | 300364 | 300323 | 5005 | 8.4 / 12.0 / 14.3 | 80 / 131 / 142 | 11 / 59 / 46 / 48 | 3884 | 197 | 3 / 3 (errori 0 / 0) |  |
snapshot all'altezza 1800: RSS subito prima 3884 MB, 1 minuto dopo 3904 MB (+20 MB)
| 4 | 300085 | 300085 | 300569 | 5009 | 8.3 / 12.5 / 16.1 | 81 / 132 / 149 | 11 / 57 / 44 / 54 | 3904 | 263 | 4 / 4 (errori 0 / 0) |  |
| 5 | 300011 | 300011 | 299517 | 4992 | 8.4 / 11.3 / 13.3 | 77 / 129 / 137 | 11 / 57 / 44 / 46 | 3925 | 329 | 5 / 5 (errori 0 / 0) | ok (fee 0.000005 CnFA, reward 0.0000045 CnFA, refund 0 CnFA, net cost 0.0000005 CnFA) |
| 6 | 298984 | 298984 | 299008 | 4983 | 8.9 / 13.6 / 15.8 | 79 / 132 / 144 | 12 / 58 / 47 / 48 | 3950 | 395 | 6 / 6 (errori 0 / 0) |  |
snapshot all'altezza 3600: RSS subito prima 3950 MB, 1 minuto dopo 3971 MB (+21 MB)
| 7 | 299485 | 299485 | 299518 | 4992 | 8.7 / 12.6 / 14.7 | 82 / 132 / 149 | 11 / 57 / 45 / 54 | 3971 | 460 | 7 / 7 (errori 0 / 0) |  |
| 8 | 299922 | 299922 | 300281 | 5005 | 9.3 / 14.2 / 16.5 | 80 / 131 / 143 | 12 / 60 / 47 / 47 | 3972 | 526 | 8 / 8 (errori 0 / 0) |  |
| 9 | 299132 | 299132 | 299127 | 4985 | 9.4 / 14.0 / 16.9 | 81 / 131 / 145 | 12 / 58 / 46 / 48 | 3972 | 592 | 9 / 9 (errori 0 / 0) |  |
snapshot all'altezza 5400: RSS subito prima 3972 MB, 1 minuto dopo 3982 MB (+10 MB)
| 10 | 300239 | 300239 | 299250 | 4988 | 8.9 / 13.6 / 17.7 | 82 / 134 / 148 | 12 / 57 / 46 / 56 | 3982 | 658 | 10 / 10 (errori 0 / 0) | ok (fee 0.000005 CnFA, reward 0.0000045 CnFA, refund 0 CnFA, net cost 0.0000005 CnFA) |
| 11 | 298760 | 298760 | 299439 | 4991 | 8.6 / 12.6 / 14.1 | 79 / 130 / 145 | 11 / 59 / 46 / 47 | 3982 | 723 | 11 / 11 (errori 0 / 0) |  |
| 12 | 300458 | 300458 | 300352 | 5006 | 8.4 / 12.2 / 14.0 | 78 / 128 / 140 | 11 / 59 / 46 / 47 | 3992 | 789 | 12 / 12 (errori 0 / 0) |  |
snapshot all'altezza 7200: RSS subito prima 3992 MB, 1 minuto dopo 3992 MB (+0 MB)
| 13 | 300614 | 300614 | 301067 | 5018 | 8.4 / 12.4 / 14.6 | 82 / 131 / 146 | 11 / 57 / 44 / 55 | 3992 | 855 | 13 / 13 (errori 0 / 0) |  |
| 14 | 299843 | 299843 | 299903 | 4998 | 8.0 / 11.9 / 14.0 | 80 / 129 / 140 | 10 / 55 / 43 / 46 | 3993 | 921 | 14 / 14 (errori 0 / 0) |  |
| 15 | 300665 | 300665 | 300144 | 5002 | 8.1 / 11.2 / 13.3 | 79 / 130 / 142 | 11 / 56 / 44 / 46 | 3993 | 987 | 15 / 15 (errori 0 / 0) | ok (fee 0.000005 CnFA, reward 0.0000045 CnFA, refund 0 CnFA, net cost 0.0000005 CnFA) |
totale: inviate 4498759, accettate 4498759, rifiutate all'ingresso 0, incluse 4498762, non incluse 0
TPS effettivi medi 4996 (richiesti 5000); altezza 9008
blocco (tutta la prova): p50 8.6 / p95 13.0 / p99 15.7 / max 29.3 ms
RAM: all'inizio 474 MB, prima del nodo; alla fine 3993 MB; stati in RAM 150216
latenza scomposta (71961 campioni), p50 / p99 ms: invio -> eseguito 59 / 109; eseguito -> durevole 1 / 10; durevole -> pubblicato 3 / 5; pubblicato -> visto dal soak 18 / 38
istogramma invio -> eseguito (inizio del passo da 10 ms: campioni; 300 = oltre): 0:657 10:6689 20:7235 30:7271 40:7119 50:7096 60:7067 70:7260 80:7257 90:7319 100:6522 110:462 120:7 130:0 140:0 150:0 160:0 170:0 180:0 190:0 200:0 210:0 220:0 230:0 240:0 250:0 260:0 270:0 280:0 290:0 300:0
fase dell'invio dopo l'ultimo blocco eseguito (passi da 10 ms): 0:7175 10:7229 20:7238 30:7136 40:7076 50:7092 60:7313 70:7274 80:7095 90:6643 100:680 110:10 120:0 130:0 140:0 150:0 160:0 170:0 180:0 190:0 200:0 210:0 220:0 230:0 240:0 250:0 260:0 270:0 280:0 290:0 300:0
blocchi eseguiti tra l'invio e il blocco della tx (0 = il primo utile): [65586, 6375, 0, 0, 0, 0]
thread del nodo per blocco (9004 blocchi), ms: query 0.08 (preflight 0.00, di cui simulazione 0.00); produzione 9.07 (esecuzione 5.77, hash + firma 1.71); occupato 9.14; preflight = 0.0% del thread occupato
dall'accodamento al journal alla durabilità: p50 3.6 / p99 12.9 / max 224.8 ms (il blocco sopra è il tempo sul thread del nodo, attesa per coda piena compresa)

| snapshot (altezza) | durata s | picco Dirty MB | blocchi nella finestra | sync_data p99 / max ms |
|---|---|---|---|---|
| 1800 | 9.3 | 8 | 93 | 66.0 / 103.3 |
| 3600 | 9.3 | 10 | 93 | 123.5 / 220.4 |
| 5400 | 9.4 | 7 | 94 | 83.7 / 150.6 |
| 7200 | 9.4 | 7 | 93 | 93.8 / 101.0 |

sync_data del journal: finestra reale degli snapshot (373 blocchi) p99 103.3 / max 220.4 ms; finestra fissa di 600 blocchi (2409 blocchi) p99 13.8 / max 220.4 ms; resto (6595 blocchi) p99 3.5 / max 40.7 ms; picco Dirty in tutta la prova 10 MB
invio→inclusione (71961 campioni, 1 ogni ~1000 tx, risoluzione ~40 ms): p50 80 / p95 131 / p99 145 / max 321 ms
CPU media (% di un core): nodo 11, rete 58, client 46, altro 50
RSS intorno agli snapshot (altezza: prima -> 1 min dopo): 1800: 3884 -> 3904, 3600: 3950 -> 3971, 5400: 3972 -> 3982, 7200: 3992 -> 3992
journal 987 MB = 219 byte/tx, 3944 MB/ora; snapshot 892 MB; status.bin 11.4 MB
stati: 15 trovati e 15 scaduti come atteso (errori 0 / 0); wallet sotto carico: 3 ok, 0 errori
snapshot durante il carico: 5 (pausa sul thread del nodo: codifica degli stati e invio allo stato ombra; copia e scrittura sono in background: 0 ms, 0 ms, 0 ms, 0 ms, 0 ms), massimo 0 ms
blocchi oltre 100 ms: 0 
snapshot saltati (scrittura in corso, coda piena o ombra persa): 0; ultima scrittura 9362 ms; coda massima durante una scrittura 89 blocchi
