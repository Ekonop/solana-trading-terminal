# API Reference

Base: the value of `STT_API` (default `http://127.0.0.1:8765`).

All JSON is kebab-case. Timestamps are **microseconds since epoch** (i64).
Money is integer lamports; USD prices are Decimal-as-string (may be very long).

Conventions:
- Pagination: `?limit=<int ≤500>&offset=<int>` → `{"items": [...], "total": n}`
- Errors: JSON `{"error": "message", "code": "..."}` + non-2xx status
- All write endpoints are idempotent-by-request-id or guarded by actor state

---

## System

### `GET /health`
Liveness. Returns `{"status":"ok"}`.

### `GET /api/v1/status`
```json
{"mode":"demo","provider":{"state":"healthy","simulated":true,"consecutive_failures":0},
 "simulated_data":true,"version":"0.1.0","database":{"data_dir":"pglite","initialized":true}}
```

### `GET /api/v1/settings`
Current effective configuration (what the server is actually running with).
Mirrors `stt.toml` sections but flattened; includes
`paper-trading.slippage-model`, `application.mode`, etc.

### `GET /api/v1/dashboard`
```json
{
  "account": {"cash-balance-lamports": 124584372768, "realized-pnl-lamports": ...},
  "equity": {"equity_usd": "15683.12", "exposure_pct": "0.0001", ...},
  "positions": [...], "strategies": [...], "open_positions": 1,
  "market_overview": {...}
}
```

---

## Markets

### `GET /api/v1/markets`
All observed mints with latest snapshot: price, 24h vol, liquidity,
`history_len`, `updated_at`.

### `GET /api/v1/markets/{mint}`
Single mint (lowercase base58, e.g. `so11111111111111111111111111111111111111112`).

---

## Portfolio / paper trades

### `GET /api/v1/portfolio`
Account + positions + recent fills rolled up.

### `GET /api/v1/positions`
```json
{"positions":[{"symbol":"SOL","qty_raw":1234567890,"qty_human":"1.234567890",
  "avg_entry_price":"150.02","mark_price":"150.31",
  "unrealized_pnl_usd":"0.35", "mint":"so111..."}]}
```

### `GET /api/v1/orders?limit=&offset=`
Paper orders. Fields: `id, account_id, side, order_type, mint, qty_raw,
limit_price?, status, strategy_id, created_at, filled_at?`.

### `GET /api/v1/orders/{id}/cancel`
Cancel an open paper order.

### `GET /api/v1/fills?limit=&offset=`
Fills: `id, order_id, price, qty_raw, fee_lamports, fee_usd, slippage_bps,
is_paper`.

### `GET /api/v1/ledger?limit=&offset=`
Ledger rows newest-first: `kind, amount_lamports, balance_after, ts, note,
ref_id`. **Kinds that move cash:** `initial_deposit|buy|sell|fee|adjustment`.
`realized_pnl` is informational.

### `POST /api/v1/orders` (manual order from client)
Body (kebab-case):
```json
{"size-usd": 50, "order-type": "market", "mint": "so11111111111111111111111111111111111111112"}
```
Response: the created order / fill result. `order-type`: `market` (default)
or `limit` (with `limit-price`).

### `POST /api/v1/stop-all`, `POST /api/v1/demo/reset`
Stop all strategy tasks / reset demo DB + account back to initial deposit.

---

## Strategies

### `GET /api/v1/strategies`
```json
[{"id":"momentum","status":"paper_trading","signals_total":15,
  "config":{...}}]
```
Statuses: `disabled | paper_trading | paused | error`.

### `GET /api/v1/strategies/{id}`, `GET /api/v1/strategies/{id}/config`
Detail + current config JSON.

### `POST /api/v1/strategies/{id}/start`
Body: `{"mode":"demo","execution":"paper"}` → `{"status":"paper_trading"}`.

### `POST /api/v1/strategies/{id}/pause`, `POST /api/v1/strategies/{id}/stop`

### `GET /api/v1/signals?limit=&offset=&strategy=`
`SignalRecord`: `id, strategy_id, mint, direction, score?, decision,
reason, ref_price, data_observed_at, snapshot_id?`.
`decision`: `submitted | approved | rejected | hold`.

Signals are persisted for **every** evaluation round, including rejections —
research the vetoes, not just the trades.

---

## Risk events

### `GET /api/v1/risk/events?limit=&offset=`
Risk decisions + why. `GET /api/v1/system/events` similarly for system-level
events (mode changes, provider health, shutdown).

---

## Backtests

### `GET /api/v1/backtests?limit=&offset=`
Runs: `id, status (running|done|failed), started_at, finished_at?,
metrics {total_return_pct, sharpe, max_drawdown_pct, trades}, strategy_ids`.

### `GET /api/v1/backtests/{id}`
Run + per-tick equity curve (`equity_curve: [[ts, usd], ...]`) + closures.

---

## CSV exports

Every table has a CSV export. Columns mirror the DB schema.

```
GET /api/v1/export/fills
GET /api/v1/export/orders
GET /api/v1/export/ledger
GET /api/v1/export/signals
GET /api/v1/export/positions
GET /api/v1/export/equity
GET /api/v1/export/backtests
GET /api/v1/export/risk-events
GET /api/v1/export/system-events
```

- Response: `Content-Type: text/csv` + `Content-Disposition: attachment`.
- `X-Export-Truncated: true` header present if rows were cut at
  `EXPORT_LIMIT = 100_000`.
- `positions` exports the *open* position row; closed positions live in
  `position_closures` (see DATA_MODEL) and are included via the fills/ledger
  exports.

---

## WebSocket `/api/v1/ws`

Server pushes (JSON):
```json
{"t":"market-batch","count":12,"simulated":true,"observed_at":1793233030509629}
{"t":"snapshot","msg":{...MarketSnapshot...}}
{"t":"equity","msg":{...}}
{"t":"signal","msg":{...SignalRecord...}}
{"t":"order","msg":{...}}
{"t":"fill","msg":{...}}
{"t":"strategy","id":"momentum","status":"paper_trading"}
{"t":"risk","msg":{...}}
{"t":"health","provider":"demo","healthy":true,"provider_up":true,"stale":false}
{"t":"backtest","id":"...","status":"done"}
```

Client sends `{"t":"watch","mints":["so111..."]}` to subscribe to snapshots
for specific mints (the server tracks watchlists).