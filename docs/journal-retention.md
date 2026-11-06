# Compressione del journal e retention (misura e proposta, nessun codice del nodo)

## 1. Misura della compressione

**Dati**: i primi 400 MB del `journal.bin` del soak RPC a 1 milione di account
(`docs/soak/rpc-limit-1m-gradini-48client.md`): trasferimenti di CnFA tra 4.000
mittenti e 1 milione di destinatari, 219 byte per tx, ~500–900 tx per blocco.

**Strumento**: `scripts/journal-compression.py` (Python con i moduli `zstandard` e `lz4`,
installati solo per la misura: non sono dipendenze del nodo). Ogni variante viene
decompressa e confrontata con l'originale.

```
python3 -m venv /tmp/cvenv && /tmp/cvenv/bin/pip install zstandard lz4
/tmp/cvenv/bin/python -I scripts/journal-compression.py <journal.bin> 400000000
```

Una macchina con 4 vCPU (Xeon 2,1 GHz), un solo core, nessun dizionario.

Colonne: byte per tx dopo la compressione (219 senza); velocità in MB/s di
compressione e di decompressione; costo per blocco, cioè il tempo per comprimere e
decomprimere un blocco tipico a 5.000 tx/s (500 tx, ~110 KB); CPU per ora di journal a
5.000 tx/s (3,9 GB). **Verifica byte per byte: superata per tutte le righe**: lo script
decomprime ogni segmento e ogni blocco e lo confronta con l'originale
(`assert back == parts`), poi stampa la riga.

| algoritmo | unità | rapporto | byte/tx | MB/s compr. / decompr. | costo per blocco (compr. / decompr.) | CPU per ora di journal (compr.) |
|---|---|---|---|---|---|---|
| lz4 | segmenti da 4 MB | 1,94× | 113 | 847 / 2.722 | 0,13 ms / 0,04 ms | 5 s |
| zstd-1 | segmenti da 4 MB | 2,09× | 105 | 649 / 299 | 0,17 ms / 0,37 ms | 6 s |
| zstd-3 | segmenti da 4 MB | 2,42× | 90 | 185 / 370 | 0,59 ms / 0,30 ms | 21 s |
| zstd-9 | segmenti da 4 MB | 2,67× | 82 | 72 / 365 | 1,52 ms / 0,30 ms | 55 s |
| zstd-19 | segmenti da 4 MB | 2,70× | 81 | 3 / 2.233 | 36,5 ms / 0,05 ms | 1.314 s |
| gzip-6 | segmenti da 4 MB | 2,04× | 107 | 52 / 316 | 2,11 ms / 0,35 ms | 76 s |
| xz-6 | segmenti da 4 MB | 2,70× | 81 | 4 / 76 | 27,4 ms / 1,44 ms | 986 s |
| lz4 | singolo blocco | 1,94× | 113 | 1.039 / 3.587 | 0,11 ms / 0,03 ms | 4 s |
| zstd-1 | singolo blocco | 2,09× | 105 | 532 / 1.594 | 0,21 ms / 0,07 ms | 7 s |
| zstd-3 | singolo blocco | 2,08× | 105 | 388 / 2.329 | 0,28 ms / 0,05 ms | 10 s |
| zstd-9 | singolo blocco | 2,10× | 104 | 95 / 2.744 | 1,15 ms / 0,04 ms | 41 s |
| gzip-6 | singolo blocco | 2,04× | 107 | 56 / 313 | 1,96 ms / 0,35 ms | 70 s |

Nella proposta (§2) la compressione non è sul thread del nodo: il costo per blocco indica
quanta CPU serve all'archiviazione, non un ritardo dei blocchi.

**Velocità ripetute (segmenti da 4 MB, 3 giri, macchina ferma)**: mediana e i tre giri
in MB/s; ogni giro verifica di nuovo byte per byte.
`REPEAT=3 ONLY_SEGMENTS=1 python -I scripts/journal-compression.py <journal.bin> 400000000`

| algoritmo | rapporto | byte/tx | compressione | decompressione |
|---|---|---|---|---|
| lz4 | 1,94× | 113 | 980 (948 / 980 / 1.005) | 3.566 (2.681 / 3.566 / 3.617) |
| zstd-1 | 2,09× | 105 | 616 (616 / 393 / 674) | 1.573 (1.573 / 381 / 1.650) |
| zstd-3 | 2,42× | 90 | 306 (317 / 306 / 280) | 2.660 (422 / 2.680 / 2.660) |
| **zstd-9** | **2,67×** | **82** | **86 (88 / 86 / 86)** | **3.016 (3.041 / 3.016 / 2.858)** |
| zstd-19 | 2,70× | 81 | 3 (3 / 3 / 3) | 2.641 (2.674 / 2.641 / 2.622) |
| gzip-6 | 2,04× | 107 | 51 (53 / 50 / 51) | 316 (316 / 316 / 310) |
| xz-6 | 2,70× | 81 | 4 (4 / 4 / 4) | 74 (74 / 73 / 75) |

- zstd **non** decomprime più lentamente ai livelli alti: zstd-9 è a ~3.000 MB/s, come
  zstd-3 e zstd-19. I ~300 MB/s della tabella precedente erano primi giri a freddo (si
  vedono ancora come valori isolati: zstd-1 381, zstd-3 422).
- zstd-9 comprime a ~86 MB/s: un segmento da 4 MB in ~50 ms, un'ora di journal a
  5.000 tx/s (3,9 GB) in ~45 s di CPU, fuori dal thread del nodo.

**Lettura**
- Il limite è nelle firme: 64 byte casuali per tx su 219 non si comprimono. Il resto
  (chiavi ripetute, blockhash uguale in tutto il blocco, istruzioni identiche) sì.
- Comprimere blocco per blocco rende circa 2,1×, qualunque algoritmo. Su segmenti interi
  zstd guadagna ancora (2,4× al livello 3, 2,7× al massimo), perché vede le chiavi
  ripetute tra blocchi.
- **Il carico è sintetico**: tutte transfer, poche migliaia di mittenti. Con programmi
  diversi e più mittenti il rapporto reale sarà più basso. Va rimisurato sul journal di
  devnet quando ci sarà traffico vero.

**Spazio** (219 byte per tx, 5.000 tx/s = 3,9 GB/ora): 95 GB al giorno senza
compressione; ~39 GB al giorno con zstd-3 a segmenti; **~35 GB al giorno con zstd-9**
(default dell'archivio).

## 2. Retention (disegno approvato)

Disattivata per default (`journal_retention = false`). **Nulla si cancella senza una
copia verificata per hash in almeno 2 archivi indipendenti.**

**Sul nodo**
- Journal a segmenti: un file per intervallo tra due snapshot,
  `journal-<prima altezza>.bin`, **non compresso**: il nodo scrive e rilegge solo questi
  (riavvio, recupero dello stream, indexer vicino alla testa).
- Restano sul nodo: gli ultimi due snapshot e i segmenti da (penultimo snapshot −
  `blockhash_window`) in avanti.

**Manifest di ogni segmento chiuso** (JSON, pubblico)
- Contenuto: prima e ultima altezza, SHA-256 del segmento non compresso, `prev_hash` del
  primo blocco, hash dell'ultimo blocco, SHA-256 del file compresso, livello zstd.
- Dove si pubblica:
  - via RPC: `centrifugaGetJournalManifest([altezza?])` restituisce il manifest del
    segmento che contiene l'altezza, oppure l'elenco di tutti;
  - come file nell'archivio: `manifest-<prima altezza>.json` accanto al segmento.
- Con i manifest chiunque può verificare l'archivio senza fidarsi del nodo. Ogni
  segmento si controlla con lo SHA-256; la catena si controlla dal `prev_hash` del primo
  blocco di un segmento, che è l'hash dell'ultimo del precedente. Ogni header è firmato
  dall'operatore.

**Archivi esterni**
- Due o più destinazioni configurate (cartelle o comandi di copia), indipendenti tra
  loro. Un segmento locale si cancella solo dopo che è verificato in **almeno 2**.
- Formato: un frame zstd standard per segmento, **livello 9 di default** (configurabile),
  leggibile con lo strumento `zstd` senza il codice del nodo.
- Procedura per ogni segmento chiuso:
  1. compressione in un file temporaneo;
  2. per ogni archivio: copia, rilettura, decompressione, verifica dello SHA-256 non
     compresso e della catena degli hash dal `prev_hash` del primo blocco all'hash
     dell'ultimo, scrittura del manifest accanto;
  3. solo con 2 o più archivi verificati, e solo con `journal_retention = true`,
     cancellazione del segmento locale. Il manifest resta anche sul nodo.

**Come il nodo decomprime per la verifica: comando esterno, nessun crate**
- Compressione e decompressione le fa il binario `zstd` (`zstd -9 -q -c` e
  `zstd -d -q -c`). Il nodo legge la decompressione dalla pipe e calcola SHA-256 e
  catena mentre legge, senza file intermedi.
- Con `journal_retention = true`, all'avvio il nodo esegue `zstd --version`: se il
  binario manca o non risponde, il nodo non parte e spiega cosa installare. Con la
  retention disattivata `zstd` non serve.
- Alternativa scartata: un crate (`zstd` con codice C, oppure `ruzstd` in Rust puro solo
  per la decompressione). Sarebbe una dipendenza nuova; il comando esterno ha lo stesso
  formato e si sostituisce senza toccare il nodo.

**Test richiesti**
1. Riavvio con il journal ridotto: snapshot e segmenti recenti danno lo stesso hash finale.
2. Verifica completa dagli archivi esterni: riesecuzione dal genesis leggendo i segmenti
   archiviati (decompressi) e confronto con l'hash del nodo.
3. Segmento modificato in un archivio: rifiutato (SHA-256 o catena); con un solo
   archivio verificato il segmento locale non si cancella.
4. `zstd` assente con la retention attiva: il nodo non parte, con un errore chiaro.

**Da fare quando la devnet sarà attiva**: rimisurare il rapporto sul suo journal (traffico
reale) e aggiornare la stima dello spazio.

**Righe stimate**: ~200, in una PR separata dopo lo snapshot in background (segmenti ~70,
manifest, RPC e verifica ~70, archivi e cancellazione ~60).

**Rischi**
- Rapporto reale più basso di quello sintetico: lo spazio stimato va rivisto con il
  traffico di devnet.
- Archivio configurato male: mitigato dalla verifica di 2 copie prima di cancellare.
- Riesecuzione dal genesis e indexer molto indietro: leggono dagli archivi (decompressione
  zstd ~3.000 MB/s per core).
