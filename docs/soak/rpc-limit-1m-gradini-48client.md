# soak rpc-limit-1m-gradini-48client

comando: `scripts/soak-run.sh rpc-limit-1m-gradini-48client 1000000 7500 21 --rpc` (commit 8de6eb4)

macchina: 4 vCPU, Intel(R) Xeon(R) Processor @ 2.10GHz; commit 8de6eb4; avvio 1793969746490 ms (unix)
1000000 account, 7500 tx/s richiesti, 21 minuti, invio via HTTP/JSON-RPC (end-to-end) (48 thread client), rpc_recent_txs 20000, snapshot ogni 3000 blocchi, stream_catchup_blocks 36000
CPU in % di un core: nodo = thread dei blocchi; rete = runtime tokio (RPC e stream, lato nodo); client = thread che firmano e inviano (in modalità diretta includono la verifica delle firme all'ingresso)
| min | inviate | accettate | incluse | TPS effettivi | blocco p50 / p95 / p99 ms | invio→inclusione p50 / p95 / p99 ms | CPU nodo / rete / client / altro % | RSS MB | journal MB | stati ok (290 blocchi) / scaduti ok (>300) | wallet |
gradini: 7500, 10000, 12500, 15000, 17500, 20000, 25000 tx/s, 3 minuti ciascuno; stop al primo con p99 del blocco > 100 ms o tx accettate non incluse
| 1 | 451200 | 451200 | 451200 | 7520 | 18.9 / 25.9 / 30.5 | 89 / 133 / 140 | 75 / 57 / 68 / 0 | 753 | 99 | 1 / 1 (errori 0 / 0) |  |
| 2 | 449150 | 449150 | 449117 | 7485 | 19.7 / 27.2 / 32.2 | 82 / 132 / 137 | 78 / 58 / 70 / 0 | 784 | 198 | 2 / 2 (errori 0 / 0) |  |
| 3 | 450850 | 450850 | 450883 | 7515 | 20.0 / 28.0 / 31.3 | 80 / 133 / 139 | 78 / 58 / 70 / 0 | 813 | 296 | 3 / 3 (errori 0 / 0) |  |
gradino 7500 tx/s: | 7500 | 7507 | 7507 | 7507 | 19.6 / 27.0 / 31.6 | 84 / 133 / 139 | 0 |
| 4 | 572650 | 572650 | 573277 | 9555 | 22.6 / 27.1 / 32.1 | 71 / 130 / 135 | 95 / 68 / 72 / 0 | 862 | 422 | 4 / 4 (errori 0 / 0) |  |
| 5 | 560750 | 560750 | 559545 | 9326 | 23.0 / 27.7 / 32.8 | 74 / 130 / 136 | 96 / 67 / 73 / 0 | 884 | 545 | 5 / 5 (errori 0 / 0) | ok (fee 0.000005 CnFA, reward 0.0000045 CnFA, refund 0 CnFA, net cost 0.0000005 CnFA) |
| 6 | 552000 | 552000 | 552745 | 9212 | 22.6 / 27.0 / 30.7 | 74 / 130 / 136 | 95 / 68 / 73 / 0 | 884 | 666 | 6 / 6 (errori 0 / 0) |  |
gradino 10000 tx/s: | 10000 | 9363 | 9363 | 9364 | 22.8 / 27.3 / 32.5 | 73 / 130 / 136 | 0 |
snapshot all'altezza 3000: RSS subito prima 876 MB, 1 minuto dopo 884 MB (+8 MB)
| 7 | 565700 | 565700 | 565836 | 9431 | 22.7 / 27.4 / 30.0 | 72 / 130 / 135 | 95 / 68 / 74 / 0 | 884 | 790 | 7 / 7 (errori 0 / 0) |  |
| 8 | 569700 | 569700 | 569748 | 9496 | 22.6 / 26.8 / 30.0 | 74 / 130 / 135 | 95 / 68 / 73 / 0 | 886 | 914 | 8 / 8 (errori 0 / 0) |  |
| 9 | 554100 | 554100 | 554220 | 9237 | 22.7 / 27.7 / 30.3 | 73 / 130 / 137 | 95 / 67 / 74 / 0 | 886 | 1036 | 9 / 9 (errori 0 / 0) |  |
gradino 12500 tx/s: | 12500 | 9386 | 9386 | 9388 | 22.7 / 27.4 / 30.2 | 73 / 130 / 136 | 0 |

## Esito (run fermato dopo il gradino 12500)

| gradino tx/s | inviate/s | accettate/s | incluse/s | blocco p50 / p95 / p99 ms | invio→inclusione p50 / p95 / p99 ms | in coda a fine gradino |
|---|---|---|---|---|---|---|
| 7500 | 7507 | 7507 | 7507 | 19.6 / 27.0 / 31.6 | 84 / 133 / 139 | 0 |
| 10000 | 9363 | 9363 | 9364 | 22.8 / 27.3 / 32.5 | 73 / 130 / 136 | 0 |
| 12500 | 9386 | 9386 | 9388 | 22.7 / 27.4 / 30.2 | 73 / 130 / 136 | 0 |

Limite end-to-end via RPC a 1M account su questa macchina (4 vCPU, client sulla stessa
macchina): **~9.400 tx/s**. Non lo raggiunge nessuno dei due criteri (p99 del blocco 30 ms,
nessuna tx accettata e non inclusa): oltre ~9.400 i client non riescono a inviare di più.
Il collo è il thread del nodo: ~95% di un core dal gradino 10000, mentre l'esecuzione dei
blocchi ne usa ~23% (22,7 ms ogni 100 ms). Il resto è il lavoro delle richieste RPC, che
girano sul thread del nodo tra un blocco e l'altro. Con 16 client il tetto era ~9.000:
più client non lo spostano. I gradini 15000-25000 non sono stati eseguiti (stesso tetto);
riavvio e riesecuzione non eseguiti per questo run.
