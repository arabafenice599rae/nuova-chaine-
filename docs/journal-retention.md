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

| algoritmo | unità | rapporto | MB/s compressione | MB/s decompressione |
|---|---|---|---|---|
| lz4 | segmenti da 4 MB | 1,94× | 847 | 2.722 |
| zstd-1 | segmenti da 4 MB | 2,09× | 649 | 299 |
| zstd-3 | segmenti da 4 MB | 2,42× | 185 | 370 |
| zstd-9 | segmenti da 4 MB | 2,67× | 72 | 365 |
| zstd-19 | segmenti da 4 MB | 2,70× | 3 | 2.233 |
| gzip-6 | segmenti da 4 MB | 2,04× | 52 | 316 |
| xz-6 | segmenti da 4 MB | 2,70× | 4 | 76 |
| lz4 | singolo blocco | 1,94× | 1.039 | 3.587 |
| zstd-1 | singolo blocco | 2,09× | 532 | 1.594 |
| zstd-3 | singolo blocco | 2,08× | 388 | 2.329 |
| zstd-9 | singolo blocco | 2,10× | 95 | 2.744 |
| gzip-6 | singolo blocco | 2,04× | 56 | 313 |

Le velocità di decompressione su segmenti da 4 MB sono una sola misura e variano molto
tra varianti (zstd-1 e zstd-19 non dovrebbero differire di 7×): vanno ripetute prima di
sceglierle. I rapporti invece non dipendono dalla macchina.

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
compressione; ~39 GB al giorno con zstd-3 a segmenti; ~35 GB al giorno con zstd-9.

## 2. Proposta di retention

Come il disegno già approvato, con la compressione nell'archivio esterno. Disattivata di
default (`journal_retention = false`). **Nulla si cancella senza una copia esterna
verificata per hash.**

**Sul nodo**
- Journal a segmenti: un file per intervallo tra due snapshot,
  `journal-<prima altezza>.bin`, **non compresso**: il nodo scrive e rilegge solo questi
  (riavvio, recupero dello stream, indexer vicino alla testa).
- Restano sul nodo: gli ultimi due snapshot e i segmenti da (penultimo snapshot −
  `blockhash_window`) in avanti.
- Per ogni segmento chiuso, un manifest: prima e ultima altezza, SHA-256 del segmento
  non compresso, `prev_hash` del primo blocco, hash dell'ultimo blocco, SHA-256 del file
  compresso.

**Archivio esterno**
- Segmento chiuso compresso con zstd (livello 3 come default; configurabile).
- Procedura, nell'ordine: compressione in un file temporaneo; copia nell'archivio
  (cartella o comando configurato); rilettura della copia; decompressione; verifica dello
  SHA-256 non compresso e della catena degli hash dal `prev_hash` del primo blocco
  all'hash dell'ultimo; solo allora, con `journal_retention = true`, cancellazione del
  segmento locale. Il manifest resta sul nodo.
- Formato: un frame zstd standard per segmento, leggibile con lo strumento `zstd` senza
  il codice del nodo.

**Dipendenza**: la compressione nel nodo richiede un crate zstd (con codice C) o lz4 (in
Rust puro: `lz4_flex`, 1,94× sui dati misurati). Proposta: nessuna dipendenza nel nodo.
La compressione e la copia esterna le fa un comando esterno configurato (per esempio
`zstd -3`, poi la copia), e il nodo verifica soltanto la copia: legge il file compresso
con lo stesso comando e controlla gli hash. Così il nodo resta senza dipendenze nuove e
la scelta dell'algoritmo resta fuori dal consenso.

**Test richiesti (dall'approvazione del disegno)**
1. Riavvio con il journal ridotto: snapshot e segmenti recenti danno lo stesso hash finale.
2. Verifica completa dall'archivio esterno: riesecuzione dal genesis leggendo i segmenti
   archiviati (decompressi) e confronto con l'hash del nodo.
3. Segmento modificato nell'archivio: rifiutato (SHA-256 o catena degli hash), e il
   segmento locale non si cancella.

**Righe stimate**: ~200 (segmenti ~80, manifest e verifica ~60, comando esterno e
cancellazione ~60).

**Rischi**
- Rapporto reale più basso di quello sintetico: lo spazio stimato va rivisto con il
  traffico di devnet.
- Comando esterno configurato male: mitigato dalla verifica prima di cancellare.
- Riesecuzione dal genesis e indexer molto indietro: leggono dall'archivio, più lenti
  (decompressione zstd-3 sopra i 300 MB/s per core).
