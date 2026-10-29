# ADR-0004 — Real-mode is a gated, opt-in path; the default never broadcasts

Status: accepted
Date: 2026-session

## Context

The platform is paper-only by design. Users asking for "real trading" need a
clear engineering path, but nothing in the codebase may trade real money by
default, and no single config mistake should arm it.

## Decision

1. A `[real]` config section exists but is **disarmed by default**
   (`armed=false`, `dry-run=true`).
2. Live broadcast requires THREE independent conditions:
   a. `real.armed = true` (or `STT_REAL_ARMED=true`),
   b. `real.dry-run = false` (or `STT_REAL_DRY_RUN=false`),
   c. a runtime `POST /api/v1/real/arm` with written-down operator
      acknowledgement of risk.
3. `RealService.can_broadcast()` is the single check every real order
   passes; it is `false` unless all three hold.
4. A max-loss kill disarms the runtime gate so a loss spike stops trading.

## Consequences

- Default builds are still 100% paper; the safe path is the zero-config path.
- A careless config edit cannot alone arm the system (runtime gate remains).
- Every real endpoint is non-destructive in the default state (verified live).
