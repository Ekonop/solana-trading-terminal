# ADR-0006 — The live broadcast step is a safe stub until a real signer + arming

Status: accepted (interim)

## Context

A correct Solana transaction signer requires parsing the bincode/shortvec
wire format, signing the message with the wallet key, and splicing the
signature. Doing that right — and only after live-testing against a real RPC —
is the difference between a working tool and one that silently misfires (or
invalidly signs). This sandbox has no funded wallet, no RPC key, and no way to
legitimately operate real money.

## Decision

1. `RealExecutor::execute_buy`'s broadcast branch calls `sign_base64_tx`,
   which for now **always returns a refusal error** ("live transaction signing
   requires the ADR-0006 signer + operator arming; nothing was broadcast").
2. The full gated/sizing/quote/dry-run path is real and tested; only the final
   sign-and-broadcast is stubbed.
3. When real trading is wanted, replace that stub with the actual signer, then
   rehearse on devnet/a $1 soak before any meaningful funds.

## Consequences

- No code path in this build can spend money, even if an operator arms.
- The seam is a single function, so the upgrade is localized and reviewable.
