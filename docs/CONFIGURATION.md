# Configuration

The backend reads kebab-case TOML (ALL keys and section headers are
kebab-case — `bind-address`, `[market-data]`, `provider = "dex-screener"`).

Files: `stt.toml` (repo default), `stt.toml.example` (template),
`dist/stt-terminal-0.1.0/stt.toml` (bundle). Env vars `STT_*` override any
key, e.g. `STT_PORT=9000`, `STT_MARKET_DATA_PROVIDER=dex-screener`.

Precedence: built-in defaults → `STT_CONFIG` file → `STT_*` env.

```toml
[application]
mode = "demo"                    # demo (simulated feed) | live (dexscreener)
bind-address = "127.0.0.1"       # use 0.0.0.0 to accept phone/remote clients
port = 8765
log-level = "info"

[database]
data-directory = "./var/pglite"  # bundle uses ./var/pglite, repo ./data/stt

[market-data]
provider = "dex-screener"        # demo | dex-screener
refresh-interval-ms = 3000
request-timeout-ms = 8000
maximum-data-age-ms = 15000
max-concurrent-requests = 4
backoff-base-ms = 500
max-retries = 3
demo-token-count = 8

[paper-trading]
initial-balance-lamports = 100_000_000_000
fee-bps = 25
slippage-bps = 10
slippage-model = "fixed-bps"     # fixed-bps only
max-order-notional-usd = 25000

[risk]
max-position-fraction = 0.25     # per position / equity
max-total-exposure = 0.8
max-daily-loss = 0.10
max-drawdown = 0.25
max-open-positions = 20
minimum-liquidity-usd = 25000
minimum-volume-24h-usd = 50000
max-slippage-bps = 150
max-price-age-ms = 30000
per-token-cooldown-ms = 60000
max-liquidity-impact = 0.05
stop-loss = 0.15
take-profit = 0.50
```

## Settings API

`GET /api/v1/settings` returns the effective config the server booted with
(what actually applies, including env overrides). The client's Settings tab
mirrors this.

## Environment variables (see .env-examples/)

| var | purpose |
|---|---|
| `STT_CONFIG` | path to TOML (bundle launcher sets it) |
| `STT_BIND` / `STT_PORT` | override address / port |
| `STT_DATABASE_DATA_DIRECTORY` | where the embedded PG data lives |
| `STT_MARKET_DATA_PROVIDER` | demo / dex-screener |
| `STT_LOG_LEVEL` | trace..error |

## Notes

- Changing `data-directory` to a fresh path gives you a clean slate —
  perfect for reproducible research runs. Wipe it (`rm -rf var/pglite`) to
  re-seed.
- The **demo provider is deterministic** per seed — same tick → same price.
  `DEMO_SEED` internal constant. Research runs are reproducible.
- `mode = "live"` uses dexscreener public API (network required); demo mode
  is fully offline.