# fio: journal simulato, nodo fermo (15 minuti)

Nessun altro processo sul disco (dopo i run e, f, g; prima della build).

```
fio --name=j --rw=write --bs=16k --fdatasync=1 --rate_iops=10 --runtime=900 --time_based --size=1g
```

| | campioni | p50 | p99 | p99,9 | max |
|---|---|---|---|---|---|
| fdatasync | 9.000 | 0,76 ms | 3,00 ms | 23,7 ms | 129,9 ms |
| write (completamento) | 9.000 | 0,13 ms | 0,38 ms | 2,8 ms | 12,0 ms |

fdatasync oltre 20 ms: 10; oltre 50 ms: 4; oltre 100 ms: 1; oltre 200 ms: 0.

Lettura: anche da solo il disco ha stalli fino a ~130 ms (sotto la soglia di 200 ms).
Durante il soak, fuori dagli snapshot, sync_data è arrivato a 167–457 ms: il carico del
nodo (journal da ~1 MB/s più lo stream) li allunga.
