# STT Terminal — Documentation Index

Research-first documentation for the platform.

| Doc | Read it to… |
|---|---|
| [ARCHITECTURE.md](ARCHITECTURE.md) | Understand services, data flow, the ledger invariant, failure guards |
| [API.md](API.md) | Call every REST endpoint + WebSocket event (including CSV exports) |
| [DATA_MODEL.md](DATA_MODEL.md) | Learn the tables, money semantics, and verified invariants |
| [STRATEGIES.md](STRATEGIES.md) | Understand the five strategies, their knobs, and what their signals mean |
| [CONFIGURATION.md](CONFIGURATION.md) | Configure stt.toml, env vars, providers, slippage/fees |
| [RESEARCH.md](RESEARCH.md) | Run a research session: exports, joins, backtests, A/B, veto analysis |
| [decisions/](decisions/) | Architecture decision records (why the ledger is the source of truth, etc.) |
| ../AGENTS.md | Operational context for AI agents (pglite quirks, pins, conventions) |
| ../README.md | Project overview + quick start |

## Suggested reading order for new researchers

1. **README.md** — what this is, get it running in 2 minutes.
2. **RESEARCH.md** — the workflow you'll actually use.
3. **STRATEGIES.md** — what the five models do.
4. **DATA_MODEL.md** — how to interpret every column you export.
5. **API.md** — when you need programmatic access.

## Decisions worth knowing

- Ledger = single source of truth for cash (see `decisions/0001-ledger-source-of-truth.md`).
- Paper executor = single-writer actor; every trade one DB transaction.
- Strategies are pure functions shared by live loop and backtest engine.
- Demo feed is deterministic (seed+tick) with narrow volume bursts so
  rel-volume strategies genuinely exercise.

## Generating up-to-date docs

Docs are hand-maintained against the current code; after any change to the
API surface (routes, WS events, config keys) update the relevant doc —
`AGENTS.md` is read by future AI sessions, so keep it honest.