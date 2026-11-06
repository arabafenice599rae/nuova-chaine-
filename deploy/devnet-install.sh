#!/usr/bin/env bash
# Nodo centrifuga devnet su un VPS Linux pulito (Debian 12, Ubuntu 22.04 o 24.04):
# compila, genera le chiavi sul server, installa il servizio systemd, attiva HTTPS
# (Caddy + Let's Encrypt) e avvia il genesis devnet con i conti prefinanziati.
#
#   sudo deploy/devnet-install.sh --domain rpc.example.org --email tu@example.org \
#        --prefund <pubkey>[:CnFA] [--prefund ...] [--reset]
#
# Rieseguibile: ricompila e riavvia, non tocca chiavi, config e dati esistenti.
# --reset cancella config e dati della chain (non le chiavi) e rifà il genesis.
# Le chiavi private restano in /var/lib/centrifuga (permessi 600) e non vengono
# mai stampate. CENTRIFUGA_SYSTEM=0 salta pacchetti, utente, systemd e Caddy (prove).
# --tls-internal (SOLO per i test senza dominio pubblico): certificato della CA
# locale di Caddy al posto di Let's Encrypt.
set -euo pipefail
umask 077

die() { echo "errore: $*" >&2; exit 1; }
log() { echo "==> $*"; }

DOMAIN="" EMAIL="" RESET=0 TLS_INTERNAL=0 PREFUND=()
while [ $# -gt 0 ]; do
  case "$1" in
    --domain) DOMAIN="$2"; shift 2 ;;
    --email) EMAIL="$2"; shift 2 ;;
    --prefund) PREFUND+=("$2"); shift 2 ;;
    --reset) RESET=1; shift ;;
    --tls-internal) TLS_INTERNAL=1; shift ;;
    *) die "opzione sconosciuta: $1" ;;
  esac
done
SYSTEM="${CENTRIFUGA_SYSTEM:-1}"
[ "$(id -u)" -eq 0 ] || die "eseguire come root (sudo)"
if [ "$SYSTEM" = 1 ]; then
  [ -n "$DOMAIN" ] && [ -n "$EMAIL" ] || die "servono --domain e --email"
fi

SRC="$(cd "$(dirname "$0")/.." && pwd)"
ETC="${CENTRIFUGA_ETC:-/etc/centrifuga}"
VAR="${CENTRIFUGA_VAR:-/var/lib/centrifuga}"
BIN="${CENTRIFUGA_BIN:-/usr/local/bin}"
CFG="$ETC/centrifuga.toml"
OWNER="centrifuga"; [ "$SYSTEM" = 1 ] || OWNER="$(id -un)"

if [ "$SYSTEM" = 1 ]; then
  log "pacchetti di sistema"
  export DEBIAN_FRONTEND=noninteractive
  apt-get update -q
  apt-get install -y -q build-essential pkg-config git curl ca-certificates gnupg \
    debian-keyring debian-archive-keyring apt-transport-https
  # La compilazione delle dipendenze Solana richiede ~4 GB: swap se la RAM è meno.
  mem_kb=$(awk '/MemTotal/ {print $2}' /proc/meminfo)
  if [ "$mem_kb" -lt 3500000 ] && [ "$(swapon --noheadings | wc -l)" -eq 0 ]; then
    log "RAM < 3,5 GB: swap da 4 GB in /swapfile"
    fallocate -l 4G /swapfile && chmod 600 /swapfile && mkswap /swapfile && swapon /swapfile
    grep -q '^/swapfile ' /etc/fstab || echo '/swapfile none swap sw 0 0' >> /etc/fstab
  fi
  if ! command -v caddy > /dev/null; then
    log "Caddy (repository ufficiale)"
    curl -1sLf https://dl.cloudsmith.io/public/caddy/stable/gpg.key | gpg --dearmor --yes -o /usr/share/keyrings/caddy-stable-archive-keyring.gpg
    curl -1sLf https://dl.cloudsmith.io/public/caddy/stable/debian.deb.txt > /etc/apt/sources.list.d/caddy-stable.list
    chmod 644 /usr/share/keyrings/caddy-stable-archive-keyring.gpg /etc/apt/sources.list.d/caddy-stable.list
    apt-get update -q && apt-get install -y -q caddy
  fi
  id centrifuga > /dev/null 2>&1 || useradd --system --home-dir "$VAR" --shell /usr/sbin/nologin centrifuga
fi

log "Rust (versione fissata da rust-toolchain.toml) e compilazione"
if ! command -v cargo > /dev/null; then
  [ -x "$HOME/.cargo/bin/cargo" ] || curl -sSf https://sh.rustup.rs | sh -s -- -y --profile minimal --default-toolchain none
  export PATH="$HOME/.cargo/bin:$PATH"
fi
rustup toolchain install "$(sed -n 's/^channel = "\(.*\)"/\1/p' "$SRC/rust-toolchain.toml")" --profile minimal
(cd "$SRC" && cargo build --release --locked --bins --examples)
install -m 755 "$SRC/target/release/centrifuga" "$BIN/centrifuga"
KEYGEN="$SRC/target/release/examples/keygen"

if [ "$RESET" = 1 ]; then
  log "--reset: cancello config e dati della chain (le chiavi restano)"
  [ "$SYSTEM" = 1 ] && systemctl stop centrifuga centrifuga-indexer 2> /dev/null || true
  rm -rf "$VAR/data" "$VAR/indexer" "$CFG"
fi

log "chiavi (generate qui, mai stampate)"
install -d -m 750 -o "$OWNER" "$VAR" && install -d -m 755 "$ETC"
for k in operator faucet; do
  if [ ! -f "$VAR/$k.json" ]; then
    "$KEYGEN" > "$VAR/$k.json" 2> /dev/null
  fi
  chown "$OWNER" "$VAR/$k.json" && chmod 600 "$VAR/$k.json"
done
OPERATOR=$("$KEYGEN" "$VAR/operator.json")
FAUCET=$("$KEYGEN" "$VAR/faucet.json")

if [ -f "$CFG" ]; then
  [ ${#PREFUND[@]} -eq 0 ] || echo "attenzione: genesis già esistente, --prefund ignorati (usare --reset per rifarlo)"
else
  log "config devnet e prefund"
  now_ms=$(( $(date +%s) * 1000 ))
  sed -e 's/^network = .*/network = "devnet"/' \
      -e 's/^chain_id = .*/chain_id = "centrifuga:devnet"/' \
      -e "s/^operator = .*/operator = \"$OPERATOR\"/" \
      -e "s/^timestamp_ms = .*/timestamp_ms = $now_ms/" \
      -e "s|^data_dir = .*|data_dir = \"$VAR/data\"|" \
      -e "s|^operator_keypair = .*|operator_keypair = \"$VAR/operator.json\"|" \
      -e 's/^rpc_addr = .*/rpc_addr = "127.0.0.1:8899"/' \
      -e 's/^stream_addr = .*/stream_addr = "127.0.0.1:8901"/' \
      -e 's/^rpc_log = .*/rpc_log = true/' \
      "$SRC/centrifuga.example.toml" > "$CFG.tmp"
  printf '\n# Conto per lo script del token SPL di prova (chiave: %s/faucet.json).\n[[genesis.prefund]]\npubkey = "%s"\nlamports = 10000000000\n' "$VAR" "$FAUCET" >> "$CFG.tmp"
  for p in "${PREFUND[@]}"; do
    key="${p%%:*}"; amount="${p#*:}"; [ "$amount" = "$p" ] && amount=100
    [[ "$amount" =~ ^[0-9]+$ ]] || die "--prefund $p: importo in CnFA interi"
    printf '\n[[genesis.prefund]]\npubkey = "%s"\nlamports = %s000000000\n' "$key" "$amount" >> "$CFG.tmp"
  done
  mv "$CFG.tmp" "$CFG" && chmod 644 "$CFG"
fi

# Storico: indexer separato sulla stessa macchina (anche per config già esistenti).
if ! grep -q '^\[indexer\]' "$CFG"; then
  log "indexer dello storico"
  sed -i -e '/^# indexer_rpc = /d' -e '/^\[node\]/a indexer_rpc = "http://127.0.0.1:8903"' "$CFG"
  printf '\n[indexer]\ndata_dir = "%s/indexer"\nrpc_addr = "127.0.0.1:8903"\n' "$VAR" >> "$CFG"
fi

log "verifica del genesis (conservazione e determinismo)"
"$BIN/centrifuga" genesis "$CFG"

if [ "$SYSTEM" = 1 ]; then
  log "servizio systemd"
  cat > /etc/systemd/system/centrifuga.service <<UNIT
[Unit]
Description=centrifuga devnet
After=network-online.target
Wants=network-online.target

[Service]
User=centrifuga
ExecStart=$BIN/centrifuga run $CFG
Restart=always
RestartSec=2
NoNewPrivileges=true
ProtectSystem=strict
ProtectHome=true
PrivateTmp=true
ReadWritePaths=$VAR
LimitNOFILE=65536

[Install]
WantedBy=multi-user.target
UNIT
  chmod 644 /etc/systemd/system/centrifuga.service
  # Indexer: processo separato, legge il journal del nodo; il nodo non dipende da lui.
  sed -e 's/^Description=.*/Description=centrifuga devnet: indexer dello storico/' \
      -e "s|^ExecStart=.*|ExecStart=$BIN/centrifuga indexer $CFG|" \
      /etc/systemd/system/centrifuga.service > /etc/systemd/system/centrifuga-indexer.service
  chmod 644 /etc/systemd/system/centrifuga-indexer.service
  systemctl daemon-reload
  systemctl enable centrifuga centrifuga-indexer > /dev/null
  systemctl restart centrifuga
  systemctl restart centrifuga-indexer

  log "HTTPS con Caddy e Let's Encrypt"
  TLS_LINE="	# certificato Let's Encrypt automatico"
  if [ "$TLS_INTERNAL" = 1 ]; then TLS_LINE="	tls internal # SOLO TEST: CA locale di Caddy"; fi
  cat > /etc/caddy/Caddyfile <<CADDY
{
	email $EMAIL
}

$DOMAIN {
${TLS_LINE}
	# Stream per bot e repliche: wss://$DOMAIN/stream?from_height=...
	handle /stream* {
		reverse_proxy 127.0.0.1:8901
	}
	# WebSocket JSON-RPC (accountSubscribe, signatureSubscribe) sulla stessa porta.
	# Senza distinzione di maiuscole: alcuni client (es. WebSocket di Node) mandano
	# "connection: upgrade" e "upgrade: websocket" in minuscolo.
	@ws header_regexp Upgrade (?i)websocket
	handle @ws {
		reverse_proxy 127.0.0.1:8900
	}
	handle {
		reverse_proxy 127.0.0.1:8899
	}
}
CADDY
  chmod 644 /etc/caddy/Caddyfile
  if command -v ufw > /dev/null && ufw status | grep -q "Status: active"; then
    ufw allow 80/tcp > /dev/null && ufw allow 443/tcp > /dev/null
  fi
  caddy validate --config /etc/caddy/Caddyfile --adapter caddyfile > /dev/null || die "Caddyfile non valido"
  systemctl enable caddy > /dev/null
  systemctl restart caddy

  log "controllo"
  for _ in $(seq 30); do curl -sf http://127.0.0.1:8899/health > /dev/null && break; sleep 1; done
  curl -sf http://127.0.0.1:8899/health > /dev/null || die "il nodo non risponde: journalctl -u centrifuga"
  ok=0 CA=()
  if [ "$TLS_INTERNAL" = 1 ]; then CA=(--cacert /var/lib/caddy/.local/share/caddy/pki/authorities/local/root.crt); fi
  for _ in $(seq 60); do curl -sf "${CA[@]}" "https://$DOMAIN/health" > /dev/null 2>&1 && { ok=1; break; }; sleep 2; done
  [ "$ok" = 1 ] || echo "attenzione: https://$DOMAIN non risponde ancora (DNS? porte 80/443?): journalctl -u caddy"
fi

cat <<FINE

Nodo devnet pronto.
  RPC:             https://${DOMAIN:-<dominio>}
  WebSocket:       wss://${DOMAIN:-<dominio>}
  Stream:          wss://${DOMAIN:-<dominio>}/stream
  Operatore:       $OPERATOR
  Faucet:          $FAUCET (chiave in $VAR/faucet.json)
  Config:          $CFG
  Log del nodo:    journalctl -u centrifuga -f
  Metodi RPC:      cut -d' ' -f2- $VAR/data/rpc.log | sort | uniq -c | sort -rn
FINE
