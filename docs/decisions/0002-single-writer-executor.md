# ADR-0002 — Paper executor is a single-writer actor; every trade is one DB transaction

Status: accepted

## Context

Trades arrive from several sources (manual order API, live strategies,
backtest closures). Without serialization, a concurrent buy+sell on the same
mint could read the same cash balance and both "succeed", creating a
double-spend. pglite serializes statements on one connection, but that is a
low-level guarantee — not a domain-level one, and it can't express
read-modify-write atomicity across a multi-step apply.

## Decision

1. All fills run inside `trading::executor` as a single tokio actor: every
   intent goes into an mpsc channel; one task processes them one at a time
   with the DB connection held.
2. `trading::apply::insert_and_fill` performs the fill in ONE transaction:
   insert order row (status `open`) → compute position math from the ledger
   balance → write ledger entries → set balance → upsert/close position →
   set order status `filled` → insert fill row. Any step failure rolls back
   the whole trade.
3. Manual orders are hedged as `strategy_id: "manual"` sentinel, mapped to
   NULL at intent-construction time.

## Consequences

- No partial applies: a crash mid-trade leaves the order row `open` and no
  fill — recoverable, not corrupt.
- The FK invariant "order row exists before any fill" holds by construction.
- Single-writer also gives deterministic ordering of fills for backtest
  replay of the live loop (same sequence of intents → same ledger history).