# Processo di release

Da seguire a ogni release del binario `centrifuga`. Per il genesis definitivo di
mainnet c'è una sezione apposta, da eseguire una sola volta.

## Ogni release

1. **Certificati radice**: aggiornare `webpki-roots` all'ultima versione, così le CA
   pubbliche usate dal wallet (HTTPS) non invecchiano. La versione è fissata esatta in
   `Cargo.toml` (`webpki-roots = "=X.Y.Z"`, come `rustls`): si cambia lì, poi
   ```sh
   cargo update -p webpki-roots
   git diff Cargo.toml Cargo.lock   # solo webpki-roots
   ```
2. **Avvisi di terze parti**: rigenerare `THIRD_PARTY_NOTICES` (la CI fallisce se il
   file non corrisponde a `Cargo.lock`):
   ```sh
   scripts/third-party-notices.py > THIRD_PARTY_NOTICES
   ```
3. **Verifiche**: `cargo build --release --locked` e `cargo test --release --locked`.
   La CI (build, test, web3.js, hash del timelock, deploy dietro Caddy con il
   wallet) deve essere verde sul commit della release.
4. **Timelock**: lo SHA-256 di `programs/timelock/centrifuga_timelock.so` deve
   coincidere con il README (lo controlla la CI). Il programma è nel genesis: dopo il
   lancio di mainnet non cambia più.
5. Tag della release firmato (`git tag -s vX.Y.Z`), poi push del tag.

## Genesis definitivo di mainnet (una sola volta)

Oggi non esiste: nessun valore di mainnet va scritto prima di questo momento.

1. Preparare `genesis/mainnet.toml` dalla `centrifuga.example.toml`: pubkey reale
   dell'operatore (mai la chiave privata) e `timestamp_ms` del lancio, senza
   `prefund`. Committarlo.
2. Dal commit con quel file, su una macchina pulita:
   ```sh
   cargo build --release --locked
   sha256sum programs/timelock/centrifuga_timelock.so
   target/release/centrifuga genesis genesis/mainnet.toml   # "genesis hash: ..."
   git rev-parse HEAD
   ```
   Ripetere su una seconda macchina: i valori devono coincidere.
3. Pubblicare **gli stessi quattro valori** (`chain_id`, `genesis_hash`, SHA-256 del
   timelock, commit) in tre posti:
   - **tag Git firmato** `mainnet-genesis` sul commit del punto 2, con i valori nel
     messaggio:
     ```sh
     git tag -s mainnet-genesis -m "chain_id: centrifuga:mainnet
     genesis_hash: <...>
     timelock_sha256: <...>
     commit: <...>"
     git push origin mainnet-genesis
     ```
   - **README**: sezione "Identità di mainnet" con i quattro valori e l'impronta
     della chiave che firma il tag. È un commit successivo, che non cambia
     `genesis/mainnet.toml` né il codice del genesis.
   - **sito**: la stessa tabella e la stessa impronta.
4. File e dati necessari al ricalcolo: solo `genesis/mainnet.toml`, committato nel
   commit del tag. Se un giorno il ricalcolo richiederà altri file (per esempio una
   lista di account iniziali), vanno committati nello stesso commit, o allegati al
   tag con il loro SHA-256 nel messaggio del tag.
5. Verificare da un clone nuovo la procedura di [wallet.md](wallet.md) ("Verificare
   che i tre valori coincidano" e "Ricalcolarli").
