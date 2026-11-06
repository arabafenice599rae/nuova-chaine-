# Diagnosi dei picchi a 10M account (60 minuti, 5.000 tx/s, ingresso diretto)

comando: `RESULTS=docs/soak/diag-10m-60min.md LOAD_DIR=<dir> target/release/examples/soak 10000000 5000 60` (branch diag-spikes, da main 046509e, snapshot_every 3000 come in main: snapshot sincrono)

## Ambiente

```
THP enabled: always [madvise] never
THP defrag:  always defer defer+madvise [madvise] never
swapon --show: 0 righe (nessuna swap)
vCPU: 4
/dev/vda        252G   24G   15G  62% /
```

## fio sul disco del journal

`fio --name=journal --rw=write --bs=8k --fdatasync=1 --size=256m --directory=<cartella del journal>`

```
  write: IOPS=4143, BW=32.4MiB/s (33.9MB/s)(256MiB/7908msec); 0 zone resets
    clat (usec): min=2, max=378, avg= 5.37, stdev= 4.04
     lat (usec): min=3, max=378, avg= 5.48, stdev= 4.06
    clat percentiles (nsec):
     | 99.00th=[16768], 99.50th=[20864], 99.90th=[34560], 99.95th=[46336],
     | 99.99th=[85504]
   bw (  KiB/s): min=28080, max=36896, per=99.33%, avg=32928.00, stdev=2543.92, samples=15
   iops        : min= 3510, max= 4612, avg=4116.00, stdev=317.99, samples=15
  lat (usec)   : 4=22.42%, 10=72.15%, 20=4.85%, 50=0.55%, 100=0.02%
  lat (usec)   : 250=0.01%, 500=0.01%
  fsync/fdatasync/sync_file_range:
    sync (usec): min=150, max=30207, avg=235.15, stdev=202.63
    sync percentiles (usec):
     | 99.00th=[  478], 99.50th=[  586], 99.90th=[ 1156], 99.95th=[ 1680],
     | 99.99th=[ 6259]
```

## Soak

macchina: 4 vCPU, Intel(R) Xeon(R) Processor @ 2.10GHz; commit 046509e-diag; avvio 1793978524779 ms (unix)
10000000 account, 5000 tx/s richiesti, 60 minuti, invio diretto all'ingresso (limite del nodo), rpc_recent_txs 20000, snapshot ogni 3000 blocchi, stream_catchup_blocks 36000
CPU in % di un core: nodo = thread dei blocchi; rete = runtime tokio (RPC e stream, lato nodo); client = thread che firmano e inviano (in modalità diretta includono la verifica delle firme all'ingresso)
| min | inviate | accettate | incluse | TPS effettivi | blocco p50 / p95 / p99 ms | invio→inclusione p50 / p95 / p99 ms | CPU nodo / rete / client / altro % | RSS MB | journal MB | stati ok (290 blocchi) / scaduti ok (>300) | wallet |
| 1 | 300450 | 300450 | 299850 | 4998 | 12.7 / 17.5 / 22.7 | 81 / 124 / 126 | 13 / 0 / 30 / 0 | 2830 | 66 | 1 / 1 (errori 0 / 0) |  |
| 2 | 299950 | 299950 | 300000 | 5000 | 13.1 / 17.4 / 20.2 | 82 / 124 / 126 | 13 / 0 / 30 / 0 | 2849 | 132 | 2 / 2 (errori 0 / 0) |  |
| 3 | 300000 | 300000 | 300000 | 5000 | 13.8 / 19.9 / 21.8 | 82 / 124 / 126 | 14 / 0 / 31 / 0 | 2868 | 197 | 3 / 3 (errori 0 / 0) |  |
| 4 | 299900 | 299900 | 300000 | 5000 | 13.7 / 18.9 / 21.2 | 82 / 124 / 126 | 14 / 0 / 31 / 0 | 2887 | 263 | 4 / 4 (errori 0 / 0) |  |
| 5 | 357900 | 357900 | 297000 | 4950 | 13.9 / 19.0 / 22.3 | 82 / 125 / 10418 | 32 / 0 / 38 / 0 | 2957 | 342 | 5 / 5 (errori 0 / 0) | ok (fee 0.000005 CnFA, reward 0.000002 CnFA, refund 0 CnFA, net cost 0.000003 CnFA) |
| 6 | 242150 | 242150 | 303151 | 5053 | 13.4 / 18.7 / 22.1 | 86 / 8119 / 10519 | 11 / 0 / 26 / 0 | 2961 | 395 | 6 / 6 (errori 0 / 0) |  |
snapshot all'altezza 3000: RSS subito prima 2906 MB, 1 minuto dopo 2965 MB (+59 MB)
| 7 | 300100 | 300100 | 300000 | 5000 | 13.7 / 18.6 / 20.8 | 68 / 137 / 141 | 14 / 0 / 33 / 0 | 2981 | 461 | 7 / 7 (errori 0 / 0) |  |
| 8 | 299850 | 299850 | 300000 | 5000 | 13.5 / 18.8 / 22.0 | 68 / 137 / 140 | 14 / 0 / 31 / 0 | 2981 | 526 | 8 / 8 (errori 0 / 0) |  |
| 9 | 300000 | 300000 | 300000 | 5000 | 12.9 / 17.7 / 21.6 | 68 / 137 / 140 | 13 / 0 / 30 / 0 | 2981 | 592 | 9 / 9 (errori 0 / 0) |  |
| 10 | 300650 | 300650 | 300000 | 5000 | 12.8 / 17.8 / 21.1 | 67 / 137 / 140 | 13 / 0 / 30 / 0 | 2981 | 658 | 10 / 10 (errori 0 / 0) | ok (fee 0.000005 CnFA, reward 0.0000045 CnFA, refund 0 CnFA, net cost 0.0000005 CnFA) |
| 11 | 299500 | 299500 | 300251 | 5004 | 13.0 / 19.1 / 95.9 | 99 / 6777 / 8719 | 26 / 0 / 31 / 0 | 2992 | 724 | 11 / 11 (errori 0 / 0) |  |
snapshot all'altezza 6000: RSS subito prima 2981 MB, 1 minuto dopo 2992 MB (+11 MB)
| 12 | 300000 | 300000 | 299950 | 4999 | 13.1 / 18.2 / 19.9 | 83 / 123 / 125 | 13 / 0 / 31 / 0 | 2992 | 790 | 12 / 12 (errori 0 / 0) |  |
| 13 | 300000 | 300000 | 300050 | 5001 | 13.9 / 19.2 / 22.8 | 83 / 123 / 125 | 14 / 0 / 31 / 0 | 2992 | 855 | 13 / 13 (errori 0 / 0) |  |
| 14 | 300000 | 300000 | 300000 | 5000 | 14.3 / 19.9 / 22.1 | 83 / 123 / 125 | 15 / 0 / 31 / 0 | 2992 | 921 | 14 / 14 (errori 0 / 0) |  |
| 15 | 300450 | 300450 | 299950 | 4999 | 14.0 / 19.8 / 25.9 | 82 / 123 / 124 | 14 / 0 / 31 / 0 | 2992 | 987 | 15 / 15 (errori 0 / 0) | ok (fee 0.000005 CnFA, reward 0.0000045 CnFA, refund 0 CnFA, net cost 0.0000005 CnFA) |
| 16 | 299400 | 299400 | 300001 | 5000 | 13.5 / 19.6 / 104.1 | 96 / 7699 / 9607 | 28 / 0 / 31 / 0 | 2998 | 1053 | 16 / 16 (errori 0 / 0) |  |
snapshot all'altezza 9000: RSS subito prima 2992 MB, 1 minuto dopo 2998 MB (+6 MB)
| 17 | 300100 | 300100 | 300000 | 5000 | 12.8 / 17.8 / 23.5 | 89 / 119 / 121 | 13 / 0 / 30 / 0 | 2998 | 1118 | 17 / 17 (errori 0 / 0) |  |
| 18 | 299900 | 299900 | 300000 | 5000 | 13.6 / 18.9 / 21.3 | 79 / 120 / 121 | 14 / 0 / 31 / 0 | 2998 | 1184 | 18 / 18 (errori 0 / 0) |  |
| 19 | 300100 | 300100 | 300000 | 5000 | 13.5 / 18.8 / 20.4 | 80 / 120 / 121 | 14 / 0 / 31 / 0 | 2998 | 1250 | 19 / 19 (errori 0 / 0) |  |
| 20 | 300500 | 300500 | 300000 | 5000 | 13.2 / 18.2 / 21.9 | 79 / 120 / 121 | 14 / 0 / 30 / 0 | 2998 | 1316 | 20 / 20 (errori 0 / 0) | ok (fee 0.000005 CnFA, reward 0.0000045 CnFA, refund 0 CnFA, net cost 0.0000005 CnFA) |
| 21 | 299450 | 299450 | 299601 | 4993 | 13.2 / 18.7 / 96.5 | 97 / 7649 / 9825 | 28 / 0 / 30 / 0 | 3006 | 1382 | 21 / 21 (errori 0 / 0) |  |
snapshot all'altezza 12000: RSS subito prima 2998 MB, 1 minuto dopo 3006 MB (+8 MB)
| 22 | 299950 | 299950 | 300000 | 5000 | 13.2 / 19.0 / 23.6 | 96 / 134 / 135 | 14 / 0 / 30 / 0 | 3006 | 1447 | 22 / 22 (errori 0 / 0) |  |
| 23 | 300100 | 300100 | 300500 | 5008 | 13.5 / 19.1 / 24.1 | 92 / 133 / 136 | 14 / 0 / 30 / 0 | 3006 | 1513 | 23 / 23 (errori 0 / 0) |  |
| 24 | 299850 | 299850 | 299500 | 4992 | 13.8 / 19.2 / 23.6 | 92 / 132 / 135 | 14 / 0 / 30 / 0 | 3006 | 1579 | 24 / 24 (errori 0 / 0) |  |
| 25 | 300650 | 300650 | 300000 | 5000 | 14.0 / 18.9 / 22.6 | 93 / 133 / 135 | 14 / 0 / 29 / 0 | 3006 | 1645 | 25 / 25 (errori 0 / 0) | ok (fee 0.000005 CnFA, reward 0.0000045 CnFA, refund 0 CnFA, net cost 0.0000005 CnFA) |
| 26 | 299550 | 299550 | 300351 | 5006 | 14.1 / 20.1 / 107.0 | 107 / 8263 / 10442 | 29 / 0 / 30 / 0 | 3007 | 1711 | 26 / 26 (errori 0 / 0) |  |
snapshot all'altezza 15000: RSS subito prima 3006 MB, 1 minuto dopo 3007 MB (+1 MB)
| 27 | 300000 | 300000 | 300000 | 5000 | 13.8 / 20.0 / 26.6 | 62 / 103 / 104 | 14 / 0 / 31 / 0 | 3007 | 1776 | 27 / 27 (errori 0 / 0) |  |
| 28 | 299850 | 299850 | 300000 | 5000 | 13.6 / 18.4 / 21.2 | 63 / 103 / 105 | 13 / 0 / 30 / 0 | 3007 | 1842 | 28 / 28 (errori 0 / 0) |  |
| 29 | 300000 | 300000 | 300000 | 5000 | 13.6 / 19.1 / 22.8 | 63 / 104 / 105 | 14 / 0 / 30 / 0 | 3007 | 1908 | 29 / 29 (errori 0 / 0) |  |
| 30 | 300550 | 300550 | 300000 | 5000 | 13.5 / 18.3 / 20.4 | 62 / 103 / 105 | 14 / 0 / 30 / 0 | 3007 | 1974 | 30 / 30 (errori 0 / 0) | ok (fee 0.000005 CnFA, reward 0.0000045 CnFA, refund 0 CnFA, net cost 0.0000005 CnFA) |
| 31 | 299450 | 299450 | 300101 | 5002 | 13.9 / 19.8 / 96.9 | 80 / 8757 / 10666 | 30 / 0 / 31 / 0 | 3011 | 2039 | 31 / 31 (errori 0 / 0) |  |
snapshot all'altezza 18000: RSS subito prima 3007 MB, 1 minuto dopo 3011 MB (+4 MB)
| 32 | 300050 | 300050 | 300000 | 5000 | 13.4 / 21.3 / 28.9 | 85 / 125 / 128 | 14 / 0 / 31 / 0 | 3011 | 2105 | 32 / 32 (errori 0 / 0) |  |
| 33 | 300000 | 300000 | 300000 | 5000 | 13.3 / 18.5 / 24.4 | 85 / 125 / 128 | 13 / 0 / 30 / 0 | 3011 | 2171 | 33 / 33 (errori 0 / 0) |  |
| 34 | 299950 | 299950 | 300000 | 5000 | 13.6 / 18.5 / 21.4 | 85 / 125 / 128 | 14 / 0 / 30 / 0 | 3011 | 2237 | 34 / 34 (errori 0 / 0) |  |
| 35 | 300650 | 300650 | 300000 | 5000 | 13.4 / 18.7 / 23.6 | 86 / 125 / 128 | 14 / 0 / 30 / 0 | 3011 | 2303 | 35 / 35 (errori 0 / 0) | ok (fee 0.000005 CnFA, reward 0.0000045 CnFA, refund 0 CnFA, net cost 0.0000005 CnFA) |
| 36 | 338350 | 338350 | 284451 | 4741 | 14.1 / 19.2 / 26.6 | 86 / 127 / 10076 | 29 / 0 / 34 / 0 | 3011 | 2368 | 36 / 36 (errori 0 / 0) |  |
| 37 | 261150 | 261150 | 315300 | 5255 | 13.4 / 20.1 / 34.7 | 70 / 6913 / 9313 | 13 / 0 / 26 / 0 | 3011 | 2434 | 37 / 37 (errori 0 / 0) |  |
snapshot all'altezza 21000: RSS subito prima 3011 MB, 1 minuto dopo 3011 MB (+0 MB)
| 38 | 299850 | 299850 | 300000 | 5000 | 13.8 / 18.2 / 20.9 | 52 / 123 / 125 | 14 / 0 / 30 / 0 | 3011 | 2500 | 38 / 38 (errori 0 / 0) |  |
| 39 | 300000 | 300000 | 300000 | 5000 | 13.7 / 18.3 / 21.1 | 53 / 123 / 125 | 14 / 0 / 30 / 0 | 3011 | 2566 | 39 / 39 (errori 0 / 0) |  |
| 40 | 300750 | 300750 | 300000 | 5000 | 13.8 / 18.9 / 20.9 | 53 / 123 / 124 | 14 / 0 / 30 / 0 | 3011 | 2632 | 40 / 40 (errori 0 / 0) | ok (fee 0.000005 CnFA, reward 0.0000045 CnFA, refund 0 CnFA, net cost 0.0000005 CnFA) |
| 41 | 299300 | 299300 | 300001 | 5000 | 13.9 / 19.4 / 21.8 | 53 / 122 / 125 | 14 / 0 / 30 / 0 | 3011 | 2697 | 41 / 41 (errori 0 / 0) |  |
| 42 | 300050 | 300050 | 299850 | 4998 | 13.5 / 19.4 / 103.3 | 99 / 9508 / 11468 | 31 / 0 / 30 / 0 | 3013 | 2763 | 42 / 42 (errori 0 / 0) |  |
snapshot all'altezza 24000: RSS subito prima 3011 MB, 1 minuto dopo 3013 MB (+2 MB)
| 43 | 299900 | 299900 | 300000 | 5000 | 13.9 / 19.7 / 23.1 | 81 / 124 / 126 | 14 / 0 / 30 / 0 | 3013 | 2829 | 43 / 43 (errori 0 / 0) |  |
| 44 | 300150 | 300150 | 300500 | 5008 | 14.0 / 19.4 / 22.1 | 81 / 123 / 125 | 14 / 0 / 30 / 0 | 3013 | 2895 | 44 / 44 (errori 0 / 0) |  |
| 45 | 300450 | 300450 | 299500 | 4992 | 14.6 / 20.1 / 23.0 | 80 / 124 / 125 | 15 / 0 / 31 / 0 | 3013 | 2961 | 45 / 45 (errori 0 / 0) | ok (fee 0.000005 CnFA, reward 0.0000045 CnFA, refund 0 CnFA, net cost 0.0000005 CnFA) |
| 46 | 299550 | 299550 | 300451 | 5008 | 15.0 / 20.3 / 22.6 | 82 / 124 / 126 | 15 / 0 / 31 / 0 | 3013 | 3026 | 46 / 46 (errori 0 / 0) |  |
| 47 | 299850 | 299850 | 299600 | 4993 | 15.6 / 27.6 / 121.5 | 96 / 8585 / 10691 | 32 / 0 / 32 / 0 | 3013 | 3092 | 47 / 46 (errori 0 / 1) |  |
snapshot all'altezza 27000: RSS subito prima 3013 MB, 1 minuto dopo 3013 MB (+0 MB)
| 48 | 300100 | 300100 | 300100 | 5002 | 15.6 / 23.8 / 30.3 | 61 / 127 / 143 | 16 / 0 / 34 / 0 | 3013 | 3158 | 48 / 47 (errori 0 / 1) |  |
| 49 | 299900 | 299900 | 300000 | 5000 | 13.9 / 19.6 / 29.2 | 60 / 130 / 131 | 14 / 0 / 31 / 0 | 3013 | 3224 | 49 / 48 (errori 0 / 1) |  |
| 50 | 300600 | 300600 | 300000 | 5000 | 14.4 / 19.9 / 26.1 | 59 / 131 / 132 | 15 / 0 / 33 / 0 | 3013 | 3290 | 50 / 49 (errori 0 / 1) | ok (fee 0.000005 CnFA, reward 0.0000045 CnFA, refund 0 CnFA, net cost 0.0000005 CnFA) |
| 51 | 299400 | 299400 | 300001 | 5000 | 14.8 / 22.1 / 31.7 | 59 / 130 / 132 | 16 / 0 / 32 / 0 | 3013 | 3355 | 51 / 50 (errori 0 / 1) |  |
| 52 | 300100 | 300100 | 300150 | 5002 | 14.7 / 21.9 / 135.1 | 80 / 10720 / 12729 | 35 / 0 / 33 / 0 | 3028 | 3421 | 52 / 51 (errori 0 / 1) |  |
snapshot all'altezza 30000: RSS subito prima 3013 MB, 1 minuto dopo 3028 MB (+15 MB)
| 53 | 300050 | 300050 | 300000 | 5000 | 14.0 / 19.8 / 23.1 | 61 / 102 / 104 | 14 / 0 / 31 / 0 | 3028 | 3487 | 53 / 52 (errori 0 / 1) |  |
| 54 | 299950 | 299950 | 300000 | 5000 | 14.1 / 19.3 / 22.0 | 61 / 101 / 103 | 14 / 0 / 30 / 0 | 3028 | 3553 | 54 / 53 (errori 0 / 1) |  |
| 55 | 300500 | 300500 | 300000 | 5000 | 14.0 / 20.1 / 24.8 | 62 / 101 / 103 | 14 / 0 / 30 / 0 | 3028 | 3618 | 55 / 54 (errori 0 / 1) | ok (fee 0.000005 CnFA, reward 0.0000045 CnFA, refund 0 CnFA, net cost 0.0000005 CnFA) |
| 56 | 299400 | 299400 | 300001 | 5000 | 14.3 / 19.6 / 23.7 | 62 / 101 / 103 | 15 / 0 / 31 / 0 | 3028 | 3684 | 56 / 55 (errori 0 / 1) |  |
| 57 | 300050 | 300050 | 300150 | 5002 | 14.0 / 20.6 / 108.0 | 80 / 8405 / 10581 | 30 / 0 / 30 / 0 | 3028 | 3750 | 57 / 56 (errori 0 / 1) |  |
snapshot all'altezza 33000: RSS subito prima 3028 MB, 1 minuto dopo 3028 MB (+0 MB)
| 58 | 300000 | 300000 | 299450 | 4991 | 13.5 / 19.0 / 21.4 | 91 / 132 / 134 | 14 / 0 / 30 / 0 | 3028 | 3816 | 58 / 57 (errori 0 / 1) |  |
| 59 | 300100 | 300100 | 300550 | 5009 | 14.0 / 19.5 / 22.6 | 92 / 133 / 134 | 14 / 0 / 30 / 0 | 3028 | 3882 | 59 / 58 (errori 0 / 1) |  |
| 60 | 300550 | 300550 | 300000 | 5000 | 13.8 / 18.7 / 20.4 | 92 / 133 / 135 | 14 / 0 / 30 / 0 | 3028 | 3947 | 60 / 59 (errori 0 / 1) | ok (fee 0.000005 CnFA, reward 0.0000045 CnFA, refund 0 CnFA, net cost 0.0000005 CnFA) |
totale: inviate 18001000, accettate 18001000, rifiutate all'ingresso 0, incluse 18001012, non incluse 0
TPS effettivi medi 5000 (richiesti 5000); altezza 34900
blocco (tutta la prova): p50 13.7 / p95 19.5 / p99 25.3 / max 13360.2 ms
RAM: all'inizio 474 MB, prima del nodo; alla fine 3028 MB; stati in RAM 150251

| causa dei blocchi > 100 ms | blocchi | max ms |
|---|---|---|
| altro | 116 | 387 |
| disco (sync_data) | 8 | 308 |
| snapshot | 11 | 13361 |

blocco 112 (disco (sync_data)): 238 ms = esecuzione 6 + hash 2 + firma 0 + write 0.5 + sync_data 224.9 + pubblicazione 0.1; minflt 247 majflt 0; attesa CPU 0 ms; mappa 10004289->10004289 (capacity 14680064->14680064)
blocco 114 (disco (sync_data)): 308 ms = esecuzione 19 + hash 6 + firma 0 + write 1.2 + sync_data 273.5 + pubblicazione 0.1; minflt 612 majflt 0; attesa CPU 0 ms; mappa 10004289->10004289 (capacity 14680064->14680064)
blocco 117 (disco (sync_data)): 158 ms = esecuzione 26 + hash 6 + firma 0 + write 1.4 + sync_data 116.7 + pubblicazione 0.1; minflt 771 majflt 0; attesa CPU 0 ms; mappa 10004289->10004289 (capacity 14680064->14680064)
blocco 3000 (snapshot): 10889 ms = esecuzione 7 + hash 2 + firma 0 + write 0.4 + sync_data 0.8 + pubblicazione 0.2; minflt 61360 majflt 0; attesa CPU 3 ms; mappa 10004289->10004289 (capacity 14680064->14680064)
blocco 3010 (altro): 121 ms = esecuzione 72 + hash 20 + firma 1 + write 0.8 + sync_data 1.8 + pubblicazione 0.2; minflt 0 majflt 0; attesa CPU 0 ms; mappa 10004289->10004289 (capacity 14680064->14680064)
blocco 3011 (altro): 106 ms = esecuzione 59 + hash 19 + firma 1 + write 1.9 + sync_data 1.9 + pubblicazione 0.2; minflt 0 majflt 0; attesa CPU 0 ms; mappa 10004289->10004289 (capacity 14680064->14680064)
blocco 3012 (altro): 107 ms = esecuzione 59 + hash 20 + firma 1 + write 1.0 + sync_data 2.3 + pubblicazione 0.2; minflt 0 majflt 0; attesa CPU 0 ms; mappa 10004289->10004289 (capacity 14680064->14680064)
blocco 3013 (altro): 117 ms = esecuzione 62 + hash 21 + firma 1 + write 0.9 + sync_data 1.8 + pubblicazione 0.3; minflt 0 majflt 0; attesa CPU 0 ms; mappa 10004289->10004289 (capacity 14680064->14680064)
blocco 3014 (altro): 136 ms = esecuzione 85 + hash 25 + firma 1 + write 0.5 + sync_data 2.0 + pubblicazione 0.2; minflt 0 majflt 0; attesa CPU 0 ms; mappa 10004289->10004289 (capacity 14680064->14680064)
blocco 3015 (altro): 107 ms = esecuzione 59 + hash 22 + firma 1 + write 0.7 + sync_data 1.9 + pubblicazione 0.2; minflt 0 majflt 0; attesa CPU 0 ms; mappa 10004289->10004289 (capacity 14680064->14680064)
blocco 3016 (altro): 102 ms = esecuzione 58 + hash 19 + firma 1 + write 0.6 + sync_data 2.2 + pubblicazione 0.2; minflt 146 majflt 0; attesa CPU 0 ms; mappa 10004289->10004289 (capacity 14680064->14680064)
blocco 3017 (altro): 102 ms = esecuzione 57 + hash 20 + firma 1 + write 0.4 + sync_data 2.1 + pubblicazione 0.2; minflt 197 majflt 0; attesa CPU 0 ms; mappa 10004289->10004289 (capacity 14680064->14680064)
blocco 3018 (altro): 102 ms = esecuzione 57 + hash 19 + firma 1 + write 0.5 + sync_data 2.1 + pubblicazione 0.2; minflt 227 majflt 0; attesa CPU 0 ms; mappa 10004289->10004289 (capacity 14680064->14680064)
blocco 3019 (altro): 105 ms = esecuzione 62 + hash 19 + firma 1 + write 0.4 + sync_data 2.1 + pubblicazione 0.2; minflt 226 majflt 0; attesa CPU 0 ms; mappa 10004289->10004289 (capacity 14680064->14680064)
blocco 3020 (altro): 102 ms = esecuzione 58 + hash 20 + firma 1 + write 0.5 + sync_data 1.9 + pubblicazione 0.2; minflt 23 majflt 0; attesa CPU 0 ms; mappa 10004289->10004289 (capacity 14680064->14680064)
blocco 3021 (altro): 103 ms = esecuzione 59 + hash 19 + firma 1 + write 0.4 + sync_data 1.9 + pubblicazione 0.2; minflt 352 majflt 0; attesa CPU 0 ms; mappa 10004289->10004290 (capacity 14680064->14680064)
blocco 6000 (snapshot): 9470 ms = esecuzione 6 + hash 2 + firma 0 + write 0.1 + sync_data 0.9 + pubblicazione 0.1; minflt 60471 majflt 0; attesa CPU 1 ms; mappa 10004291->10004291 (capacity 14680064->14680064)
blocco 6010 (altro): 102 ms = esecuzione 59 + hash 18 + firma 1 + write 0.4 + sync_data 1.6 + pubblicazione 0.2; minflt 0 majflt 0; attesa CPU 0 ms; mappa 10004291->10004291 (capacity 14680064->14680064)
blocco 6011 (altro): 106 ms = esecuzione 62 + hash 19 + firma 2 + write 0.4 + sync_data 1.6 + pubblicazione 0.2; minflt 0 majflt 0; attesa CPU 0 ms; mappa 10004291->10004291 (capacity 14680064->14680064)
blocco 6012 (altro): 127 ms = esecuzione 75 + hash 20 + firma 2 + write 1.7 + sync_data 1.9 + pubblicazione 0.2; minflt 0 majflt 0; attesa CPU 0 ms; mappa 10004291->10004291 (capacity 14680064->14680064)
blocco 6013 (altro): 116 ms = esecuzione 74 + hash 18 + firma 1 + write 1.3 + sync_data 1.6 + pubblicazione 0.2; minflt 1 majflt 0; attesa CPU 0 ms; mappa 10004291->10004291 (capacity 14680064->14680064)
blocco 6017 (altro): 101 ms = esecuzione 61 + hash 18 + firma 1 + write 0.4 + sync_data 1.7 + pubblicazione 0.2; minflt 227 majflt 0; attesa CPU 0 ms; mappa 10004291->10004291 (capacity 14680064->14680064)
blocco 7704 (altro): 103 ms = esecuzione 9 + hash 2 + firma 0 + write 84.5 + sync_data 2.9 + pubblicazione 0.1; minflt 0 majflt 0; attesa CPU 0 ms; mappa 10004291->10004291 (capacity 14680064->14680064)
blocco 9000 (snapshot): 10308 ms = esecuzione 7 + hash 2 + firma 0 + write 0.5 + sync_data 0.7 + pubblicazione 0.1; minflt 58620 majflt 0; attesa CPU 2 ms; mappa 10004292->10004292 (capacity 14680064->14680064)
blocco 9010 (altro): 113 ms = esecuzione 63 + hash 21 + firma 1 + write 1.1 + sync_data 1.8 + pubblicazione 0.2; minflt 32 majflt 0; attesa CPU 0 ms; mappa 10004292->10004292 (capacity 14680064->14680064)
blocco 9011 (altro): 119 ms = esecuzione 62 + hash 23 + firma 2 + write 0.4 + sync_data 1.8 + pubblicazione 0.4; minflt 4 majflt 0; attesa CPU 0 ms; mappa 10004292->10004292 (capacity 14680064->14680064)
blocco 9012 (altro): 136 ms = esecuzione 79 + hash 24 + firma 2 + write 0.4 + sync_data 1.9 + pubblicazione 0.3; minflt 4 majflt 0; attesa CPU 0 ms; mappa 10004292->10004292 (capacity 14680064->14680064)
blocco 9013 (altro): 119 ms = esecuzione 69 + hash 20 + firma 1 + write 0.4 + sync_data 2.0 + pubblicazione 0.3; minflt 4 majflt 0; attesa CPU 0 ms; mappa 10004292->10004292 (capacity 14680064->14680064)
blocco 9014 (altro): 104 ms = esecuzione 61 + hash 20 + firma 1 + write 0.4 + sync_data 1.8 + pubblicazione 0.3; minflt 4 majflt 0; attesa CPU 0 ms; mappa 10004292->10004292 (capacity 14680064->14680064)
blocco 9015 (altro): 109 ms = esecuzione 62 + hash 20 + firma 1 + write 0.4 + sync_data 1.6 + pubblicazione 0.3; minflt 4 majflt 0; attesa CPU 0 ms; mappa 10004292->10004292 (capacity 14680064->14680064)
blocco 9016 (altro): 120 ms = esecuzione 74 + hash 20 + firma 1 + write 0.4 + sync_data 1.9 + pubblicazione 0.3; minflt 4 majflt 0; attesa CPU 0 ms; mappa 10004292->10004292 (capacity 14680064->14680064)
blocco 9017 (altro): 104 ms = esecuzione 58 + hash 19 + firma 1 + write 0.4 + sync_data 2.1 + pubblicazione 0.2; minflt 4 majflt 0; attesa CPU 0 ms; mappa 10004292->10004292 (capacity 14680064->14680064)
blocco 9018 (altro): 109 ms = esecuzione 60 + hash 21 + firma 1 + write 0.4 + sync_data 1.6 + pubblicazione 0.3; minflt 444 majflt 0; attesa CPU 0 ms; mappa 10004292->10004292 (capacity 14680064->14680064)
blocco 9019 (altro): 112 ms = esecuzione 65 + hash 23 + firma 1 + write 0.4 + sync_data 1.8 + pubblicazione 0.2; minflt 863 majflt 0; attesa CPU 0 ms; mappa 10004292->10004292 (capacity 14680064->14680064)
blocco 9020 (altro): 102 ms = esecuzione 60 + hash 19 + firma 1 + write 0.7 + sync_data 1.8 + pubblicazione 0.2; minflt 345 majflt 0; attesa CPU 0 ms; mappa 10004292->10004292 (capacity 14680064->14680064)
blocco 12000 (snapshot): 10496 ms = esecuzione 7 + hash 2 + firma 0 + write 0.3 + sync_data 0.9 + pubblicazione 0.2; minflt 60616 majflt 0; attesa CPU 1 ms; mappa 10004293->10004293 (capacity 14680064->14680064)
blocco 12010 (altro): 126 ms = esecuzione 77 + hash 20 + firma 1 + write 1.4 + sync_data 1.6 + pubblicazione 0.3; minflt 0 majflt 0; attesa CPU 0 ms; mappa 10004293->10004293 (capacity 14680064->14680064)
blocco 12011 (altro): 106 ms = esecuzione 62 + hash 19 + firma 1 + write 0.5 + sync_data 1.7 + pubblicazione 0.2; minflt 0 majflt 0; attesa CPU 0 ms; mappa 10004293->10004293 (capacity 14680064->14680064)
blocco 12012 (altro): 109 ms = esecuzione 58 + hash 23 + firma 2 + write 0.5 + sync_data 1.9 + pubblicazione 0.3; minflt 0 majflt 0; attesa CPU 0 ms; mappa 10004293->10004293 (capacity 14680064->14680064)
blocco 12013 (altro): 111 ms = esecuzione 60 + hash 20 + firma 1 + write 4.6 + sync_data 1.8 + pubblicazione 0.2; minflt 0 majflt 0; attesa CPU 0 ms; mappa 10004293->10004293 (capacity 14680064->14680064)
blocco 12014 (altro): 106 ms = esecuzione 64 + hash 19 + firma 1 + write 0.4 + sync_data 1.8 + pubblicazione 0.3; minflt 0 majflt 0; attesa CPU 0 ms; mappa 10004293->10004293 (capacity 14680064->14680064)
blocco 12015 (altro): 101 ms = esecuzione 59 + hash 19 + firma 1 + write 0.4 + sync_data 1.7 + pubblicazione 0.3; minflt 0 majflt 0; attesa CPU 0 ms; mappa 10004293->10004293 (capacity 14680064->14680064)
blocco 12016 (altro): 103 ms = esecuzione 60 + hash 18 + firma 2 + write 0.4 + sync_data 1.6 + pubblicazione 0.2; minflt 0 majflt 0; attesa CPU 0 ms; mappa 10004293->10004293 (capacity 14680064->14680064)
blocco 12017 (altro): 105 ms = esecuzione 58 + hash 20 + firma 1 + write 0.4 + sync_data 1.7 + pubblicazione 0.2; minflt 0 majflt 0; attesa CPU 0 ms; mappa 10004293->10004293 (capacity 14680064->14680064)
blocco 12018 (altro): 117 ms = esecuzione 69 + hash 20 + firma 2 + write 0.5 + sync_data 1.6 + pubblicazione 0.3; minflt 0 majflt 0; attesa CPU 0 ms; mappa 10004293->10004293 (capacity 14680064->14680064)
blocco 15000 (snapshot): 10825 ms = esecuzione 7 + hash 2 + firma 0 + write 0.5 + sync_data 0.7 + pubblicazione 0.1; minflt 58954 majflt 0; attesa CPU 3 ms; mappa 10004294->10004294 (capacity 14680064->14680064)
blocco 15010 (altro): 122 ms = esecuzione 69 + hash 20 + firma 1 + write 0.5 + sync_data 1.9 + pubblicazione 0.3; minflt 0 majflt 0; attesa CPU 0 ms; mappa 10004294->10004294 (capacity 14680064->14680064)
blocco 15011 (altro): 103 ms = esecuzione 59 + hash 19 + firma 1 + write 0.4 + sync_data 1.8 + pubblicazione 0.3; minflt 0 majflt 0; attesa CPU 0 ms; mappa 10004294->10004294 (capacity 14680064->14680064)
blocco 15012 (altro): 110 ms = esecuzione 61 + hash 21 + firma 1 + write 1.1 + sync_data 1.7 + pubblicazione 0.3; minflt 0 majflt 0; attesa CPU 0 ms; mappa 10004294->10004294 (capacity 14680064->14680064)
blocco 15013 (altro): 111 ms = esecuzione 62 + hash 19 + firma 1 + write 0.5 + sync_data 1.6 + pubblicazione 0.2; minflt 0 majflt 0; attesa CPU 0 ms; mappa 10004294->10004294 (capacity 14680064->14680064)
blocco 15014 (altro): 107 ms = esecuzione 63 + hash 20 + firma 1 + write 2.1 + sync_data 1.9 + pubblicazione 0.2; minflt 0 majflt 0; attesa CPU 0 ms; mappa 10004294->10004294 (capacity 14680064->14680064)
blocco 15015 (altro): 107 ms = esecuzione 62 + hash 19 + firma 1 + write 1.1 + sync_data 1.7 + pubblicazione 0.3; minflt 0 majflt 0; attesa CPU 0 ms; mappa 10004294->10004294 (capacity 14680064->14680064)
blocco 15016 (altro): 106 ms = esecuzione 61 + hash 19 + firma 1 + write 0.6 + sync_data 1.7 + pubblicazione 0.2; minflt 0 majflt 0; attesa CPU 0 ms; mappa 10004294->10004294 (capacity 14680064->14680064)
blocco 15017 (altro): 105 ms = esecuzione 60 + hash 19 + firma 1 + write 0.4 + sync_data 2.0 + pubblicazione 0.3; minflt 0 majflt 0; attesa CPU 0 ms; mappa 10004294->10004294 (capacity 14680064->14680064)
blocco 15018 (altro): 110 ms = esecuzione 59 + hash 22 + firma 1 + write 0.4 + sync_data 1.7 + pubblicazione 0.2; minflt 0 majflt 0; attesa CPU 0 ms; mappa 10004294->10004294 (capacity 14680064->14680064)
blocco 15019 (altro): 117 ms = esecuzione 73 + hash 20 + firma 1 + write 0.5 + sync_data 1.8 + pubblicazione 0.2; minflt 0 majflt 0; attesa CPU 0 ms; mappa 10004294->10004294 (capacity 14680064->14680064)
blocco 15021 (altro): 113 ms = esecuzione 69 + hash 19 + firma 1 + write 0.6 + sync_data 1.8 + pubblicazione 0.2; minflt 0 majflt 0; attesa CPU 0 ms; mappa 10004294->10004294 (capacity 14680064->14680064)
blocco 15025 (disco (sync_data)): 110 ms = esecuzione 12 + hash 2 + firma 0 + write 0.1 + sync_data 91.8 + pubblicazione 0.1; minflt 0 majflt 0; attesa CPU 0 ms; mappa 10004294->10004294 (capacity 14680064->14680064)
blocco 18000 (snapshot): 11390 ms = esecuzione 10 + hash 2 + firma 0 + write 0.5 + sync_data 0.9 + pubblicazione 0.1; minflt 59300 majflt 0; attesa CPU 1 ms; mappa 10004295->10004295 (capacity 14680064->14680064)
blocco 18010 (altro): 111 ms = esecuzione 61 + hash 18 + firma 1 + write 0.5 + sync_data 2.1 + pubblicazione 0.3; minflt 0 majflt 0; attesa CPU 0 ms; mappa 10004295->10004295 (capacity 14680064->14680064)
blocco 18011 (altro): 112 ms = esecuzione 66 + hash 20 + firma 1 + write 1.6 + sync_data 1.8 + pubblicazione 0.3; minflt 0 majflt 0; attesa CPU 0 ms; mappa 10004295->10004295 (capacity 14680064->14680064)
blocco 18012 (altro): 102 ms = esecuzione 57 + hash 18 + firma 1 + write 0.4 + sync_data 1.8 + pubblicazione 0.3; minflt 0 majflt 0; attesa CPU 0 ms; mappa 10004295->10004295 (capacity 14680064->14680064)
blocco 18013 (altro): 117 ms = esecuzione 65 + hash 20 + firma 2 + write 1.2 + sync_data 2.0 + pubblicazione 0.3; minflt 0 majflt 0; attesa CPU 0 ms; mappa 10004295->10004295 (capacity 14680064->14680064)
blocco 18014 (altro): 143 ms = esecuzione 87 + hash 23 + firma 2 + write 4.2 + sync_data 1.9 + pubblicazione 0.4; minflt 0 majflt 0; attesa CPU 0 ms; mappa 10004295->10004295 (capacity 14680064->14680064)
blocco 18016 (altro): 102 ms = esecuzione 59 + hash 20 + firma 1 + write 0.4 + sync_data 1.5 + pubblicazione 0.3; minflt 0 majflt 0; attesa CPU 0 ms; mappa 10004295->10004295 (capacity 14680064->14680064)
blocco 18018 (altro): 112 ms = esecuzione 69 + hash 19 + firma 1 + write 0.4 + sync_data 1.7 + pubblicazione 0.3; minflt 0 majflt 0; attesa CPU 0 ms; mappa 10004295->10004295 (capacity 14680064->14680064)
blocco 20348 (disco (sync_data)): 124 ms = esecuzione 7 + hash 2 + firma 0 + write 0.6 + sync_data 109.7 + pubblicazione 0.1; minflt 0 majflt 0; attesa CPU 0 ms; mappa 10004295->10004295 (capacity 14680064->14680064)
blocco 21000 (snapshot): 10575 ms = esecuzione 7 + hash 2 + firma 0 + write 0.4 + sync_data 0.9 + pubblicazione 0.1; minflt 58619 majflt 0; attesa CPU 1 ms; mappa 10004296->10004296 (capacity 14680064->14680064)
blocco 21010 (altro): 113 ms = esecuzione 63 + hash 20 + firma 2 + write 0.7 + sync_data 1.8 + pubblicazione 0.3; minflt 0 majflt 0; attesa CPU 0 ms; mappa 10004296->10004296 (capacity 14680064->14680064)
blocco 21011 (altro): 107 ms = esecuzione 63 + hash 19 + firma 1 + write 0.5 + sync_data 1.7 + pubblicazione 0.3; minflt 0 majflt 0; attesa CPU 0 ms; mappa 10004296->10004296 (capacity 14680064->14680064)
blocco 21012 (altro): 109 ms = esecuzione 63 + hash 20 + firma 1 + write 1.0 + sync_data 1.5 + pubblicazione 0.3; minflt 0 majflt 0; attesa CPU 0 ms; mappa 10004296->10004296 (capacity 14680064->14680064)
blocco 21013 (altro): 108 ms = esecuzione 61 + hash 21 + firma 1 + write 0.9 + sync_data 1.6 + pubblicazione 0.2; minflt 0 majflt 0; attesa CPU 0 ms; mappa 10004296->10004296 (capacity 14680064->14680064)
blocco 21019 (altro): 101 ms = esecuzione 56 + hash 18 + firma 1 + write 4.3 + sync_data 1.7 + pubblicazione 0.3; minflt 0 majflt 0; attesa CPU 0 ms; mappa 10004296->10004296 (capacity 14680064->14680064)
blocco 21020 (altro): 107 ms = esecuzione 62 + hash 19 + firma 1 + write 1.8 + sync_data 2.2 + pubblicazione 0.2; minflt 0 majflt 0; attesa CPU 0 ms; mappa 10004296->10004296 (capacity 14680064->14680064)
blocco 24000 (snapshot): 12027 ms = esecuzione 8 + hash 2 + firma 0 + write 0.6 + sync_data 0.8 + pubblicazione 0.2; minflt 58619 majflt 0; attesa CPU 4 ms; mappa 10004297->10004297 (capacity 14680064->14680064)
blocco 24010 (altro): 108 ms = esecuzione 61 + hash 19 + firma 1 + write 0.4 + sync_data 1.6 + pubblicazione 0.2; minflt 0 majflt 0; attesa CPU 0 ms; mappa 10004297->10004297 (capacity 14680064->14680064)
blocco 24011 (altro): 103 ms = esecuzione 58 + hash 19 + firma 1 + write 1.4 + sync_data 1.6 + pubblicazione 0.3; minflt 0 majflt 0; attesa CPU 0 ms; mappa 10004297->10004297 (capacity 14680064->14680064)
blocco 24012 (altro): 113 ms = esecuzione 61 + hash 19 + firma 1 + write 4.9 + sync_data 1.4 + pubblicazione 0.3; minflt 0 majflt 0; attesa CPU 0 ms; mappa 10004297->10004297 (capacity 14680064->14680064)
blocco 24013 (altro): 109 ms = esecuzione 61 + hash 19 + firma 1 + write 2.5 + sync_data 1.6 + pubblicazione 0.4; minflt 0 majflt 0; attesa CPU 0 ms; mappa 10004297->10004297 (capacity 14680064->14680064)
blocco 24014 (altro): 103 ms = esecuzione 58 + hash 19 + firma 1 + write 0.9 + sync_data 1.7 + pubblicazione 0.3; minflt 0 majflt 0; attesa CPU 0 ms; mappa 10004297->10004297 (capacity 14680064->14680064)
blocco 24015 (altro): 104 ms = esecuzione 60 + hash 19 + firma 1 + write 0.4 + sync_data 1.7 + pubblicazione 0.2; minflt 0 majflt 0; attesa CPU 0 ms; mappa 10004297->10004297 (capacity 14680064->14680064)
blocco 24016 (altro): 121 ms = esecuzione 61 + hash 22 + firma 2 + write 4.4 + sync_data 1.7 + pubblicazione 0.2; minflt 0 majflt 0; attesa CPU 0 ms; mappa 10004297->10004297 (capacity 14680064->14680064)
blocco 24017 (altro): 105 ms = esecuzione 62 + hash 19 + firma 1 + write 0.4 + sync_data 1.7 + pubblicazione 0.2; minflt 0 majflt 0; attesa CPU 0 ms; mappa 10004297->10004297 (capacity 14680064->14680064)
blocco 24018 (altro): 105 ms = esecuzione 59 + hash 19 + firma 1 + write 1.9 + sync_data 1.7 + pubblicazione 0.2; minflt 0 majflt 0; attesa CPU 0 ms; mappa 10004297->10004297 (capacity 14680064->14680064)
blocco 24019 (altro): 116 ms = esecuzione 74 + hash 19 + firma 1 + write 0.5 + sync_data 1.4 + pubblicazione 0.2; minflt 0 majflt 0; attesa CPU 0 ms; mappa 10004297->10004297 (capacity 14680064->14680064)
blocco 24020 (altro): 103 ms = esecuzione 59 + hash 20 + firma 1 + write 0.4 + sync_data 1.5 + pubblicazione 0.2; minflt 0 majflt 0; attesa CPU 0 ms; mappa 10004297->10004297 (capacity 14680064->14680064)
blocco 24021 (altro): 109 ms = esecuzione 60 + hash 19 + firma 1 + write 5.0 + sync_data 1.7 + pubblicazione 0.3; minflt 0 majflt 0; attesa CPU 0 ms; mappa 10004297->10004297 (capacity 14680064->14680064)
blocco 24022 (altro): 105 ms = esecuzione 58 + hash 19 + firma 1 + write 3.7 + sync_data 1.5 + pubblicazione 0.2; minflt 0 majflt 0; attesa CPU 0 ms; mappa 10004297->10004297 (capacity 14680064->14680064)
blocco 27000 (snapshot): 11299 ms = esecuzione 8 + hash 2 + firma 0 + write 0.4 + sync_data 0.9 + pubblicazione 0.2; minflt 58619 majflt 0; attesa CPU 2 ms; mappa 10004298->10004298 (capacity 14680064->14680064)
blocco 27010 (altro): 120 ms = esecuzione 69 + hash 20 + firma 1 + write 0.5 + sync_data 1.6 + pubblicazione 0.2; minflt 0 majflt 0; attesa CPU 0 ms; mappa 10004298->10004298 (capacity 14680064->14680064)
blocco 27011 (altro): 145 ms = esecuzione 85 + hash 26 + firma 2 + write 0.5 + sync_data 1.6 + pubblicazione 0.3; minflt 0 majflt 0; attesa CPU 0 ms; mappa 10004298->10004298 (capacity 14680064->14680064)
blocco 27013 (altro): 132 ms = esecuzione 81 + hash 21 + firma 2 + write 0.4 + sync_data 2.0 + pubblicazione 0.3; minflt 0 majflt 0; attesa CPU 0 ms; mappa 10004298->10004298 (capacity 14680064->14680064)
blocco 27014 (altro): 122 ms = esecuzione 67 + hash 22 + firma 1 + write 1.7 + sync_data 2.2 + pubblicazione 0.3; minflt 0 majflt 0; attesa CPU 0 ms; mappa 10004298->10004298 (capacity 14680064->14680064)
blocco 27015 (altro): 140 ms = esecuzione 82 + hash 24 + firma 2 + write 1.3 + sync_data 1.8 + pubblicazione 0.5; minflt 0 majflt 0; attesa CPU 0 ms; mappa 10004298->10004298 (capacity 14680064->14680064)
blocco 27016 (altro): 136 ms = esecuzione 81 + hash 23 + firma 2 + write 0.6 + sync_data 1.8 + pubblicazione 0.3; minflt 0 majflt 0; attesa CPU 0 ms; mappa 10004298->10004298 (capacity 14680064->14680064)
blocco 27017 (altro): 145 ms = esecuzione 82 + hash 24 + firma 2 + write 3.4 + sync_data 1.9 + pubblicazione 0.4; minflt 0 majflt 0; attesa CPU 0 ms; mappa 10004298->10004298 (capacity 14680064->14680064)
blocco 27019 (altro): 141 ms = esecuzione 87 + hash 21 + firma 2 + write 0.8 + sync_data 1.8 + pubblicazione 0.2; minflt 0 majflt 0; attesa CPU 0 ms; mappa 10004298->10004298 (capacity 14680064->14680064)
blocco 27020 (altro): 119 ms = esecuzione 67 + hash 22 + firma 2 + write 0.4 + sync_data 1.8 + pubblicazione 0.2; minflt 0 majflt 0; attesa CPU 0 ms; mappa 10004298->10004298 (capacity 14680064->14680064)
blocco 27021 (altro): 104 ms = esecuzione 59 + hash 20 + firma 1 + write 0.6 + sync_data 2.0 + pubblicazione 0.3; minflt 0 majflt 0; attesa CPU 0 ms; mappa 10004298->10004298 (capacity 14680064->14680064)
blocco 27022 (altro): 110 ms = esecuzione 63 + hash 21 + firma 1 + write 0.5 + sync_data 2.0 + pubblicazione 0.2; minflt 0 majflt 0; attesa CPU 0 ms; mappa 10004298->10004298 (capacity 14680064->14680064)
blocco 27023 (altro): 145 ms = esecuzione 91 + hash 21 + firma 2 + write 0.5 + sync_data 1.8 + pubblicazione 0.3; minflt 0 majflt 0; attesa CPU 0 ms; mappa 10004298->10004298 (capacity 14680064->14680064)
blocco 27025 (altro): 107 ms = esecuzione 64 + hash 19 + firma 1 + write 0.4 + sync_data 1.8 + pubblicazione 0.2; minflt 0 majflt 0; attesa CPU 0 ms; mappa 10004298->10004298 (capacity 14680064->14680064)
blocco 27797 (disco (sync_data)): 210 ms = esecuzione 74 + hash 3 + firma 0 + write 0.1 + sync_data 127.7 + pubblicazione 0.1; minflt 0 majflt 0; attesa CPU 0 ms; mappa 10004298->10004298 (capacity 14680064->14680064)
blocco 27800 (altro): 387 ms = esecuzione 371 + hash 6 + firma 0 + write 0.3 + sync_data 1.0 + pubblicazione 0.1; minflt 0 majflt 0; attesa CPU 0 ms; mappa 10004298->10004298 (capacity 14680064->14680064)
blocco 29143 (altro): 121 ms = esecuzione 111 + hash 3 + firma 0 + write 0.7 + sync_data 1.0 + pubblicazione 0.0; minflt 0 majflt 0; attesa CPU 0 ms; mappa 10004299->10004299 (capacity 14680064->14680064)
blocco 29643 (disco (sync_data)): 135 ms = esecuzione 7 + hash 2 + firma 0 + write 0.4 + sync_data 121.3 + pubblicazione 0.1; minflt 0 majflt 0; attesa CPU 0 ms; mappa 10004299->10004299 (capacity 14680064->14680064)
blocco 30000 (snapshot): 13361 ms = esecuzione 10 + hash 2 + firma 0 + write 0.9 + sync_data 1.8 + pubblicazione 0.1; minflt 58619 majflt 0; attesa CPU 5 ms; mappa 10004299->10004299 (capacity 14680064->14680064)
blocco 30010 (altro): 120 ms = esecuzione 72 + hash 20 + firma 1 + write 0.5 + sync_data 1.6 + pubblicazione 0.3; minflt 0 majflt 0; attesa CPU 0 ms; mappa 10004299->10004299 (capacity 14680064->14680064)
blocco 30011 (altro): 119 ms = esecuzione 63 + hash 20 + firma 1 + write 4.6 + sync_data 2.0 + pubblicazione 0.3; minflt 0 majflt 0; attesa CPU 0 ms; mappa 10004299->10004299 (capacity 14680064->14680064)
blocco 30012 (altro): 130 ms = esecuzione 68 + hash 20 + firma 1 + write 6.0 + sync_data 1.8 + pubblicazione 0.8; minflt 0 majflt 0; attesa CPU 0 ms; mappa 10004299->10004299 (capacity 14680064->14680064)
blocco 30013 (altro): 170 ms = esecuzione 97 + hash 25 + firma 3 + write 8.8 + sync_data 2.4 + pubblicazione 0.3; minflt 0 majflt 0; attesa CPU 3 ms; mappa 10004299->10004299 (capacity 14680064->14680064)
blocco 30015 (altro): 156 ms = esecuzione 97 + hash 25 + firma 2 + write 1.4 + sync_data 2.0 + pubblicazione 0.4; minflt 0 majflt 0; attesa CPU 0 ms; mappa 10004299->10004299 (capacity 14680064->14680064)
blocco 30017 (altro): 128 ms = esecuzione 74 + hash 24 + firma 1 + write 0.7 + sync_data 3.2 + pubblicazione 0.3; minflt 0 majflt 0; attesa CPU 0 ms; mappa 10004299->10004299 (capacity 14680064->14680064)
blocco 30018 (altro): 133 ms = esecuzione 62 + hash 22 + firma 2 + write 0.4 + sync_data 22.2 + pubblicazione 0.2; minflt 0 majflt 0; attesa CPU 0 ms; mappa 10004299->10004299 (capacity 14680064->14680064)
blocco 30019 (disco (sync_data)): 301 ms = esecuzione 65 + hash 18 + firma 1 + write 0.4 + sync_data 187.2 + pubblicazione 0.4; minflt 0 majflt 0; attesa CPU 0 ms; mappa 10004299->10004299 (capacity 14680064->14680064)
blocco 30022 (altro): 148 ms = esecuzione 93 + hash 23 + firma 1 + write 0.6 + sync_data 1.9 + pubblicazione 0.3; minflt 0 majflt 0; attesa CPU 0 ms; mappa 10004299->10004299 (capacity 14680064->14680064)
blocco 30024 (altro): 136 ms = esecuzione 76 + hash 25 + firma 2 + write 2.9 + sync_data 2.7 + pubblicazione 0.4; minflt 0 majflt 0; attesa CPU 0 ms; mappa 10004299->10004299 (capacity 14680064->14680064)
blocco 30025 (altro): 181 ms = esecuzione 92 + hash 29 + firma 2 + write 5.9 + sync_data 25.3 + pubblicazione 0.3; minflt 0 majflt 0; attesa CPU 0 ms; mappa 10004299->10004299 (capacity 14680064->14680064)
blocco 30027 (altro): 174 ms = esecuzione 66 + hash 22 + firma 2 + write 5.7 + sync_data 53.0 + pubblicazione 0.3; minflt 0 majflt 0; attesa CPU 0 ms; mappa 10004299->10004299 (capacity 14680064->14680064)
blocco 30029 (altro): 132 ms = esecuzione 75 + hash 21 + firma 2 + write 4.1 + sync_data 2.3 + pubblicazione 0.3; minflt 0 majflt 0; attesa CPU 0 ms; mappa 10004299->10004299 (capacity 14680064->14680064)
blocco 30030 (altro): 134 ms = esecuzione 78 + hash 23 + firma 2 + write 4.6 + sync_data 1.6 + pubblicazione 0.3; minflt 0 majflt 0; attesa CPU 0 ms; mappa 10004299->10004299 (capacity 14680064->14680064)
blocco 30031 (altro): 119 ms = esecuzione 66 + hash 23 + firma 1 + write 4.8 + sync_data 1.9 + pubblicazione 0.2; minflt 0 majflt 0; attesa CPU 0 ms; mappa 10004299->10004299 (capacity 14680064->14680064)
blocco 33000 (snapshot): 11003 ms = esecuzione 6 + hash 2 + firma 0 + write 0.5 + sync_data 0.7 + pubblicazione 0.1; minflt 58619 majflt 0; attesa CPU 1 ms; mappa 10004300->10004300 (capacity 14680064->14680064)
blocco 33010 (altro): 114 ms = esecuzione 66 + hash 20 + firma 1 + write 0.7 + sync_data 1.8 + pubblicazione 0.4; minflt 0 majflt 0; attesa CPU 0 ms; mappa 10004300->10004300 (capacity 14680064->14680064)
blocco 33011 (altro): 108 ms = esecuzione 63 + hash 19 + firma 1 + write 0.5 + sync_data 1.7 + pubblicazione 0.3; minflt 0 majflt 0; attesa CPU 0 ms; mappa 10004300->10004300 (capacity 14680064->14680064)
blocco 33012 (altro): 109 ms = esecuzione 61 + hash 20 + firma 1 + write 0.4 + sync_data 1.9 + pubblicazione 0.4; minflt 0 majflt 0; attesa CPU 0 ms; mappa 10004300->10004300 (capacity 14680064->14680064)
blocco 33013 (altro): 112 ms = esecuzione 59 + hash 20 + firma 1 + write 0.4 + sync_data 1.9 + pubblicazione 0.3; minflt 0 majflt 0; attesa CPU 0 ms; mappa 10004300->10004300 (capacity 14680064->14680064)
blocco 33014 (altro): 111 ms = esecuzione 64 + hash 21 + firma 1 + write 0.8 + sync_data 1.9 + pubblicazione 0.2; minflt 0 majflt 0; attesa CPU 0 ms; mappa 10004300->10004300 (capacity 14680064->14680064)
blocco 33015 (altro): 152 ms = esecuzione 70 + hash 32 + firma 2 + write 1.0 + sync_data 23.8 + pubblicazione 0.3; minflt 0 majflt 0; attesa CPU 0 ms; mappa 10004300->10004300 (capacity 14680064->14680064)
blocco 33017 (altro): 113 ms = esecuzione 60 + hash 21 + firma 1 + write 3.9 + sync_data 1.7 + pubblicazione 0.3; minflt 0 majflt 0; attesa CPU 0 ms; mappa 10004300->10004300 (capacity 14680064->14680064)
blocco 33018 (altro): 113 ms = esecuzione 63 + hash 21 + firma 1 + write 4.5 + sync_data 1.5 + pubblicazione 0.3; minflt 0 majflt 0; attesa CPU 0 ms; mappa 10004300->10004300 (capacity 14680064->14680064)
blocco 33019 (altro): 112 ms = esecuzione 63 + hash 19 + firma 1 + write 4.5 + sync_data 1.7 + pubblicazione 0.2; minflt 0 majflt 0; attesa CPU 0 ms; mappa 10004300->10004300 (capacity 14680064->14680064)
blocco 33020 (altro): 108 ms = esecuzione 60 + hash 21 + firma 1 + write 4.3 + sync_data 1.6 + pubblicazione 0.3; minflt 0 majflt 0; attesa CPU 0 ms; mappa 10004300->10004300 (capacity 14680064->14680064)
blocco 33021 (altro): 108 ms = esecuzione 60 + hash 19 + firma 1 + write 4.5 + sync_data 1.8 + pubblicazione 0.2; minflt 0 majflt 0; attesa CPU 0 ms; mappa 10004300->10004300 (capacity 14680064->14680064)
blocco 33022 (altro): 119 ms = esecuzione 69 + hash 21 + firma 2 + write 4.2 + sync_data 1.7 + pubblicazione 0.3; minflt 0 majflt 0; attesa CPU 0 ms; mappa 10004300->10004300 (capacity 14680064->14680064)
invio→inclusione (18000 campioni, 1 ogni ~1000 tx, risoluzione ~40 ms): p50 81 / p95 135 / p99 8562 / max 13122 ms
CPU media (% di un core): nodo 17, rete 0, client 31, altro 0
RSS intorno agli snapshot (altezza: prima -> 1 min dopo): 3000: 2906 -> 2965, 6000: 2981 -> 2992, 9000: 2992 -> 2998, 12000: 2998 -> 3006, 15000: 3006 -> 3007, 18000: 3007 -> 3011, 21000: 3011 -> 3011, 24000: 3011 -> 3013, 27000: 3013 -> 3013, 30000: 3013 -> 3028, 33000: 3028 -> 3028
journal 3948 MB = 219 byte/tx, 3947 MB/ora; snapshot 892 MB; status.bin 11.4 MB
stati: 60 trovati e 59 scaduti come atteso (errori 0 / 1); wallet sotto carico: 12 ok, 0 errori
snapshot durante il carico: 11 (scrittura dello snapshot nel suo blocco: 10876 ms, 9458 ms, 10294 ms, 10482 ms, 10811 ms, 11374 ms, 10561 ms, 12012 ms, 11283 ms, 13340 ms, 10990 ms), massimo 13340 ms
FINE

## Classificazione dei 135 blocchi oltre 100 ms

La tabella automatica sopra mette in "altro" anche i blocchi di recupero dopo uno
snapshot; qui sono separati, guardando le fasi di ogni blocco (elenco sopra).

| causa | blocchi | max ms | da cosa si vede |
|---|---|---|---|
| snapshot sincrono (main) | 11 | 13.361 | il blocco dello snapshot: 9,5–13,4 s, minflt ~61.000 |
| recupero dopo lo snapshot | 113 | 181 | entro 31 blocchi da uno snapshot: blocchi pieni delle tx accumulate, esecuzione 57–85 ms + hash ~20 ms; sync_data ~2 ms |
| disco: sync_data | 8 | 308 | sync_data 92–274 ms. 3 all'avvio (blocchi 112–117, mentre il disco scrive lo snapshot iniziale da 1 GB), 2 vicino a snapshot (15025, 30019), 3 altrove |
| write del journal | 1 | 103 | write 84,5 ms (blocco 7704) |
| esecuzione lenta senza altra causa | 2 | 387 | esecuzione 111 e 371 ms, nessuna attesa in coda, nessun page fault (27800, 29143) |
| resize della mappa | 0 | — | capacity sempre 14.680.064 per ~10.004.300 account |
| page fault | 0 | — | majflt sempre 0; minflt alti solo nei blocchi di snapshot |
| CPU rubata nel guest | 0 | — | attesa in coda (schedstat) 0–3 ms; `vmstat` st medio 0,14 %, max 16 % per 3 secondi |

**Conclusione**: con lo snapshot sincrono di main il blocco di snapshot (10 s) e il
recupero che segue fanno 124 dei 135 blocchi lenti. Il resto è disco (sync_data e una
write lenta, 9 blocchi) e due esecuzioni lente isolate. Nessun resize, page fault o CPU
rubata. Lo snapshot in background (PR #13) e il journal su un thread separato (PR B)
coprono le prime due cause; le due esecuzioni lente restano da osservare.

**Nota sull'ambiente**: intorno al minuto 9 THP defrag è stato cambiato per errore da
`madvise` a `never` e rimesso subito a `madvise`; `AnonHugePages` era 0 kB (il processo
non usa THP), quindi nessun effetto atteso.
