#!/usr/bin/env bash
# Assemble a self-contained Linux bundle: Flutter client + embedded Rust
# backend (pglite PostgreSQL) + launcher, into dist/stt-terminal-<ver>/.
#
# The launcher starts the backend, waits for health, runs the client, and
# shuts the backend down with SIGINT (the only graceful path for pglite).
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
VER="$(grep -m1 '^version:' "$ROOT/frontend/pubspec.yaml" | awk '{print $2}' | cut -d+ -f1)"
VER="${VER:-0.1.0}"

BUNDLE_SRC="$ROOT/frontend/build/linux/x64/release/bundle"
BE_BIN="$ROOT/backend/target/release/stt"
OUT="$ROOT/dist/stt-terminal-$VER"
TARBALL="$ROOT/dist/stt-terminal-$VER.tar.gz"

for p in "$BUNDLE_SRC/stt_desktop" "$BE_BIN"; do
  [[ -f "$p" ]] || { echo "missing $p — build it first"; exit 1; }
done

echo "==> assembling $OUT"
rm -rf "$OUT" "$TARBALL"
mkdir -p "$OUT/backend" "$OUT/var/pglite" "$OUT/logs"
cp -r "$BUNDLE_SRC"/. "$OUT/"
cp "$BE_BIN" "$OUT/backend/stt"
chmod +x "$OUT/backend/stt" "$OUT/stt_desktop"

cat > "$OUT/stt.toml" <<'TOML'
# STT Terminal — embedded backend configuration.
# Paper-only: no real trades, no keys. Demo mode generates the market
# feed locally and offline; set mode = "market-data" for a read-only
# DexScreener feed (still no real orders).
# All keys and section headers are kebab-case ([market-data], bind-address).
version = 1

[application]
mode = "demo"
bind-address = "127.0.0.1"
port = 8765
log-level = "info"

[database]
# Resolved relative to the bundle directory by run.sh (it cd's there).
data-directory = "./var/pglite"

[market-data]
provider = "dex-screener"
refresh-interval-ms = 3000
request-timeout-ms = 8000
maximum-data-age-ms = 15000
max-concurrent-requests = 4
backoff-base-ms = 500
max-retries = 3
demo-token-count = 8

[paper-trading]
initial-balance-lamports = 100_000_000_000
fee-bps = 25
slippage-bps = 10
slippage-model = "fixed-bps"
max-order-notional-usd = 25000

[risk]
max-position-fraction = 0.25
max-total-exposure = 0.8
max-daily-loss = 0.10
max-drawdown = 0.25
max-open-positions = 20
minimum-liquidity-usd = 25000
minimum-volume-24h-usd = 50000
max-slippage-bps = 150
max-price-age-ms = 30000
per-token-cooldown-ms = 60000
max-liquidity-impact = 0.05
stop-loss = 0.15
take-profit = 0.50
TOML

cat > "$OUT/run.sh" <<'SH'
#!/usr/bin/env bash
# STT Terminal launcher.
#   ./run.sh            start everything (backend + client)
#   STT_BIND=0.0.0.0 ./run.sh   bind remote (backend logs the demo boundary)
set -u

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$ROOT"                      # relative config/data paths resolve here

# Load server-side secrets from a gitignored .env beside the bundle (if the
# operator placed one here). Never committed; never echoed. This populates
# the same STT_REAL_* env the backend reads, so RPC/Jupiter/wallet keys are
# resolved strictly server-side.
if [[ -f "$ROOT/.env" ]]; then
  set -a; source "$ROOT/.env"; set +a
fi

# Point the backend at the bundle config (env var; no auto-discovery).
export STT_CONFIG="$ROOT/stt.toml"

BIND="${STT_BIND:-127.0.0.1}"
PORT="${STT_PORT:-8765}"
LOG="logs/backend.log"
mkdir -p var/pglite logs

BACK_PID=""
cleanup() {
  if [[ -n "$BACK_PID" ]] && kill -0 "$BACK_PID" 2>/dev/null; then
    echo "==> stopping embedded backend (SIGINT, graceful)..."
    kill -INT "$BACK_PID"
    for _ in $(seq 1 40); do kill -0 "$BACK_PID" 2>/dev/null || break; sleep 0.25; done
    kill -9 "$BACK_PID" 2>/dev/null || true
  fi
}
trap cleanup EXIT INT TERM

port_in_use() {
  (exec 3<>"/dev/tcp/$BIND/$PORT") 2>/dev/null && { exec 3>&- 3<&-; return 0; } || return 1
}

if port_in_use; then
  echo "==> $BIND:$PORT already listening — reusing existing backend"
else
  echo "==> starting embedded backend (data: var/pglite, log: $LOG)"
  # pglite-rs quirk: stdout as a plain-file redirect can make it exit 100
  # at boot; a pipe (process substitution) is reliable AND keeps a log.
  # setsid detaches it from the terminal; $! IS the backend pid (setsid execs stt).
  setsid ./backend/stt > >(tee -a "$LOG") 2>&1 < /dev/null &
  BACK_PID=$!
  ok=0
  for _ in $(seq 1 60); do
    if port_in_use; then ok=1; break; fi
    kill -0 "$BACK_PID" 2>/dev/null || { echo "!! backend exited early; see $LOG"; exit 1; }
    sleep 0.5
  done
  [[ "$ok" == 1 ]] || { echo "!! backend not reachable on $BIND:$PORT; see $LOG"; exit 1; }
fi

echo "==> launching terminal client (Ctrl+C quits both)"
exec ./stt_desktop
SH
chmod +x "$OUT/run.sh"

cat > "$OUT/README.md" <<MD
# STT Terminal — self-contained Linux bundle

Solana trading **research** terminal with paper trading only:

- No real trades, no wallet keys, no live-execution path (simulation boundary).
- Embedded backend: Rust + pglite (real PostgreSQL engine, local file storage).

## Run

    ./run.sh

The launcher starts the embedded API (127.0.0.1:8765), waits for health,
opens the desktop client, and shuts the backend down gracefully when the
client exits.

## Controls

- **DASHBOARD / MARKETS / STRATEGIES / PORTFOLIO / BACKTESTS / SETTINGS**
- Start strategies from STRATEGIES, run backtests from BACKTESTS.
- Strategy trades are paper fills in the embedded database; the ledger is
  the source of truth.

## Reset demo state

    rm -rf var/pglite        # deletes the embedded database (demo data only)

## Live market feed (optional)

Edit \`stt.toml\` → \`mode = "market-data"\` for a read-only DexScreener feed.
Trading stays paper. No network → close the app as-is (demo mode).

## Layout

    backend/stt     embedded API server (Rust)
    var/pglite      PostgreSQL data directory (runtime)
    logs/           backend log
    data/ lib/      Flutter desktop client runtime
MD

echo "==> packaging $TARBALL"
# Defense-in-depth: never let a .env or secret file ship inside the tarball.
find "$OUT" -maxdepth 2 -name '.env' -o -maxdepth 2 -name '*.pem' | while read -r f; do
  rm -f "$f"
  echo "  (removed secret-adjacent file from bundle: $f)"
done
tar -czf "$TARBALL" -C "$ROOT/dist" "stt-terminal-$VER"
echo "==> done:"
du -sh "$OUT" "$TARBALL"