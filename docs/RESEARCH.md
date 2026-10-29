# Research guide

This platform is built so you can **do research**, not just watch a demo:
deterministic feed, historical snapshots, CSV exports, signal vetoes, and a
generic backtest engine.

## 1. Headless research session

```bash
# clean slate, deterministic demo feed
STT_CONFIG=stt.toml ./backend/target/release/stt &   # or dist run.sh
curl -s -X POST http://127.0.0.1:8765/api/v1/strategies/momentum/start \
  -H 'Content-Type: application/json' -d '{"mode":"demo","execution":"paper"}'
# let it run: ~3 s/tick, 20 ticks ≈ 1 min
```

## 2. What data you can pull

Everything the DB knows, via CSV:

```bash
for k in fills orders ledger signals positions equity backtests risk-events system-events; do
  curl -s "http://127.0.0.1:8765/api/v1/export/$k" -o "$k.csv"
done
```

- `signals.csv` — **every strategy decision, including vetoes** (`decision`,
  `reason`, `score`, `ref_price`). This is your research ground truth for
  "what would this strategy have done".
- `fills.csv` + `ledger.csv` — what actually traded and the exact cash math
  (25 bps fee, 10 bps slippage are in there).
- `equity.csv` — per-tick equity curve (from the dashboard metric series).

## 3. Join the chain signal → order → fill → ledger

Signals carry `snapshot_id` / reasons; orders carry `strategy_id` + side +
mint; fills carry `order_id`; ledger entries carry `ref_id` (order id).
In pandas:

```python
import pandas as pd
sig = pd.read_csv('signals.csv')           # decision/score for every eval
fill = pd.read_csv('fills.csv')            # actual fills
fill['order_id'].astype(str)               # join to orders.csv
led = pd.read_csv('ledger.csv')            # balance_after = ground truth
```

Then: per-strategy win rate and expected value **on the submitted subset**
(max no. of trades), fee drag (fills.fee_lamports.sum() vs gross PnL),
slippage cost of frequency (scalper will be fee-dominated; momentum should
still clear costs).

## 4. Backtests

`POST /api/v1/backtests` (see API.md) replays stored history deterministically.
Inspect per-run equity + closed clusters from `GET /api/v1/backtests/{id}`;
export as CSV via `/api/v1/export/backtests`.

## 5. A/B and reproducibility

- Each `data-directory` is a full experiment container. Copy it before a run
  and you can diff two runs exactly (deterministic demo).
- Reset the account back to initial deposit with `POST /api/v1/demo/reset`.
- Change strategy config, `/stop` + `/start`, and re-run — same feed, tuned
  knobs, clean signal history (or wipe the DB dir for a fully fresh sample).

## 6. What the vetoes are telling you

`risk-events.csv` == everything risk manager stopped. Inspect the top
`reason`s: in demo you'll see scalper entries rejected because modeled cost
(90 bps) exceeds expected edge — that is the kind of insight to carry into
config tuning (`min_edge_bps`, `round_trip_cost_bps`).

## 7. Limits

- Signal-history and export rows cap at `EXPORT_LIMIT=100_000` (header
  `X-Export-Truncated`).
- Cache holds 300 snapshots per mint; for long-horizon studies, persist
  snapshots to TSV yourself (the `/markets` feed is streaming via WS).
- Real (dexscreener) mode needs network; demo is offline & deterministic.