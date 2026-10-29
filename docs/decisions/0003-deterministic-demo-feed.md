# ADR-0003 — Deterministic demo feed (seed + tick) with real volume bursts

Status: accepted

## Context

The first demo feed generated volume as `base ± 3% noise`, so momentum's
`rel_volume ≥ 1.2` could never fire — the platform looked like a demo that
never trades. Making the feed *more* random would have broken the
reproducibility promise (research must be able to re-run an experiment and
get the same stream).

## Decision

1. Every price/volume/liquidity draw is a pure function of
   `(seed, token_index, tick)`: deterministic, stateless, reproducible from
   any process with the same seed.
2. Price = sinusoidal wave + mean-reverting regime drift (`0.35 × amplitude`)
   + small noise. Drift is deliberately subtle so momentum's 2-bar
   confirmation gate passes (strong drift → bars reverse → no confirmations).
3. Volume = base × burst multiplier. Bursts are **narrow (1–2 ticks)** so
   `recent / trailing average` genuinely spikes (wide plateaus let the
   average catch up and flatten rel-vol). Burst chance ~6–18% per tick,
   multiplier 1.6×–6× — enough for whale_tail (`≥3×`) and momentum (`≥1.2×`)
   to trigger periodically, not always.

## Consequences

- Verified: a regression test asserts `max rel-vol > 1.5` over 600 ticks of
  the BONK series; the live loop produced submitted momentum round trips
  (buy → take-profit sells) and scalper entries/exits within minutes.
- Same-seed runs are byte-identical in the feed; experiment A/Bs are fair.
- Strategy authors must accept that demo dynamics ≠ any real market: treat
  demo as a *deterministic harness*, not a claim about real alpha.