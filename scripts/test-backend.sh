#!/usr/bin/env bash
# Run all backend tests.
set -euo pipefail
cd "$(dirname "$0")/.."
cargo test --manifest-path backend/Cargo.toml --lib --bin stt "$@"