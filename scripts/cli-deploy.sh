#!/usr/bin/env bash
# `solana program deploy` (CLI ufficiale Agave) contro un nodo devnet locale:
# deploy di echo v1, chiamata, aggiornamento a v2, chiamata, riavvio del nodo
# (riesecuzione del journal), chiamata. Fallisce se la CLI usa un metodo non
# supportato.
# Uso: SOLANA=/percorso/solana scripts/cli-deploy.sh
# (richiede target/release/centrifuga, target/release/examples/keygen, tests/web3 con npm ci)
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
BIN="$ROOT/target/release"
SOLANA="${SOLANA:-solana}"
WORK="$(mktemp -d)"
trap 'kill "$NODE" 2>/dev/null || true; rm -rf "$WORK"' EXIT

"$BIN/examples/keygen" > "$WORK/operator.json" 2> "$WORK/op.txt"
"$BIN/examples/keygen" > "$WORK/user.json" 2> "$WORK/user.txt"
"$BIN/examples/keygen" > "$WORK/program.json" 2> "$WORK/program.txt"
PROGRAM="$(cut -d' ' -f2 "$WORK/program.txt")"
sed -e "s/SOSTITUIRE_CON_PUBKEY_OPERATORE/$(cut -d' ' -f2 "$WORK/op.txt")/" \
    -e 's/network = "mainnet"/network = "devnet"/' \
    -e 's/chain_id = "centrifuga:mainnet"/chain_id = "centrifuga:devnet"/' \
    -e "s/^timestamp_ms = .*/timestamp_ms = $(( $(date +%s) * 1000 ))/" \
    -e "s|^data_dir = .*|data_dir = \"$WORK/data\"|" \
    -e "s|^operator_keypair = .*|operator_keypair = \"$WORK/operator.json\"|" \
    -e 's/^rpc_log = false/rpc_log = true/' \
    "$ROOT/centrifuga.example.toml" > "$WORK/centrifuga.toml"
printf '\n[[genesis.prefund]]\npubkey = "%s"\nlamports = 100000000000\n' "$(cut -d' ' -f2 "$WORK/user.txt")" >> "$WORK/centrifuga.toml"

start() {
  "$BIN/centrifuga" run "$WORK/centrifuga.toml" >> "$WORK/node.log" 2>&1 &
  NODE=$!
  for _ in $(seq 50); do
    curl -sf http://127.0.0.1:8899/health > /dev/null && return 0
    sleep 0.2
  done
  echo "nodo non partito"; cat "$WORK/node.log"; return 1
}
sol() { timeout 180 "$SOLANA" -u http://127.0.0.1:8899 -k "$WORK/user.json" "$@"; }
call() { # call <versione attesa>
  local got; got=$(cd "$ROOT/tests/web3" && timeout 60 node call-echo.mjs http://127.0.0.1:8899 "$WORK/user.json" "$PROGRAM")
  [ "$got" = "$1" ] || { echo "atteso '$1', ricevuto '$got'"; return 1; }
  echo "ok  chiamata: $got"
}

start
# --use-rpc: senza, la CLI invia le scritture via TPU/QUIC (non c'è: un solo validatore, niente QUIC).
sol program deploy "$ROOT/tests/programs/echo_v1.so" --program-id "$WORK/program.json" --use-rpc
call "echo v1"
sol program deploy "$ROOT/tests/programs/echo_v2.so" --program-id "$WORK/program.json" --use-rpc
call "echo v2"
sol program show "$PROGRAM"
kill "$NODE"; wait "$NODE" 2>/dev/null || true
start  # riapre dallo snapshot (se c'è) e riesegue il journal
call "echo v2"
echo "--- metodi RPC ricevuti (rpc.log):"
cut -d' ' -f2- "$WORK/data/rpc.log" | sort | uniq -c | sort -rn
! grep -q UNSUPPORTED "$WORK/data/rpc.log"
