# Architecture

## High-level picture

```
                    ┌─────────────────────────────── backend (Rust) ───────────────────────────────┐
                    │                                                                             │
  REST /api/v1/* ──▶│  api::handlers  ──▶ repos ──▶ pglite (embedded PostgreSQL)                  │
  WS   /api/v1/ws ─▶│        │              ▲           ▲                                          │
                    │        │         ┌───┘           │                                          │
                    │        ▼         │               │                                          │
  Flutter client    │   EventBus ──────┤  market_data::service (tick loop, cache, provider)       │
  (thin UI)         │                 │               │                                          │
                    │                 ▼               │                                          │
                    │   strategies::service ──▶ risk::manager ──▶ trading::executor (actor)      │
                    │        (N tasks)                    │              │                        │
                    │                                     ▼              ▼                        │
                    │                           repos + one transaction per trade                  │
                    └─────────────────────────────────────────────────────────────────────────────┘
```

- **One process.** `main.rs` loads config, opens the embedded `PGlite`, wires
  `AppState`, starts the market-data loop and the HTTP+WS server.
- **One `PGlite` per process.** Can't reopen after close. All DB access goes
  through a `DbLike` seam (`db/mod.rs`) so tests can run against a
  lightweight double; `repos/**` are the *only* files that interpret rows
  (`row_*` helpers in `db/numeric.rs` enforce types).
- **Single-writer paper executor.** All fills happen inside one tokio actor
  (`trading/executor.rs`) — sequenced read-modify-write, no concurrent
  double-spend even though pglite serializes statements anyway.
- **Strategies are pure functions.** `StrategyService` runs one task per
  enabled strategy; evaluation is stateless (config + current cache), so the
  identical code path powers the live loop *and* the backtester.

## Data flow (live loop)

1. `market_data::service` wakes every `refresh_interval_ms`, asks the
   provider (`mock` in demo, `dexscreener` for the real feed), and pushes
   observations into `MarketCache` (ring buffer per mint, capacity 300).
2. For each enabled strategy, `StrategyService` asks `cache.history_for(mint,
   n)` and calls the strategy's `evaluate(ctx)`. A strategy returns
   `Vec<SignalProposal>`.
3. `RiskManager::gate(signals)` applies every rule (position limits, drawdown,
   liquidity, cooldowns…) and returns a decision per signal: `approve`,
   `reject { reason }`, `hold`. **Approved AND rejected signals both get a
   `Signals` row persisted** (so research can inspect what risk vetoed).
4. Approved buys/sells go to `Executor` as intents. Executor validates
   snapshot freshness, applies slippage + fees, and calls
   `trading::apply::insert_and_fill` — **one atomic transaction** that inserts
   the order row, computes position math from the *ledger balance*, writes
   ledger entries, upserts the position, and emits `Fill`/`Order` WS events.
5. Every mutation is broadcast on the `EventBus` (tokio `broadcast`, 1024
   capacity); the WS server fans it out.

## Backtesting

`backtesting::engine` replays a stored history: it re-inserts each historical
`MarketSnapshot` into a fresh `MarketCache`, then evaluates enabled strategies
with the same pure functions. Results land in `backtests` (per-tick equity),
`position_closures` (cluster of trades), and summary `backtest_runs`.

## The ledger as single source of truth

```text
ledger: (id, account_id, ts, kind, amount_lamports, balance_after, ...)
```

- Cash-move kinds: `initial_deposit`, `buy`, `sell`, `fee`, `adjustment`.
- `realized_pnl` entries are **informational** — they carry an amount but do
  not move cash (the cash already moved at the `sell`).
- The executor computes the running `balance_after` itself; `set_balance`
  writes that exact integer to `paper_accounts.cash_balance_lamports`.
- Invariant (verified live): newest `balance_after` == account cash balance.
  Auditing code should sum only the cash-move kinds.

## Failure modes & guards

| Risk | Guard |
|---|---|
| Insufficient cash | ledger loop returns `insufficient-balance` rejection before commit |
| Insufficient position on sell | `InsufficientPosition` error; sell quantity clamped to held |
| Stale quotes in orders | `max_price_age_ms` in risk manager |
| Rejected-by-risk spam | signals persisted with `decision=rejected` + reason |
| Unbounded history | cache is a bounded ring (300), memory pinned |

## Config precedence

1. built-in defaults (`src/config.rs`)
2. `STT_CONFIG` file (kebab-case, all sections)
3. `STT_*` env vars (override)

## Key invariants

- Money is integer lamports everywhere; prices are `Decimal` NUMERIC; the
  only floats in the system are inside strategy *scores/reasons*.
- Every trade is exactly one DB transaction — no partial apply states.
- Order row exists before any fill/signal that references it (FK).
- Optional FKs bind real `NULL`, never `""` (23503).
- The demo provider is deterministic: same seed ⇒ same stream.