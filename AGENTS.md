# Agent Context — STT Terminal (Solana Trading Terminal)

This file gives AI agents (and humans new to the repo) the operational facts
that cost several debugging rounds to learn. Read it before touching code.

## What this is

Local-first Solana **research + paper trading** platform. Rust backend
(Axum/tokio/pglite embedded PostgreSQL), Flutter frontend (desktop-first,
later Android/iOS targets). **No real trades, no wallet keys, no live-
execution toggle. Ever.** Demo data is always labelled simulated.

## Hard-won operational facts

### pglite-rs (pinned exactly `=0.2.7`)
- **Boot quirk**: stdout redirected to a plain file → pglite may silently
  `exit 100` at boot. Use a pty, a TTY, a pipe, or process substitution:
  `setsid ./backend/stt > >(tee -a "$LOG") 2>&1 < /dev/null &`. The bundle
  launcher (`run.sh`) already does this.
- **Shutdown**: only SIGINT is graceful. SIGTERM corrupts the data dir
  (58P01). Use `kill -INT <pid>`; `pkill -x stt` matches the exact name only
  (never `pkill -f`).
- **Row typing**: INT columns read with `row_i32`; `row_i64` throws
  `bad integer column`. `pglite::Row` has no `is_null()` — use the
  `row_*_opt` helpers in `backend/src/db/numeric.rs`.
- **Single instance per process**: exactly one `PGlite` open; cannot reopen
  after close. Restart persistence is tested by launching a fresh process.
- All statements/transactions serialize on an internal async mutex, so
  concurrent double-spend is structurally impossible; trades still go through
  the single-writer executor actor for ordered read-modify-write.

### tokio watch self-deadlock
Never write `tx.send(rx.borrow().wrapping_add(1))` in one expression — the
temporary `Ref` holds the read lock during `send()`'s write lock, deadlocking
`wait_for_readers`. Split into two statements (see `market_data/service.rs`).

### Optional FK columns must bind real NULL
Binding `unwrap_or("")` to a `REFERENCES` column violates 23503. Use
`Option<&str>` — postgres-types `ToSql for Option<T>` → `IsNull::Yes`. Applied
in `insert_order` (repos/trading.rs) and `insert_signal` (repos/strategies.rs).

### Order-row FK invariant
`paper_fills.order_id` FKs to `paper_orders.id`; the order row must exist
before any fill/signal attaches to it. `insert_and_fill` inserts the order
row first (status open), then outcome paths only finalize status.

### Manual orders
API handler fabricates `strategy_id: "manual"` (there is no strategies row for
it) → executor maps the sentinel to `None`/NULL at intent-construction time.

### API wire contract
- kebab-case JSON: `SubmitOrderReq` = `size-usd`, `order-type`, `limit-price`.
- SOL mint is lowercase: `so11111111111111111111111111111111111111112`.
- Strategy keys/mints in signals are kebab: `strategy-id`, `generated-at`.

### Frontend
- `fl_chart 1.3.0`: no `getTooltipBorder`; use `tooltipBorderRadius`.
- `AppState` starts timers, screens are pure renderers — safe to pump in tests.
- `ErrorBanner` text is `BACKEND UNREACHABLE — $message` (prefix!!).
- `JsonHelpers` (models/models.dart) accepts kebab/snake keys and degrades
  malformed endpoint answers to empty collections instead of crashing — the
  refresh loop must never raw-`as Map/List` cast (that painted a red banner).
- Multi-target build: `flutter create --platforms android,ios --org dev.stt .`

### Backend config
- All keys and section headers are kebab-case (`bind-address`,
  `[market-data]`); snake keys fail parse with confusing "missing field" errors
  pointing at unrelated lines. Keep `stt.toml`/`stt.toml.example` kebab.
- Source precedence: defaults → `STT_CONFIG` file → `STT_*` env vars.
- Demo mode default data dir `./data/pglite` is relative to CWD — always set
  `STT_CONFIG` (bundle run.sh exports it).

### Toolchains (user-space, no sudo)
- Android SDK `~/Android/Sdk` (platform-36, build-tools 36.0.0); JDK needs
  `export JAVA_HOME=~/opt/jdk-21.0.13+9` for gradle.
- iOS release AOT: **use Flutter 3.47.6** (`/home/vansh/flutter-3.47.6`) —
  3.47.7 has no published Linux `gen_snapshot`. Build via
  `xcross-dev flutter build --release -i`; xcross config needs a TTY or reads
  `~/.config/xcross/*.json`.
- Official Dart/Flutter MCP: `dart_mcp_server` (dart-lang/ai) at
  `~/.pub-cache/bin/dart_mcp_server`; npm forks demand a paid token — reject.
- Mobile clients are thin Flutter UIs pointing at a remote `STT_API`; the
  embedded backend is an x86_64 Linux process and cannot run on-device.
  Linux bundling (`scripts/package-linux-bundle.sh`) is the self-contained path.

### Repo conventions
- Money = integer lamports; prices = Decimal NUMERIC; no binary floats.
- Ledger entries are the single source of truth for final cash. Latest
  `balance_after` must equal `paper_accounts.cash_balance_lamports`. Note:
  `realized_pnl` ledger entries carry an amount but do NOT move cash
  (informational) — sum only `initial_deposit|buy|sell|fee|adjustment` for
  a cash audit.
- Strategies are pure functions with one interface, used live and in
  backtests; replay inserts each snapshot before evaluation.
- Demo provider is deterministic: every price/volume draw is a pure function
  of `(seed, tick, token index)`; it now emits narrow volume bursts +
  mean-reverting price regime so rel-volume strategies actually fire.

## Layout cheatsheet

```
backend/src/
  app.rs            AppState wiring: db, cache, bus, executor, strategies
  main.rs           Config load, shutdown handling (SIGINT)
  api/              axum routes: handlers.rs, exports.rs (CSV), ws.rs
  trading/          executor.rs (actor), apply.rs (fill+ledger tx), risks
  strategies/       service.rs (task manager), traits/registry, 5 strategy files
  risk/manager.rs   central risk gates (rejects get signals + risk events)
  repos/            the ONLY place touching pglite rows; exports.rs = CSV
  market_data/      providers (mock=deterministic demo, dexscreener), cache
  backtesting/      deterministic replay engine
  db/numeric.rs     row_* typed helpers; migrations.rs
frontend/lib/
  state/app_state.dart, api/, models/, screens/ (7 + research), widgets/
scripts/            run-backend.sh, test-backend.sh, package-linux-bundle.sh
stt.toml(.example), .env-examples/  config reference
docs/               ARCHITECTURE, API, DATA_MODEL, STRATEGIES, CONFIG, RESEARCH
```

## Secrets (security best practice — Helius & Jupiter docs)

- All keys RPC/Jupiter/wallet are resolved **server-side only**, from process
  env or a gitignored `.env` (either repo root, or beside the bundle).
  `backend/src/secrets.rs` is the single source: `ResolvedSecrets`, `Secret`
  (redacts itself in Debug/Display), `load_dotenv_if_present()`.
- Never set keys in the committed `stt.toml`; the client never receives them;
  no API endpoint returns them; logs show only `<redacted>`.
- `scripts/run-backend.sh` and the bundle `run.sh` both `source .env` if
  present; the packager strips `.env /*.pem` from the tarball.
- `.env` / `.env.local` / `*.pem` are gitignored; verify before commit.

## Speed of iteration

- Backend boot ~12–16s; `/tmp` tmpfs is 14G (keep <13G or Flutter SIGABRTs).
- Always wait for the port to be free before restarting the backend
  ("Address already in use" otherwise); kill via exact-name/PID techniques.
- After changing backend: `cargo test --release` then repackage the bundle
  with `scripts/package-linux-bundle.sh` (it copies the release binary +
  rebuilt Flutter bundle).