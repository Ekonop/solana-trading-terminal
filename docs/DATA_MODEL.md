# Data model

Schema lives in `backend/migrations/0001_init.sql` + `0002_token_meta.sql`.
Produced by the embedded PostgreSQL (pglite) engine.

## Conventions

- **Money = integer lamports** (i64). Prices = `DECIMAL(40, 20)` NUMERIC.
  `rust_decimal` is pinned `=1.43.0`; pglite `row_i32` for INT, `row_i64`
  for BIGINT.
- **Timestamps = i64 microseconds** since epoch, all UTC (`backend/src/time.rs`).
- IDs are 128-bit random hex strings (`ids::new_id()`), 32 chars.
- SOFT-delete: none. Rows are immutable; state advances via new rows + a
  current-status column on the domain row (orders/positions).

## Tables

### `paper_accounts`
| col | notes |
|---|---|
| id | pk |
| cash_balance_lamports | i64 — **authoritative, must equal newest ledger `balance_after`** |
| realized_pnl_lamports | cumulated at each fill |
| created_at, updated_at | |

One account is seeded by `initial_deposit` ledger entry at boot.

### `paper_orders`
Side `buy|sell`, order_type `market|limit`, status `open|filled|canceled|rejected`.
`strategy_id NULL` = manual. `limit_price` NUMERIC nullable.
Rows are inserted **before** fills (FK dependency).

### `paper_fills`
`order_id` → `paper_orders.id` (FK). `snapshot_ref` → market snapshot row.
`is_paper` always true (simulation boundary).

### `ledger`  ← single source of truth for cash
`kind`:
- cash-movers: `initial_deposit`, `buy`, `sell`, `fee`, `adjustment`
- informational: `realized_pnl` (amount recorded, cash already moved),
  `signal` (research row)

`balance_after` is written by the executor inside the same transaction as
`set_balance`. Newest `balance_after` == `paper_accounts.cash_balance_lamports`.

### `positions`
Per-mint open position: `avg_entry_price` (NUMERIC), `qty_raw`,
`unrealized_pnl_usd` (computed at read time against mark), `opened_at`,
`updated_at`. Upsert on buy, closed on qty→0.

### `position_closures`
Closed-trade records: `mint, qty_raw, entry_price, exit_price,
realized_pnl_lamports, opened_at, closed_at, strategy_id`. Populated on close
— the original trade cluster for equity attribution.

### `signals`
One row per strategy evaluation outcome (including rejections):
`strategy_id, strategy_version, mint, direction, score NUMERIC, decision,
reason, ref_price, data_observed_at, snapshot_id?, created_at`.
This is the research feed; score is a Decimal (may be long).

### `market_snapshots`
One row per observation: mint, symbol, price NUMERIC, volume_24h, liquidity,
`observed_at`, source (`demo` simulated / dscs real), unique per
(mint, observed_at). Backtests replay from here.

### `backtest_runs` / `backtest_tick_equity` / `backtest_position_closures`
Backtest metadata, per-tick equity curve, and closed clusters per run.

### `risk_events` / `system_events`
Risk vetoes and system lifecycle events, all with `created_at`.

### `settings`
Effective config snapshot persisted at boot (research reproducibility).

## Verified invariants

1. Newest `ledger.balance_after` == `paper_accounts.cash_balance_lamports`
   (exact integer, checked live while strategies were trading).
2. `paper_fills.order_id` always resolves to an existing order row.
3. Optional references are `NULL` — never empty strings.
4. Every trade = one DB transaction; a crash between steps cannot occur
   (pglite serializes on one async mutex per connection anyway).
5. Demo feed determinism: same seed → same (price, volume) per tick, so
   research over demo data is reproducible run-to-run.