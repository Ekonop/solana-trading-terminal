#!/usr/bin/env bash
# Database proof of concept — run twice on the same data dir to prove
# cross-process persistence.
set -euo pipefail
cd "$(dirname "$0")/.."

DATA_DIR="${1:-$PWD/.poc-data}"
echo "==> DB proof of concept (data dir: $DATA_DIR)"
echo "    run 1 writes a marker; run 2 must read it back"
cargo run --manifest-path backend/Cargo.toml --bin db-probe -- "$DATA_DIR"
echo
echo "==> second run — persistence check:"
cargo run --manifest-path backend/Cargo.toml --bin db-probe -- "$DATA_DIR"