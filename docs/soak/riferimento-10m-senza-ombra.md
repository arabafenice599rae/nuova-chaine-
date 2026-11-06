# soak di riferimento, 10M, senza stato ombra

comando: `SNAPSHOT_EVERY=0 RESULTS=docs/soak/riferimento-10m-senza-ombra.md LOAD_DIR=<dir> target/release/examples/soak 10000000 5000 6` (commit 9811e94): niente snapshot né stato ombra

macchina: 4 vCPU, Intel(R) Xeon(R) Processor @ 2.10GHz; commit 9811e94; avvio 1793976631687 ms (unix)
10000000 account, 5000 tx/s richiesti, 6 minuti, invio diretto all'ingresso (limite del nodo), rpc_recent_txs 20000, snapshot ogni 0 blocchi, stream_catchup_blocks 36000
CPU in % di un core: nodo = thread dei blocchi; rete = runtime tokio (RPC e stream, lato nodo); client = thread che firmano e inviano (in modalità diretta includono la verifica delle firme all'ingresso)
| min | inviate | accettate | incluse | TPS effettivi | blocco p50 / p95 / p99 ms | invio→inclusione p50 / p95 / p99 ms | CPU nodo / rete / client / altro % | RSS MB | journal MB | stati ok (290 blocchi) / scaduti ok (>300) | wallet |
| 1 | 300400 | 300400 | 299850 | 4998 | 13.5 / 18.7 / 21.2 | 91 / 132 / 137 | 14 / 0 / 31 / 0 | 2884 | 66 | 1 / 1 (errori 0 / 0) |  |
| 2 | 299900 | 299900 | 300050 | 5001 | 13.9 / 19.3 / 22.8 | 91 / 131 / 133 | 14 / 0 / 31 / 0 | 2904 | 132 | 2 / 2 (errori 0 / 0) |  |
| 3 | 300000 | 300000 | 299950 | 4999 | 13.7 / 19.1 / 22.0 | 91 / 131 / 133 | 14 / 0 / 30 / 0 | 2925 | 197 | 3 / 3 (errori 0 / 0) |  |
| 4 | 300000 | 300000 | 300050 | 5001 | 13.9 / 18.8 / 21.2 | 90 / 131 / 133 | 14 / 0 / 30 / 0 | 2945 | 263 | 4 / 4 (errori 0 / 0) |  |
| 5 | 300550 | 300550 | 299950 | 4999 | 13.5 / 18.1 / 21.3 | 91 / 131 / 133 | 13 / 0 / 29 / 0 | 2966 | 329 | 5 / 5 (errori 0 / 0) | ok (fee 0.000005 CnFA, reward 0.0000045 CnFA, refund 0 CnFA, net cost 0.0000005 CnFA) |
| 6 | 299550 | 299550 | 300001 | 5000 | 13.7 / 18.6 / 21.3 | 91 / 131 / 135 | 14 / 0 / 29 / 0 | 2987 | 395 | 6 / 6 (errori 0 / 0) |  |
totale: inviate 1800450, accettate 1800450, rifiutate all'ingresso 0, incluse 1800451, non incluse 0
TPS effettivi medi 4995 (richiesti 5000); altezza 3609
blocco (tutta la prova): p50 13.7 / p95 18.8 / p99 21.8 / max 866.6 ms
RAM: all'inizio 474 MB, prima del nodo; alla fine 2987 MB; stati in RAM 150050
invio→inclusione (1798 campioni, 1 ogni ~1000 tx, risoluzione ~40 ms): p50 91 / p95 131 / p99 134 / max 618 ms
CPU media (% di un core): nodo 14, rete 0, client 30, altro 0
RSS intorno agli snapshot (altezza: prima -> 1 min dopo): 
journal 395 MB = 219 byte/tx, 3944 MB/ora; snapshot 892 MB; status.bin 0.0 MB
stati: 6 trovati e 6 scaduti come atteso (errori 0 / 0); wallet sotto carico: 1 ok, 0 errori
snapshot durante il carico: 0 (pausa sul thread del nodo: codifica degli stati e invio allo stato ombra; copia e scrittura sono in background: ), massimo 0 ms
snapshot saltati (il precedente era ancora in scrittura): 0
FINE
