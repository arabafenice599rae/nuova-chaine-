#!/usr/bin/env bash
# Soak con risultati salvati riga per riga: docs/soak/<nome>.md.
# Uso: scripts/soak-run.sh <nome> <account> <tx/s> <minuti> [--rpc]
#   (richiede target/release/examples/soak e target/release/centrifuga)
# Il file dei risultati si scrive mentre il test gira (fsync a ogni riga): un riavvio
# del container non cancella i dati già raccolti. Se il file esiste senza la riga
# finale (run interrotto), lo script NON riparte da capo: stampa i dati parziali ed
# esce con codice 3; per ripetere il run va cancellato a mano.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
NAME="$1"; shift
OUT="$ROOT/docs/soak/$NAME.md"
if [ -f "$OUT" ]; then
  if grep -q '^FINE' "$OUT"; then echo "già completato: $OUT"; exit 0; fi
  echo "run interrotto, dati parziali in $OUT:"; cat "$OUT"; exit 3
fi
mkdir -p "$ROOT/docs/soak"
DIR="${LOAD_DIR:-/tmp/centrifuga-soak}"
rm -rf "$DIR"
export RESULTS="$OUT" LOAD_DIR="$DIR" GIT_COMMIT="$(git -C "$ROOT" rev-parse --short HEAD)"
{
  echo "# soak $NAME"
  echo
  echo "comando: \`scripts/soak-run.sh $NAME $*\` (commit $GIT_COMMIT)"
  echo
} > "$OUT"
# Su stdout il nodo stampa una riga per blocco: va in un log a parte, i risultati nel file.
"$ROOT/target/release/examples/soak" "$@" > "$DIR.log" 2>&1
"$ROOT/target/release/examples/soak" --reopen "$DIR" >> "$DIR.log" 2>&1
echo "FINE" >> "$OUT"
sync
