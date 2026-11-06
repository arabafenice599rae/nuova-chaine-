# soak lthash-1m-5000-direct-10min-catchup128

comando: `scripts/soak-run.sh lthash-1m-5000-direct-10min-catchup128 1000000 5000 10` (commit 63973cd)

macchina: 4 vCPU, Intel(R) Xeon(R) Processor @ 2.10GHz; commit 63973cd; avvio 1793952829289 ms (unix)
1000000 account, 5000 tx/s richiesti, 10 minuti, invio diretto all'ingresso (limite del nodo), rpc_recent_txs 20000, snapshot ogni 3000 blocchi, stream_catchup_blocks 36000
| min | inviate | accettate | incluse | TPS effettivi | blocco p50 / p95 / p99 ms | RSS MB | journal MB | stati ok (290 blocchi) / scaduti ok (>300) | wallet |
| 1 | 300300 | 300300 | 299800 | 4997 | 11.8 / 16.5 / 20.4 | 719 | 66 | 1 / 1 (errori 0 / 0) |  |
| 2 | 300100 | 300100 | 300500 | 5008 | 12.0 / 17.3 / 20.2 | 738 | 132 | 2 / 2 (errori 0 / 0) |  |
| 3 | 300000 | 300000 | 300000 | 5000 | 12.5 / 19.6 / 26.7 | 757 | 197 | 3 / 3 (errori 0 / 0) |  |
| 4 | 300050 | 300050 | 300000 | 5000 | 12.5 / 21.7 / 34.2 | 775 | 263 | 4 / 4 (errori 0 / 0) |  |
| 5 | 303200 | 303200 | 297000 | 4950 | 12.8 / 22.2 / 29.9 | 813 | 330 | 5 / 5 (errori 0 / 0) | ok (fee 0.000005 CnFA, reward 0.0000045 CnFA, refund 0 CnFA, net cost 0.0000005 CnFA) |
| 6 | 296750 | 296750 | 302701 | 5045 | 13.3 / 27.6 / 34.9 | 816 | 395 | 6 / 6 (errori 0 / 0) |  |
| 7 | 300000 | 300000 | 300000 | 5000 | 12.8 / 18.5 / 20.9 | 834 | 461 | 7 / 7 (errori 0 / 0) |  |
| 8 | 299900 | 299900 | 300000 | 5000 | 12.7 / 18.3 / 21.8 | 835 | 526 | 8 / 8 (errori 0 / 0) |  |
| 9 | 300100 | 300100 | 300000 | 5000 | 12.6 / 17.2 / 20.9 | 835 | 592 | 9 / 9 (errori 0 / 0) |  |
| 10 | 303550 | 303550 | 297000 | 4950 | 15.8 / 25.3 / 53.3 | 859 | 659 | 10 / 10 (errori 0 / 0) | ok (fee 0.000005 CnFA, reward 0.0000045 CnFA, refund 0 CnFA, net cost 0.0000005 CnFA) |
totale: inviate 3004000, accettate 3004000, rifiutate all'ingresso 0, incluse 3004002, non incluse 0
TPS effettivi medi 4998 (richiesti 5000); altezza 6014
blocco (tutta la prova): p50 12.7 / p95 21.1 / p99 29.1 / max 1103.2 ms
RAM: all'inizio 85 MB, prima del nodo; alla fine 859 MB; stati in RAM 150501
journal 659 MB = 219 byte/tx, 3945 MB/ora; snapshot 91 MB; status.bin 11.4 MB
stati: 10 trovati e 10 scaduti come atteso (errori 0 / 0); wallet sotto carico: 2 ok, 0 errori
snapshot durante il carico: 2 (blocchi con snapshot: 1019 ms, 1103 ms), pausa massima 1103 ms
riavvio da snapshot (altezza 6000) + journal: 6.8 s, altezza 6014, hash uguali
riesecuzione completa: stato iniziale 1.3 s, journal 117.0 s (altezza 6014)
  preparazione (fino all'altezza 1): 1 blocchi, 0 tx, 0.0 s, 0.1 ms/blocco
  carico: 5991 blocchi, 3004002 tx, 115.1 s, 19.2 ms/blocco
  blocchi vuoti dopo l'inizio del carico (battito, coda): 0 blocchi, 0 tx, 0.0 s, 0.0 ms/blocco
  hash finale (blocco, stato incrementale, stato ricalcolato) uguali al nodo vivo
FINE
