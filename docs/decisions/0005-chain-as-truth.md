# ADR-0005 — For real money, the chain is the source of truth

Status: accepted

## Context

In paper mode, `ledger.balance_after` is the single source of truth (ADR-0001)
because there is no external ledger. In real mode there IS an authoritative
external source: the Solana chain. An internal ledger can drift (unclaimed
rent, tips, gas, airdrops) and must never override what the chain says.

## Decision

1. Real-mode loss guards and position views read from the **on-chain
   balance** via `Reconciler`, never from an internal cash figure.
2. The executor refreshes the chain balance before sizing/guarding.
3. An internal ledger (when kept) is a *record of events*, not a source of
   truth, and reconciles to the chain rather than the reverse.

## Consequences

- Loss caps are enforceable even if the internal state is wrong.
- Rent/gas are visible as chain-balance movements, so micro-capital
  accounting stays honest.
