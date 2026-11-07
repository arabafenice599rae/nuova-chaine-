# Storico delle transazioni

## Divisione dei compiti

**Nodo**: solo RAM, a dimensione limitata.
- Tiene gli stati (slot, errore) delle tx degli ultimi `blockhash_window` blocchi: 300
  slot, la finestra di scadenza dei blockhash. Sono dimensionati all'avvio e salvati in
  `status.bin` accanto allo snapshot, così sopravvivono a un riavvio.
- Tiene i record completi delle ultime `rpc_recent_txs` tx (default 20.000), per
  `getTransaction`.
- Non scrive storico sul proprio disco: il disco del nodo contiene solo `journal.bin`,
  `snapshot.bin` e `status.bin`.

**Indexer** (`centrifuga indexer <config>`): facoltativo, è un processo separato.
- Legge il journal del nodo in sola lettura, anche mentre cresce.
- Riesegue i blocchi su uno stato suo, perché log, errori e fee non sono nel journal, e
  verifica firme e hash di stato come una replica.
- Scrive l'archivio e serve `getTransaction`, `getSignaturesForAddress` e
  `getSignatureStatuses` (storico).

**Nodo e indexer insieme**:
- Con `indexer_rpc` impostato, il nodo inoltra all'indexer `getSignaturesForAddress` e
  le richieste su tx fuori dalla sua finestra. L'inoltro avviene sui thread di rete:
  il thread del nodo non aspetta mai l'indexer.
- Senza indexer, o se l'indexer non risponde, il nodo dà l'errore `-32011`.
  `centrifuga wallet history` lo mostra come "history not available on this node: …".
- L'indexer può essere spento o in ritardo: il nodo produce blocchi lo stesso. L'indexer
  recupera da solo dal journal.

**Recupero dell'indexer**:
- Ha un suo snapshot dello stato e un checkpoint dell'archivio (`snapshot_every`).
- Se l'archivio è illeggibile, o indietro rispetto al suo stato, lo mette da parte e lo
  ricostruisce dal journal a partire dal genesis.

**Conservazione**: `retention_blocks` (0 = tutto). Si eliminano interi segmenti.

## Archivio: confronto e scelta

**Scelto: file di segmenti append-only** (`src/segments.rs`). Gli altri due
sono stati rimossi dopo il confronto: l'archivio a pagine scritto a mano e `redb` 4.4.0
(feature `redb-archive`).

Misure su 2.000.000 di tx reali (trasferimenti firmati, 2 righe di log, 3 saldi), in
blocchi da 2.000, su una macchina con 4 vCPU Xeon a 2,1 GHz e disco virtuale:

| | segmenti | pagine (scritto a mano) | redb 4.4.0 |
|---|---|---|---|
| righe (senza commenti e vuote) | 274 | 343 | 136 (+ il crate) |
| dipendenze nuove | nessuna | nessuna | `redb` (senza dipendenze proprie) |
| byte per tx su disco | 546–614 | 724 | 1.078–1.800 |
| scrittura | 270.000 tx/s | 245.000 tx/s | 29.000 tx/s |
| tempo per blocco da 2.000 tx (p50 / p99) | 4,7 / 7,7 ms | 8,0 / 15,5 ms | 69 / 107 ms |
| ricerca per firma (p50 / p99) | 6 / 17 µs | 2 / 21 µs | 4 / 15 µs |
| ricerca di una firma assente | 7 µs | 1 µs | 2 µs |
| cronologia (10 voci) | 11 µs | 9 µs | 10 µs |
| checkpoint | 0,4 s | 1,1 s | 2,6 s |
| riapertura | 1,7 s (rilegge il segmento attivo) | 0,01 s | ~0 s |
| arresto a metà | tronca l'ultimo blocco, si riapre in 1,6 s | tronca l'ultimo blocco | file rifiutato ("DB corrupted"): ricostruzione dal journal |
| conservazione limitata | sì, per segmento (istantanea) | no | sì (cancellazione per intervallo) |

**Rischi di ciascuna**:
- **Segmenti**:
  - la ricerca per firma scende di segmento in segmento: il costo cresce con il numero
    di segmenti conservati (ricerca binaria su file, circa 20 letture per segmento);
  - il segmento attivo tiene i suoi indici in RAM, al massimo `segment_txs` tx, quindi
    circa 100 MB con 1 milione di tx.
- **Pagine**:
  - in caso di interruzione di corrente una pagina dell'indice scritta a metà può perdere
    voci senza che nessuno se ne accorga;
  - la copia su scrittura lascia pagine inutilizzate dentro il file;
  - non c'è conservazione limitata;
  - è il codice più complesso dei tre.
- **redb**:
  - è 9 volte più lento in scrittura (69 ms per un blocco da 2.000 tx);
  - occupa 2 volte più spazio;
  - un file danneggiato viene rifiutato e serve una ricostruzione completa;
  - in cambio dà transazioni ACID collaudate.

## Limite del nodo, con e senza indexer

Nodo in tempo reale (un blocco ogni 100 ms). L'indexer gira sulla stessa macchina e
segue il journal. Il limite è il carico massimo con p99 del blocco sotto 100 ms, su
gradini da 100 a 3.000 tx per blocco.

| account | senza indexer | con indexer | indexer alla fine |
|---|---|---|---|
| 100.000 | ≥ 30.000 tx/s (p99 74 ms a 3.000 tx/blocco) | ≥ 30.000 tx/s (p99 74 ms) | 0 blocchi indietro |
| 1.000.000 | nessun gradino: p99 476–530 ms già a 100 tx/blocco | idem | 0 blocchi indietro |

Note:
- **Sopra i 30.000 tx/s non si è misurato**: 3.000 tx per blocco era l'ultimo gradino.
  L'indexer non sposta il p99 in modo misurabile.
- **A 1.000.000 di account il blocco costa circa 400 ms** qualunque sia il carico:
  questo ramo calcola ancora l'hash di stato SHA-256 completo su tutti gli account
  (misurato da solo: 440–490 ms). L'hash incrementale LtHash è nella PR #9, non ancora
  scelta.
- **Questo è il limite del nodo, non il throughput end-to-end**: le tx entrano
  direttamente nell'ingresso, senza HTTP né JSON-RPC.

## Crescita del disco

| componente | byte/tx | a 11.000 tx/s | a 30.000 tx/s |
|---|---|---|---|
| journal (nodo) | 219 | 8,7 GB/ora | 23,7 GB/ora |
| archivio a segmenti (indexer) | 546–614 | 21,6 GB/ora | 59 GB/ora |
| archivio a pagine (scartato) | 714 | 28 GB/ora | 78 GB/ora |
| redb (scartato) | 1.350–1.800 | 53 GB/ora | 194 GB/ora |

11.000 tx/s è il throughput misurato via RPC (vedi `docs/load-test.md`).
