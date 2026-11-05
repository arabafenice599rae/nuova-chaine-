#!/usr/bin/env bash
# Confronta lo SHA-256 pubblicato nel README con il .so committato e (se presente)
# con quello appena compilato da `cargo build-sbf` in programs/timelock.
set -euo pipefail
cd "$(dirname "$0")/.."
expected=$(grep -A3 'SHA-256 di `programs/timelock/centrifuga_timelock.so`' README.md | grep -oE '[0-9a-f]{64}' | head -1)
[ -n "$expected" ] || { echo "hash non trovato nel README"; exit 1; }
status=0
for f in programs/timelock/centrifuga_timelock.so programs/timelock/target/deploy/centrifuga_timelock.so; do
  [ -f "$f" ] || continue
  got=$(sha256sum "$f" | cut -d' ' -f1)
  if [ "$got" = "$expected" ]; then echo "OK      $f $got"; else echo "DIVERSO $f $got (README: $expected)"; status=1; fi
done
exit $status
