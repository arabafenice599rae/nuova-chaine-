#!/usr/bin/env bash
# Avvia un nodo devnet locale con un conto prefinanziato ed esegue uno script web3.js.
# Uso: scripts/web3-node.sh <script.mjs> [slot_ms] [argomenti extra per lo script]
# (richiede target/release/centrifuga e target/release/examples/keygen)
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
BIN="$ROOT/target/release"
WORK="$(mktemp -d)"
SLOT="${2:-100}"
trap 'kill "$NODE" 2>/dev/null || true; rm -rf "$WORK"' EXIT

"$BIN/examples/keygen" > "$WORK/operator.json" 2> "$WORK/op.txt"
"$BIN/examples/keygen" > "$WORK/user.json" 2> "$WORK/user.txt"
sed -e "s/SOSTITUIRE_CON_PUBKEY_OPERATORE/$(cut -d' ' -f2 "$WORK/op.txt")/" \
    -e 's/network = "mainnet"/network = "devnet"/' \
    -e 's/chain_id = "centrifuga:mainnet"/chain_id = "centrifuga:devnet"/' \
    -e "s/^slot_ms = .*/slot_ms = $SLOT/" \
    -e "s/^timestamp_ms = .*/timestamp_ms = $(( $(date +%s) * 1000 ))/" \
    -e "s|^data_dir = .*|data_dir = \"$WORK/data\"|" \
    -e "s|^operator_keypair = .*|operator_keypair = \"$WORK/operator.json\"|" \
    -e 's/^rpc_log = false/rpc_log = true/' \
    "$ROOT/centrifuga.example.toml" > "$WORK/centrifuga.toml"
printf '\n[[genesis.prefund]]\npubkey = "%s"\nlamports = 100000000000\n' "$(cut -d' ' -f2 "$WORK/user.txt")" >> "$WORK/centrifuga.toml"

"$BIN/centrifuga" run "$WORK/centrifuga.toml" > "$WORK/node.log" 2>&1 &
NODE=$!
# Pronto quando risponde e ha prodotto almeno un blocco.
for _ in $(seq 50); do
  curl -sf http://127.0.0.1:8899 -H 'content-type: application/json' \
    -d '{"jsonrpc":"2.0","id":1,"method":"getSlot"}' | grep -qv '"result":0' && break
  sleep 0.2
done
cd "$ROOT/tests/web3"
node "$1" http://127.0.0.1:8899 "$WORK/user.json" "${@:3}"
echo "--- metodi RPC ricevuti (rpc.log):"
cut -d' ' -f2- "$WORK/data/rpc.log" | sort | uniq -c | sort -rn
