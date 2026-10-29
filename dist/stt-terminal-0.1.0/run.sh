#!/usr/bin/env bash
# STT Terminal launcher.
#   ./run.sh            start everything (backend + client)
#   STT_BIND=0.0.0.0 ./run.sh   bind remote (backend logs the demo boundary)
set -u

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$ROOT"                      # relative config/data paths resolve here

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
