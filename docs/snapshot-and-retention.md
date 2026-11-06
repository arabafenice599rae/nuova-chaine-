# Snapshot senza pause e retention del journal (proposte, nessun codice)

## Snapshot senza fermare i blocchi

**Oggi** lo snapshot si scrive sul thread del nodo, dentro il blocco che lo innesca:
circa 1 s a 1 milione di account e 7,4 s a 10 milioni, con l'hash completo.

### A. Copia in background (consigliata)

**Come funziona**
1. All'altezza dello snapshot il thread del nodo copia la mappa degli account. I dati
   degli account sono condivisi con `Arc`, quindi si copiano solo le voci della mappa.
2. Nello stesso momento si salva `status.bin` (altezza dello snapshot). Si salvano
   insieme anche altezza, hash di stato e genesis hash.
3. Un thread separato ordina, scrive `snapshot.tmp`, fa fsync e rinomina.

Se uno snapshot è ancora in scrittura, il successivo si salta. Un account modificato
durante la scrittura viene copiato dal nodo (`Arc::make_mut`), che poi lavora sulla
propria copia: lo snapshot resta coerente all'altezza della copia.

**Righe**: ~40–60.

**Pausa stimata**: solo la copia della mappa, ~0,1 s a 1 milione di account e ~1 s a
10 milioni. È da misurare prima di confermare.

**Rischi**
- RAM in più durante la scrittura: ~90 MB a 1 milione di account, ~1 GB a 10 milioni.
- Copia dei dati per gli account modificati mentre lo snapshot si scrive.
- Arresto a metà: resta valido lo snapshot precedente, perché il rename è atomico e
  `status.bin` porta l'altezza.
- La pausa resta proporzionale al numero di account, solo molto più piccola.

### B. Snapshot incrementale

**Come funziona**: a ogni intervallo si scrivono solo gli account cambiati da quello
precedente (un file delta); uno snapshot completo raro si fa in background.

**Righe**: ~120–150.

**Rischi**
- Più file da tenere coerenti.
- Serve una compattazione dei delta.
- Il riavvio applica base + delta + journal.
- Un delta perso rende inutilizzabili quelli successivi, fino al prossimo completo.

**Consiglio**: A subito; B solo se la copia di A risulta troppo lunga a 10 milioni di
account.

## Retention del journal

**Obiettivo**: il nodo riparte da snapshot e blocchi recenti; lo storico completo sta
altrove, con hash verificabili. **Nulla si cancella finché la procedura non è
approvata.**

**Struttura**
- **Journal a segmenti**: un file per intervallo (`journal-<prima altezza>.bin`), che si
  chiude a ogni snapshot.
- **Sul nodo restano**: gli ultimi due snapshot e i segmenti da (penultimo snapshot −
  `blockhash_window`) in avanti. Bastano per il riavvio, la finestra dei blockhash e
  l'anti-duplicati.
- **Manifest per ogni segmento chiuso**: prima e ultima altezza, SHA-256 del file,
  `prev_hash` del primo blocco, hash dell'ultimo blocco.
- **Verificabilità**: ogni header è firmato dall'operatore e concatenato al precedente.
  Partendo dall'hash di un blocco recente fidato si verifica all'indietro qualsiasi
  segmento archiviato, oltre allo SHA-256 del manifest.

**Procedura**
1. Il segmento chiuso viene copiato nell'archivio esterno, con un comando o una
   destinazione configurati (cartella, object storage).
2. La copia viene riletta e verificata: SHA-256 e catena degli hash dal `prev_hash` del
   primo blocco all'hash dell'ultimo.
3. Solo dopo la verifica riuscita, e solo con `journal_retention = true` (default:
   `false`), il segmento locale si cancella.
4. Il manifest resta sul nodo, così il nodo può sempre dire dove si trova ogni
   intervallo e con quale hash.

**Conseguenze**
- La riesecuzione dal genesis, il recupero lungo dello stream e un indexer molto
  indietro leggono i segmenti dall'archivio esterno.
- L'indexer non deve restare indietro rispetto al punto di retention, oppure legge
  dall'archivio.
- Una replica nuova parte da uno snapshot firmato più i segmenti recenti, oppure
  riesegue tutto dall'archivio.

**Righe**: ~180 (segmenti ~80, manifest e verifica ~60, copia esterna ~40).

**Rischi**
- Si perde la riesecuzione locale dal genesis.
- Una copia esterna configurata male; mitigazione: verifica prima di cancellare e due
  snapshot locali.
- Recupero dello stream e indexer vanno instradati sull'archivio.
- Il riavvio dipende dallo snapshot più recente; mitigazione: se è corrotto si usa il
  penultimo.
