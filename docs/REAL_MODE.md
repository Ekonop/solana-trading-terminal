# Real-mode (live trading) — implementation status

This documents the **complete Phase 0 → Phase 7 engineering path** for taking
STT from paper-trading to a real wired wallet on Solana. All seven phases are
**implemented as working code** in the backend, compile-tested and
unit-tested, but the system is **disarmed by default** — it cannot broadcast
anything until *you* supply a wallet key, an RPC, and explicitly arm it.

> **Simulation boundary (unchanged):** the default build stays 100% paper.
> Real-mode is an opt-in, gated, single-market micro-capital path. Nobody
> should ever hand this tool real funds it isn't meant to spend.
> — See `docs/decisions/0004-real-mode-safety.md`.

## The one thing only you can do

This sandbox cannot hold or operate real money, and there is no funded
wallet / Solana RPC key / broadcast network here. So the actual **live
signing+broadcast step is implemented as a safe stub** that refuses to sign
(`sign_base64_tx` always returns an error). Everything else — wallet load,
gates, sizing, quote, dry-run logging, reconciliation, REST surface — is real
and tested. To *activate* live trading you must:

1. Provide a funded wallet key (`STT_REAL_WALLET_KEY` or a keypair file).
2. Provide a real RPC URL (and Jupiter API key if using Ultra).
3. Set `real.armed = true` + `real.dry-run = false` (or the env vars).
4. Hook the `sign_base64_tx` stub to a real Solana transaction signer
   (ADR-0006), then do a low-funds rehearsal before any real money.

## Phase-by-phase what's done

### Phase 0 — Scope & safety boundary ✅
- `[real]` config section, **disarmed by default** (`armed=false`,
  `dry-run=true`).
- Double gate: config alone can't broadcast — a runtime
  `POST /api/v1/real/arm` with written acknowledgement is required.
- `RealService` tracks runtime-armed state; disarm on max-loss kill.
- Validation rejects impossible configs (armed+stub, empty RPC).
- ADR-0004 written.

### Phase 1 — Wallet & secure key handling ✅
- `real/wallet.rs`: ed25519-dalek keypair, base58 address.
- Loads from env (`STT_REAL_WALLET_KEY`) or a JSON keypair file.
- **Secret never logged, never returned by API, never written to DB.**
  `Debug` impl redacts it (tested).
- Round-trip sign/verify unit test (`real::wallet`).

### Phase 2 — Solana RPC + read layer ✅
- `real/rpc.rs`: JSON-RPC client — `getBalance`, `getLatestBlockhash`,
  `getSignatureStatuses`. Tested against a local mock HTTP server
  (no network needed).
- `real/reconcile.rs`: `Reconciler` returns the authoritative on-chain
  balance/snapshot.

### Phase 3 — Jupiter adapter ✅
- `real/jupiter.rs`: Ultra `order` (quote + unsigned tx) and `execute`
  call shape, `x-api-key`, error surfacing. Tested (parse + error paths).

### Phase 4 — Paper-fill → real-fill seam ✅
- `real/executor.rs` `execute_buy`:
  gates → chain balance (source of truth) → micro sizing → Jupiter quote →
  dry-run log OR sign+broadcast.
- Dry-run records attempted notional but calls **no** broadcast.
- Chain-as-truth reconciliation (ADR-0005) feeds loss guards.

### Phase 5 — Real-money hard guards ✅
- Must be armed to broadcast; disarm on max loss.
- Chain-balance vs initial micro budget loss check; kill + disarm.
- `max-real-loss-lamports`, per-order notional cap, rent reserve.
- Rate gating via `min-order-interval-ms` (reserved; extended in executor
  by the interval check).

### Phase 6 — Micro-capital sizing for a $5 wallet ✅
- `size_order_usd`: reserves rent+gas, sizes ≤50% of spendable and ≤
  `max-order-notional-usd` ($2), records intended USD.
- Single-market (SOL) only, rent-aware.
- Live-tested: `/api/v1/real/quote-dry` → $2 order, 13.3M lamports.

### Phase 7 — REST endpoints + monitoring ✅
Routes (all live on the running stack, safe in default state):
```
GET  /api/v1/real/status        state, dry-run, can-broadcast, wallet addr
POST /api/v1/real/arm           arm handshake (written ack required)
POST /api/v1/real/disarm        disarm (operator / kill)
GET  /api/v1/real/wallet        address only — never the secret
GET  /api/v1/real/quote-dry     micro-capital sizing preview
GET  /api/v1/real/sol-price     cached SOL mark from market feed
POST /api/v1/real/execute-buy   gated real SOL buy (dry-run by default)
```

## Verified live (this session, default disarmed state)
- `/real/status` → `state=dry-run`, `can_broadcast=false`.
- `/real/arm` → refused (config not armed).
- `/real/wallet` → refused (no key).
- `/real/execute-buy` → refused (no key).
- `/real/quote-dry` → correct $2 sized order.
- `/real/sol-price` → live cached mark (~$150.16).
- Paper mode fully intact (demo, simulated).

## What remains before any real money
- Wire `sign_base64_tx` to a real Solana tx parser + signer (ADR-0006).
- Supply a funded key + RPC + Jupiter key.
- Devnet → $1 live soak → $5, per `docs/RESEARCH.md` rehearsal guidance.
