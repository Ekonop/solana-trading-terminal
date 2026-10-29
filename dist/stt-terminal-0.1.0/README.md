# STT Terminal — self-contained Linux bundle

Solana trading **research** terminal with paper trading only:

- No real trades, no wallet keys, no live-execution path (simulation boundary).
- Embedded backend: Rust + pglite (real PostgreSQL engine, local file storage).

## Run

    ./run.sh

The launcher starts the embedded API (127.0.0.1:8765), waits for health,
opens the desktop client, and shuts the backend down gracefully when the
client exits.

## Controls

- **DASHBOARD / MARKETS / STRATEGIES / PORTFOLIO / BACKTESTS / SETTINGS**
- Start strategies from STRATEGIES, run backtests from BACKTESTS.
- Strategy trades are paper fills in the embedded database; the ledger is
  the source of truth.

## Reset demo state

    rm -rf var/pglite        # deletes the embedded database (demo data only)

## Live market feed (optional)

Edit `stt.toml` → `mode = "market-data"` for a read-only DexScreener feed.
Trading stays paper. No network → close the app as-is (demo mode).

## Layout

    backend/stt     embedded API server (Rust)
    var/pglite      PostgreSQL data directory (runtime)
    logs/           backend log
    data/ lib/      Flutter desktop client runtime
