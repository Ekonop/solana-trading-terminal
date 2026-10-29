# ADR-0001 — Ledger entries are the single source of truth for the paper balance

Status: accepted
Date: 2025-session

## Context

Early implementations had separate bookkeeping paths: the executor computed a
"new balance" locally and also recorded ledger entries. The two drifted —
notably, a buy double-counted its fee (cost included in the buy notional AND
deducted again as a fee entry), producing a tracked-vs-target balance guard
that fired on real trades.

## Decision

1. Every cash movement MUST be a ledger entry: `initial_deposit`, `buy`,
   `sell`, `fee`, `adjustment`.
2. The executor computes the running `balance_after` in the ledger loop
   itself (`trading/apply.rs`), starting from the account's current cash.
3. `set_balance(...)` writes that exact integer to
   `paper_accounts.cash_balance_lamports` in the SAME transaction — the
   tracked field is a cache of the ledger, never an independent computation.
4. `realized_pnl` entries carry an amount but set `moves_cash=false`; cash
   already moved at the `sell`. Audit code must sum only cash-move kinds.

## Consequences

- Invariant (verified live): newest `ledger.balance_after` == account cash.
- The old "tracked vs target" guard was removed; incorrect math now shows as
  a ledger mismatch you can query, not a silent clamp.
- Fees must never be folded into the buy `amount` (bug #1 fixed by keeping
  fee as its own `Fee` kind).