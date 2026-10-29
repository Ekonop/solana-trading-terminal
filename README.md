# STT Terminal — Solana Paper Terminal

A local-first **Solana trading research platform**: deterministic backtesting,
multi-strategy paper trading, a real embedded PostgreSQL engine, a live
strategy loop, and a research export layer — all running on your own machine.

```
┌────────────┐   REST + WebSocket    ┌─────────────────────────────────┐
│  Flutter   │ ───────────────────▶ │  Rust backend (Axum + tokio)     │
│  client    │ ◀─────────────────── │  ├─ market-data service          │
│ (desktop / │   /api/v1/* :8765    │  ├─ strategy service (N tasks)   │
│  android / │                      │  ├─ risk manager                 │
│  ios)      │                      │  ├─ paper executor (single actor)│
└────────────┘                      │  └─ pglite embedded PostgreSQL   │
                                    └─────────────────────────────────┘
```

**Simulation boundary is absolute**: there is no real-trade path, no wallet
private keys, no live-execution toggle. Every order is a paper fill in the
embedded database; the ledger is the single source of truth for the balance.
Demo data is always labelled `simulated`.

---

## Quick start

### Option A — self-contained bundle (no toolchain)

```bash
./dist/stt-terminal-0.1.0/run.sh
```

Launches the embedded backend (127.0.0.1:8765) + desktop client, shuts the
backend down gracefully on exit. Reset demo state: `rm -rf var/pglite`.

### Option B — from source

```bash
# backend
cd backend
cargo build --release
STT_CONFIG=../stt.toml ./target/release/stt

# frontend (another terminal)
cd frontend
flutter run -d linux
```

Point the client at a remote backend with `--dart-define=STT_API=http://host:8765`.

### Option C — research headless

Run just the backend and use the REST API + CSV exports (see
[docs/RESEARCH.md](docs/RESEARCH.md)).

---

## What's inside

| Area | Where | Notes |
|---|---|---|
| Backend | `backend/` | Rust, Axum, tokio, pglite (embedded PostgreSQL) |
| Frontend | `frontend/` | Flutter desktop-first, dark terminal UI |
| Migrations | `backend/migrations/` | Schema: orders, fills, ledger, signals, backtests… |
| Strategies | `backend/src/strategies/` | sniper, momentum, dip_hunter, whale_tail, scalper |
| Backtesting | `backend/src/backtesting/` | Deterministic replay engine |
| Research docs | `docs/` | Architecture, API, data model, strategies, config |
| Bundle | `dist/` | Self-contained Linux tarball |
| Scripts | `scripts/` | Build/test/package helpers |

## Core documents

- **[docs/ARCHITECTURE.md](docs/ARCHITECTURE.md)** — services, actors, threading, invariants
- **[docs/API.md](docs/API.md)** — full REST + WebSocket reference (including exports)
- **[docs/DATA_MODEL.md](docs/DATA_MODEL.md)** — tables, ledger semantics, exactly-once
- **[docs/STRATEGIES.md](docs/STRATEGIES.md)** — the five strategies and their signals
- **[docs/CONFIGURATION.md](docs/CONFIGURATION.md)** — `stt.toml` + env vars
- **[docs/RESEARCH.md](docs/RESEARCH.md)** — how to run research with exports + backtests
- **[.env-examples/](.env-examples/)** — env var reference

## Two-minute live demo

```bash
# 1. start everything
cd dist/stt-terminal-0.1.0 && ./run.sh

# 2. start the momentum strategy (paper)
curl -s -X POST http://127.0.0.1:8765/api/v1/strategies/momentum/start \
  -H 'Content-Type: application/json' -d '{"mode":"demo","execution":"paper"}'

# 3. watch it decide
curl -s "http://127.0.0.1:8765/api/v1/signals?limit=10"

# 4. pull research data
curl -s http://127.0.0.1:8765/api/v1/export/fills -o fills.csv
```

## Status

Everything below verified green on a clean database this session:

- Backend compiles warning-free (`cargo build --release`), 14/14 unit tests pass
- Live strategy loop produces **strategy-generated** orders, fills, positions,
  and ledger entries; risks rejected with reasons (e.g. scalp edge < round-trip cost)
- Ledger running balance == account cash at all times (exact integer match)
- Order dialog validated against the real API contract (`size-usd`, `order-type`)
- Frontend: `flutter analyze` clean (2 style infos), 8/8 tests pass
- Client runs over REST+WS; all 7 screens + new RESEARCH tab built

## Constraints honoured

- `pglite-rs` pinned exactly `=0.2.7`, `rust_decimal` pinned `=1.43.0`
- Ledger entries are the **single source of truth** for the final balance
- Config is kebab-case everywhere (`bind-address`, `[market-data]`)
- Backend is not bundled into mobile clients (x86_64 Linux process cannot run
  on-device); phones point at a reachable host via `STT_API`