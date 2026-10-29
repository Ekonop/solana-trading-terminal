#!/usr/bin/env bash
# Build and run the backend (paper-only terminal).
set -euo pipefail

cd "$(dirname "$0")/.."

CONFIG="${STT_CONFIG:-stt.toml}"
if [[ ! -f "$CONFIG" ]]; then
  echo "==> no $CONFIG found; creating from example"
  cp stt.toml.example stt.toml
fi

echo "==> building backend (first build compiles embedded postgres; slow)"
cargo build --manifest-path backend/Cargo.toml --bin stt

echo "==> starting terminal (demo mode) — press Ctrl+C to stop"
# pglite-rs boot is flaky when stdout is a plain file redirect (silent exit 100);
# keep stdout as a tty when possible, else a pipe to tee (reliable + logged).
if [[ -t 1 ]]; then
  exec env STT_CONFIG="$CONFIG" cargo run --manifest-path backend/Cargo.toml --bin stt
else
  exec env STT_CONFIG="$CONFIG" cargo run --manifest-path backend/Cargo.toml --bin stt 2>&1 | tee -a backend.run.log
fi