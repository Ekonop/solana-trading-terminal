# Strategies

Five strategies, all living in `backend/src/strategies/` as pure functions
(config + current cache → proposals). Defaults can be overridden per-strategy
via the config API (`GET/PUT /api/v1/strategies/{id}/config`) or persisted in
the `paper_strategies`-style settings.

> **Heuristic research models.** No strategy claims edge before backtest.
> Signals carry explicit reasons so you (or a notebook) can audit every call.

Each signal: `direction`, `score` (Decimal, higher = stronger), `reason`
(short human rationale), `ref_price`, `proposed_size`.

---

## 1. `sniper` — new-pair / newly-observed liquidity

React to mints that are newly observed in the feed with enough liquidity and
accelerating volume.

| knob | default | meaning |
|---|---|---|
| `min_liquidity_usd` | 50_000 | skip thin books |
| `min_volume_24h_usd` | 5_000 | activity floor |
| `min_observations` | 3 | consecutive counts before eligible |
| `min_rel_volume` | 1.5 | recent/trailing-average volume |
| `min_price_rise_pct` | 0.02 | price must be rising |

Best on fresh-listings feeds; in demo, dormant until a new mint (NEPT)
appears on tick ≥ 4.

## 2. `momentum` — short-term breakout continuation

| knob | default | meaning |
|---|---|---|
| `short_window` / `long_window` | 4 / 12 | return lookbacks |
| `momentum_threshold` | 0.02 | min 4-bar return to signal |
| `rel_volume_threshold` | 1.2 | volume confirmation |
| `entry_confirmation_period` | 2 | last N bars must not reverse >2% |
| `next confirm`: `exit_drop_pct` | 0.05 | stop |
| `take_profit_pct` | 0.10 | TP |

Verification note: with the demo feed's mean-reverting price regime +
narrow volume bursts, momentum fires realistic buy → take-profit round trips
(rel-vol ~1.6, 4-bar return ≥ thresholds, TP exit +10/+16%).

## 3. `dip_hunter` — mean-reversion on strong trends

Buys dips inside an uptrend.

| knob | default | meaning |
|---|---|---|
| `trend_window` | 16 | uptrend lookback |
| `trend_min_return` | 0.08 | min 16-bar return to qualify trend |
| `max_dip_pct` | 0.06 | max pullback allowed |
| `min_history` | 24 | warmup |

Rarely fires in demo (needs-a-trend *and* a dip); fires more on real
products with chop.

## 4. `whale_tail` — unusual volume proxy

`rel_volume(baseline_window=8)` ≥ `min_spike_multiple` (default 3.0) **and**
2-bar follow-through ≥ `follow_through_pct` (0.01). Reason text reminds you
this is a *proxy for large public activity, not direct wallet evidence*.

## 5. `scalper` — tiny edge capture

| knob | default | meaning |
|---|---|---|
| `window` | 3 | lookback |
| `min_edge_bps` | 4 | minimum expected move over window |
| `round_trip_cost_bps` | 90 | spread+fee modeled cost used in `reason` |

The scalper's own cost model (90 bps modeled) rejects entries whose expected
edge < cost — you'll see `rejected: scalp: 161 bps window move vs ~90 bps
modeled costs` in the signals feed. This is intentional: the risk layer
refuses trades that can't beat modeled costs. Exits are `gain < cost → submit
exit` (short trades at cost-covering gain).

---

## Service lifecycle

- `/start` (`{"mode":"demo","execution":"paper"}`) starts one eval task.
- Eval cadence = market refresh interval (default 3 s in bundle).
- Warmups: sniper `min_observations`, others `min_history` bars — schedule
  requires those bars in `MarketCache.history_for`, so give a fresh DB
  ~2–4 minutes after `/start` before expecting the first signal.
- `pause` / `stop` tear down the task; status shows in `GET /strategies` and
  is re-read by the client's strategy tab.

## Attribution & risk notes

- All decisions funnel through `risk::manager` first; `rejected` signals are
  persisted (they show up in `/api/v1/signals` with `decision=rejected`).
- Export signal history per strategy from `GET /api/v1/export/signals` and
  join on orders/fills via `generated_at` / `ref_id` (order id in signal
  `snapshot_id`/reason) for a full chain: signal → order → fill → ledger.
- Demo initial balance: 100 SOL-ish (10_000_000_000 lamports in default
  config; bundle sets 100_000_000_000). Fees 25 bps, slippage 10 bps.