#!/usr/bin/env bash
# Verifiche del deploy devnet in CI (runner con systemd), dopo devnet-install.sh --tls-internal.
# Uso: sudo deploy/ci-check.sh <dominio> <bench.json> <cartella artifact>
# Ogni controllo scrive una riga "OK|FALLITO <nome> <dettagli>" in $OUT/rapporto.txt.
set -uo pipefail
DOMAIN="$1" BENCH="$2" OUT="$3"
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
VAR=/var/lib/centrifuga
CA=/var/lib/caddy/.local/share/caddy/pki/authorities/local/root.crt
export NODE_EXTRA_CA_CERTS="$CA"
mkdir -p "$OUT"
REPORT="$OUT/rapporto.txt"; : > "$REPORT"
FAIL=0
check() { # check <nome> <comando...>
  local name="$1"; shift
  local out; out=$("$@" 2>&1); local rc=$?
  if [ $rc -eq 0 ]; then echo "OK      $name ${out:0:300}" | tr '\n' ' ' >> "$REPORT"; else echo "FALLITO $name ${out:0:600}" | tr '\n' ' ' >> "$REPORT"; FAIL=1; fi
  echo >> "$REPORT"
}
rpc() { curl -sf --cacert "$CA" "https://$DOMAIN" -H 'content-type: application/json' -d "{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"$1\",\"params\":${2:-[]}}"; }
height() { rpc getSlot | python3 -c 'import json,sys; print(json.load(sys.stdin)["result"])'; }
wait_health() { for _ in $(seq 60); do curl -sf --cacert "$CA" "https://$DOMAIN/health" > /dev/null && return 0; sleep 1; done; return 1; }
keyhash() { sha256sum "$VAR/operator.json" "$VAR/faucet.json" | cut -d' ' -f1 | tr '\n' ' '; }
perms() { for k in operator faucet; do s=$(stat -c '%a %U' "$VAR/$k.json"); [ "$s" = "600 centrifuga" ] || { echo "$k: $s"; return 1; }; echo -n "$k: $s; "; done; }

check "servizi attivi" bash -c 'systemctl is-active centrifuga && systemctl is-active caddy'
check "HTTPS /health" bash -c "curl -sf --cacert $CA https://$DOMAIN/health"
check "HTTPS getVersion" rpc getVersion
check "permessi chiavi" perms
check "web3.js (HTTPS + WebSocket)" bash -o pipefail -c "cd $ROOT/tests/web3 && timeout 180 node rpc.mjs https://$DOMAIN $BENCH | tail -3"
check "stream /stream" timeout 60 node "$ROOT/tests/web3/stream-check.mjs" "wss://$DOMAIN/stream"

H1=$(height); K1=$(keyhash); G1=$(rpc getGenesisHash)
check "riavvio del servizio" bash -c "systemctl restart centrifuga && sleep 1 && for i in \$(seq 30); do curl -sf --cacert $CA https://$DOMAIN/health >/dev/null && exit 0; sleep 1; done; exit 1"
H2=$(height)
check "stato conservato dopo il riavvio (altezza $H1 -> $H2)" test "$H2" -ge "$H1"

check "riesecuzione dello script" bash -c "$ROOT/deploy/devnet-install.sh --domain $DOMAIN --email ci@example.invalid --tls-internal > $OUT/install-rerun.log 2>&1"
wait_health
check "chiavi invariate dopo la riesecuzione" test "$(keyhash)" = "$K1"
check "genesis invariato dopo la riesecuzione" test "$(rpc getGenesisHash)" = "$G1"

BENCHPK=$(cd "$ROOT/tests/web3" && timeout 30 node -e "const w=require('@solana/web3.js');console.log(w.Keypair.fromSecretKey(Uint8Array.from(require(process.argv[1]))).publicKey.toBase58())" "$BENCH")
check "--reset" bash -c "$ROOT/deploy/devnet-install.sh --domain $DOMAIN --email ci@example.invalid --tls-internal --reset --prefund $BENCHPK:50 > $OUT/install-reset.log 2>&1"
wait_health
check "chiavi invariate dopo --reset" test "$(keyhash)" = "$K1"
check "nuovo genesis dopo --reset" test "$(rpc getGenesisHash)" != "$G1"
check "permessi chiavi dopo --reset" perms
check "web3.js dopo --reset" bash -o pipefail -c "cd $ROOT/tests/web3 && timeout 180 node rpc.mjs https://$DOMAIN $BENCH | tail -1"

# Le chiavi private non devono comparire in nessun log.
journalctl -u centrifuga -u caddy --no-pager > "$OUT/journal.log" 2>&1
cp "$VAR/data/rpc.log" "$OUT/rpc.log" 2>/dev/null || true
cp /etc/caddy/Caddyfile "$OUT/Caddyfile"
cp /etc/centrifuga/centrifuga.toml "$OUT/centrifuga.toml"
nokeys() {
  for k in operator faucet; do
    secret=$(tr -d ' \n' < "$VAR/$k.json")
    for f in "$OUT"/*.log "$OUT"/centrifuga.toml; do
      if grep -qF "${secret:1:60}" "$f"; then echo "chiave $k trovata in $(basename "$f")"; return 1; fi
    done
  done
  echo "nessuna chiave nei log ($(ls "$OUT" | tr '\n' ' '))"
}
check "chiavi assenti dai log" nokeys

cat "$REPORT"
exit $FAIL
