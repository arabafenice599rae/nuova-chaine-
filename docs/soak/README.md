# Risultati dei soak

Ogni file è l'output di `examples/soak.rs` (o di `scripts/soak-run.sh`) per un run: la
riga del comando in testa dice parametri, commit e sysctl.

**Invii**: dal 7/11/2026 il soak invia una tx alla volta con arrivi di Poisson (tempi
casuali, nessuna raffica). **I run precedenti usavano invii a raffiche** (lotti da 50 tx
per thread): con le raffiche circa un terzo delle tx arriva mentre il blocco è già chiuso e
salta allo slot successivo (33% contro il 10% atteso), quindi la loro latenza
invio→inclusione è più alta. `BURSTS=1` ripristina le raffiche per confronti con i run
vecchi.
