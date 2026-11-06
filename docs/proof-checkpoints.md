# Checkpoint delle prove (specifica, nessun codice)

Stato: **proposta**. Non tocca il consenso: blocchi, header, journal e genesis restano
come sono. L'hash di stato del consenso è LtHash (`lthash-blake3-v1`). Il checkpoint è
un'attestazione in più, firmata dall'operatore, che permette di provare il saldo di un
account (o la sua assenza) senza rieseguire la chain.

## 1. Contenuto

Ogni secondo l'operatore firma, con la stessa chiave dei blocchi ma con un dominio
separato:

```
"centrifuga-checkpoint" ‖ genesis_hash (32) ‖ versione (u8 = 1) ‖ altezza (u64 LE) ‖
hash del blocco (32) ‖ radice sparsa (32)
```

**Quale blocco**: un checkpoint per il primo blocco il cui timestamp entra in un nuovo
secondo, cioè `floor(ts/1000)` maggiore di quello del checkpoint precedente. Ne esce al
più uno al secondo, e la scelta è deterministica a partire dalla chain.

**Radice sparsa**: albero di Merkle sparso a 256 livelli, calcolato sullo stato **dopo**
quel blocco.
- La posizione di un account è `SHA-256(pubkey)`.
- La foglia è `SHA-256(0x00 ‖ pubkey ‖ lamports ‖ owner ‖ executable ‖ lunghezza ‖ dati)`.
  `rent_epoch` è escluso; un account con 0 lamport vale zero, cioè assente.
- I nodi interni sono quelli del crate scelto, con i prefissi di dominio del crate
  stesso.

**Dove si calcola**: su un thread separato, che riceve una volta al secondo gli account
cambiati. Le copie costano poco, perché i dati sono condivisi con `Arc`. Il thread del
nodo non aspetta mai.

## 2. Esposizione

**RPC**
- `centrifugaGetCheckpoint([altezza?])` restituisce
  `{version, height, blockHash, root, signature}`: l'ultimo, oppure quello di
  un'altezza (finestra degli ultimi N in RAM, configurabile).
- `centrifugaGetAccountProof([pubkey, {height?}])` restituisce `{checkpoint, account}`,
  dove `account` è null se l'account è assente, più `proof` (prova compilata in
  base64, la stessa per appartenenza e assenza).

**Stream**: un messaggio `{"type":"checkpoint", ...}` subito dopo il blocco a cui si
riferisce, quando la radice è pronta (in media 0,3 s dopo, a 22.000 tx/s con 1 milione
di account).

**Wallet** (`centrifuga wallet balance --verify`):
1. legge checkpoint e prova;
2. verifica la firma con la pubkey dell'operatore, fissata in `wallet.toml` come
   `chain_id` e `genesis_hash` e pubblicata con i valori ufficiali;
3. verifica la prova contro la radice;
4. controlla che l'altezza non torni indietro rispetto all'ultimo checkpoint visto;
5. mostra "verificato all'altezza H, X s fa".

Un account assente si verifica con la prova di assenza.

## 3. Prove di frode

Tutte si verificano con le sole firme dell'operatore, tranne l'ultima, che richiede la
riesecuzione.

1. **Due checkpoint incoerenti fra loro**: stessa versione e stessa altezza, ma hash
   del blocco o radice diversi.
2. **Checkpoint incoerente con il journal**: un checkpoint e un header firmato alla
   stessa altezza con hash del blocco diverso.
3. **Radice falsa**: un checkpoint e la riesecuzione del journal fino a quell'altezza
   danno un'altra radice. La prova compatta è una prova di account firmata (dalla
   radice del checkpoint) che contraddice lo stato rieseguito.

## 4. Repliche

L'albero è **facoltativo**: con `proof_checkpoints = false` una replica o un nodo non
lo mantiene.
- Senza albero non pubblica né verifica checkpoint, ma verifica i blocchi come prima,
  con LtHash.
- Anche l'indexer può tenerlo, al posto del nodo, e servire le prove.

## 5. Stima

| voce | valore |
|---|---|
| righe | ~300–350: thread dell'albero e firma ~120, RPC ~60, stream ~20, verifica nel wallet ~60, strumento per le prove di frode ~50 |
| dipendenze | il crate dell'albero (vedi §6) |
| RAM | +498 MB a 1 milione di account (misurato); ~5 GB a 10 milioni (stima, da misurare) |
| CPU | 306 ms al secondo su un core a 22.000 tx/s con 1 milione di account (misurato); margine circa 3 volte il carico attuale |
| riavvio | +11 s a 1 milione di account per ricostruire l'albero (misurato); ~110 s a 10 milioni (stima). Salvare l'albero toglie questo tempo ma aggiunge circa 0,5 GB di snapshot a 1 milione di account |
| prova | 739 byte, verifica in 10–14 µs (misurato) |

## 6. Crate dell'albero e audit

`sparse-merkle-tree` 0.6.1 (Nervos, MIT):
- **Audit**: nessun audit pubblico trovato, né nel repository né nei rapporti pubblici
  su Nervos. Il rapporto Least Authority del 2019 riguarda la chain Nervos, non questo
  crate.
- **Debolezza documentata dagli autori** (`SMT.md`): l'ottimizzazione dei valori zero
  `merge(N, 0) == merge(0, N)` permetterebbe radici uguali da mappe diverse.
- **Cosa fa la 0.6.1**: non usa più quella fusione. Usa nodi `MergeWithZero`, che
  registrano i bit del percorso (`zero_bits`, `zero_count`), con prefissi
  `MERGE_NORMAL` e `MERGE_ZEROS` separati. In più la nostra foglia contiene la pubkey.
  La debolezza descritta non si applica a questa versione, ma è una mia lettura del
  codice, non un audit.

Alternative già usate in produzione:

| crate | dove si usa | audit |
|---|---|---|
| Jellyfish Merkle Tree di Grug (Left Curve) | Grug | Zellic ha verificato proprio lo schema JMT di Grug (dicembre 2024). È un albero a 256 bit con prove di appartenenza e assenza: da verificare come crate indipendente e le prestazioni |
| `jmt` (Penumbra, derivato dal JMT di Diem) | Penumbra, Sovereign SDK | non ho trovato un audit dedicato al JMT. Gli audit Zellic di Penumbra del 2024 coprono altre parti |

**Proposta**:
1. misurare il JMT di Grug sullo stesso benchmark, se è pubblicato come crate
   utilizzabile;
2. altrimenti restare su `sparse-merkle-tree` 0.6.1 con la versione fissata esatta,
   più test di proprietà contro un'implementazione di riferimento semplice (albero
   completo a 256 livelli su pochi account) e una revisione esterna prima del
   mainnet.

## 7. Rischi

- **Chiave**: la stessa chiave dell'operatore firma blocchi e checkpoint. Il dominio
  separato impedisce di riusare una firma da un contesto all'altro.
- **Freschezza**: la prova si riferisce a uno stato vecchio fino a circa 1,3 s (1 s di
  intervallo più il calcolo). Il wallet deve mostrarlo.
- **Abuso**: `centrifugaGetAccountProof` costa più di `getBalance` e va limitato per IP.
- **Versioni**: un cambio di albero o di formato della foglia richiede una nuova
  `versione`; i checkpoint vecchi restano verificabili con la versione con cui sono
  stati firmati.
- **Dipendenza**: crate senza audit (§6).
