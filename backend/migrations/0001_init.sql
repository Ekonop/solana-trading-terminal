-- 0001_init.sql — Initial schema for the Solana paper-trading research terminal.
-- Conventions:
--   * Money / lamports are stored as BIGINT (integer base units, never floats).
--   * Prices and USD values are NUMERIC (arbitrary precision, bound as rust_decimal).
--   * Timestamps are BIGINT unix epoch MICROSECONDS (UTC).
--   * UUIDs are TEXT (36 chars).
--   * Every table has server-side CHECK constraints as a defense-in-depth layer.

CREATE TABLE IF NOT EXISTS schema_migrations (
    version    TEXT PRIMARY KEY,
    applied_at BIGINT NOT NULL
);

CREATE TABLE IF NOT EXISTS paper_accounts (
    id                    TEXT PRIMARY KEY,
    name                  TEXT NOT NULL,
    currency              TEXT NOT NULL DEFAULT 'SOL',
    initial_balance_lamports BIGINT NOT NULL CHECK (initial_balance_lamports >= 0),
    cash_balance_lamports BIGINT NOT NULL CHECK (cash_balance_lamports >= 0),
    realized_pnl_lamports BIGINT NOT NULL DEFAULT 0,
    status                TEXT NOT NULL DEFAULT 'active' CHECK (status IN ('active','paused','closed')),
    created_at            BIGINT NOT NULL,
    updated_at            BIGINT NOT NULL
);

CREATE TABLE IF NOT EXISTS strategies (
    id            TEXT PRIMARY KEY,
    display_name  TEXT NOT NULL,
    description   TEXT NOT NULL DEFAULT '',
    status        TEXT NOT NULL DEFAULT 'disabled'
                  CHECK (status IN ('disabled','ready','shadow','paper_trading','paused','faulted')),
    enabled       BOOLEAN NOT NULL DEFAULT FALSE,
    config_json   TEXT NOT NULL DEFAULT '{}',
    config_version INT NOT NULL DEFAULT 1,
    last_eval_at  BIGINT NULL,
    last_error    TEXT NULL,
    created_at    BIGINT NOT NULL,
    updated_at    BIGINT NOT NULL
);

CREATE TABLE IF NOT EXISTS strategy_runs (
    id          TEXT PRIMARY KEY,
    strategy_id TEXT NOT NULL REFERENCES strategies(id),
    status      TEXT NOT NULL CHECK (status IN ('starting','running','paused','stopped','faulted')),
    mode        TEXT NOT NULL CHECK (mode IN ('live','replay','demo')) ,
    config_json TEXT NOT NULL,
    started_at  BIGINT NOT NULL,
    ended_at    BIGINT NULL,
    error       TEXT NULL
);
CREATE INDEX IF NOT EXISTS idx_strategy_runs_strategy ON strategy_runs (strategy_id, started_at DESC);

CREATE TABLE IF NOT EXISTS market_snapshots (
    id             TEXT PRIMARY KEY,
    mint           TEXT NOT NULL,
    symbol         TEXT NULL,
    name           TEXT NULL,
    pair_id        TEXT NULL,
    price          NUMERIC NOT NULL CHECK (price >= 0),
    liquidity_usd  NUMERIC NULL CHECK (liquidity_usd IS NULL OR liquidity_usd >= 0),
    volume_24h_usd NUMERIC NULL CHECK (volume_24h_usd IS NULL OR volume_24h_usd >= 0),
    source         TEXT NOT NULL,
    is_simulated   BOOLEAN NOT NULL DEFAULT FALSE,
    observed_at    BIGINT NOT NULL,
    received_at    BIGINT NOT NULL,
    quality        INT NOT NULL DEFAULT 100 CHECK (quality >= 0 AND quality <= 100)
);
CREATE INDEX IF NOT EXISTS idx_snapshots_mint_ts   ON market_snapshots (mint, observed_at DESC);
CREATE INDEX IF NOT EXISTS idx_snapshots_source_ts ON market_snapshots (source, observed_at DESC);

CREATE TABLE IF NOT EXISTS paper_orders (
    id              TEXT PRIMARY KEY,
    idempotency_key TEXT NOT NULL,
    account_id      TEXT NOT NULL REFERENCES paper_accounts(id),
    strategy_id     TEXT NULL REFERENCES strategies(id),
    mint            TEXT NOT NULL,
    side            TEXT NOT NULL CHECK (side IN ('buy','sell')),
    order_type      TEXT NOT NULL DEFAULT 'market' CHECK (order_type IN ('market','limit')),
    qty_raw         BIGINT NOT NULL CHECK (qty_raw > 0),
    limit_price     NUMERIC NULL CHECK (limit_price IS NULL OR limit_price >= 0),
    status          TEXT NOT NULL CHECK (status IN ('open','filled','cancelled','rejected')),
    created_at      BIGINT NOT NULL,
    filled_at       BIGINT NULL,
    cancel_reason   TEXT NULL,
    UNIQUE (account_id, idempotency_key)
);
CREATE INDEX IF NOT EXISTS idx_orders_account ON paper_orders (account_id, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_orders_strategy ON paper_orders (strategy_id, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_orders_status   ON paper_orders (status, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_orders_mint     ON paper_orders (mint, created_at DESC);

CREATE TABLE IF NOT EXISTS strategy_signals (
    id                TEXT PRIMARY KEY,
    strategy_id       TEXT NOT NULL,
    strategy_version  INT NOT NULL,
    mint              TEXT NOT NULL,
    direction         TEXT NOT NULL CHECK (direction IN ('buy','sell','hold')),
    score             NUMERIC NULL,
    reason            TEXT NOT NULL,
    proposed_size     BIGINT NULL,
    decision          TEXT NOT NULL CHECK (decision IN ('submitted','rejected')),
    rejection_reason  TEXT NULL,
    data_observed_at  BIGINT NOT NULL,
    generated_at      BIGINT NOT NULL,
    order_id          TEXT NULL REFERENCES paper_orders(id)
);
CREATE INDEX IF NOT EXISTS idx_signals_strategy ON strategy_signals (strategy_id, generated_at DESC);
CREATE INDEX IF NOT EXISTS idx_signals_ts       ON strategy_signals (generated_at DESC);
CREATE INDEX IF NOT EXISTS idx_signals_decision ON strategy_signals (decision, generated_at DESC);

CREATE TABLE IF NOT EXISTS paper_fills (
    id            TEXT PRIMARY KEY,
    order_id      TEXT NOT NULL REFERENCES paper_orders(id),
    qty_raw       BIGINT NOT NULL CHECK (qty_raw > 0),
    price         NUMERIC NOT NULL CHECK (price > 0),
    slippage_bps  NUMERIC NOT NULL DEFAULT 0,
    fee_lamports  BIGINT NOT NULL CHECK (fee_lamports >= 0),
    fee_usd       NUMERIC NOT NULL DEFAULT 0,
    snapshot_ref  TEXT NULL,
    filled_at     BIGINT NOT NULL,
    is_paper      BOOLEAN NOT NULL DEFAULT TRUE
);
CREATE INDEX IF NOT EXISTS idx_fills_order ON paper_fills (order_id);
CREATE INDEX IF NOT EXISTS idx_fills_ts    ON paper_fills (filled_at DESC);

CREATE TABLE IF NOT EXISTS positions (
    account_id      TEXT NOT NULL REFERENCES paper_accounts(id),
    mint            TEXT NOT NULL,
    qty_raw         BIGINT NOT NULL CHECK (qty_raw >= 0),
    avg_entry_price NUMERIC NOT NULL DEFAULT 0 CHECK (avg_entry_price >= 0),
    status          TEXT NOT NULL DEFAULT 'open' CHECK (status IN ('open','closed')),
    opened_at       BIGINT NOT NULL,
    updated_at      BIGINT NOT NULL,
    PRIMARY KEY (account_id, mint)
);
CREATE INDEX IF NOT EXISTS idx_positions_status ON positions (status);

CREATE TABLE IF NOT EXISTS ledger_entries (
    id             TEXT PRIMARY KEY,
    account_id     TEXT NOT NULL REFERENCES paper_accounts(id),
    ts             BIGINT NOT NULL,
    kind           TEXT NOT NULL,
    amount_lamports BIGINT NOT NULL,
    balance_after  BIGINT NOT NULL CHECK (balance_after >= 0),
    ref_id         TEXT NULL,
    correlation_id TEXT NULL,
    note           TEXT NULL
);
CREATE INDEX IF NOT EXISTS idx_ledger_account ON ledger_entries (account_id, ts DESC);

CREATE TABLE IF NOT EXISTS equity_snapshots (
    id                    TEXT PRIMARY KEY,
    account_id            TEXT NOT NULL REFERENCES paper_accounts(id),
    ts                    BIGINT NOT NULL,
    cash_lamports         BIGINT NOT NULL CHECK (cash_lamports >= 0),
    positions_value_usd   NUMERIC NOT NULL CHECK (positions_value_usd >= 0),
    equity_usd            NUMERIC NOT NULL CHECK (equity_usd >= 0),
    unrealized_pnl_usd    NUMERIC NOT NULL DEFAULT 0,
    realized_pnl_lamports BIGINT NOT NULL DEFAULT 0,
    exposure_pct          NUMERIC NOT NULL DEFAULT 0
);
CREATE INDEX IF NOT EXISTS idx_equity_account_ts ON equity_snapshots (account_id, ts DESC);

CREATE TABLE IF NOT EXISTS risk_events (
    id          TEXT PRIMARY KEY,
    ts          BIGINT NOT NULL,
    kind        TEXT NOT NULL,
    strategy_id TEXT NULL,
    mint        TEXT NULL,
    order_id    TEXT NULL,
    rule        TEXT NOT NULL,
    detail      TEXT NULL
);
CREATE INDEX IF NOT EXISTS idx_risk_ts        ON risk_events (ts DESC);
CREATE INDEX IF NOT EXISTS idx_risk_strategy  ON risk_events (strategy_id, ts DESC);

CREATE TABLE IF NOT EXISTS system_events (
    id             TEXT PRIMARY KEY,
    ts             BIGINT NOT NULL,
    level          TEXT NOT NULL CHECK (level IN ('info','warn','error','critical')),
    subsystem      TEXT NOT NULL,
    message        TEXT NOT NULL,
    correlation_id TEXT NULL
);
CREATE INDEX IF NOT EXISTS idx_system_ts   ON system_events (ts DESC);
CREATE INDEX IF NOT EXISTS idx_system_level ON system_events (level, ts DESC);

CREATE TABLE IF NOT EXISTS backtest_runs (
    id              TEXT PRIMARY KEY,
    strategy_id     TEXT NOT NULL,
    strategy_version INT NOT NULL,
    config_json     TEXT NOT NULL,
    data_source     TEXT NOT NULL,
    interval_start  BIGINT NOT NULL,
    interval_end    BIGINT NOT NULL,
    fee_model       TEXT NOT NULL,
    slippage_bps    NUMERIC NOT NULL,
    status          TEXT NOT NULL CHECK (status IN ('queued','running','completed','failed')),
    metrics_json    TEXT NULL,
    net_return_pct  NUMERIC NULL,
    max_drawdown_pct NUMERIC NULL,
    win_rate_pct    NUMERIC NULL,
    trade_count     BIGINT NULL,
    profit_factor   NUMERIC NULL,
    started_at      BIGINT NULL,
    finished_at     BIGINT NULL,
    error           TEXT NULL
);
CREATE INDEX IF NOT EXISTS idx_backtests_strategy ON backtest_runs (strategy_id, started_at DESC);

CREATE TABLE IF NOT EXISTS settings (
    key        TEXT PRIMARY KEY,
    value      TEXT NOT NULL,
    updated_at BIGINT NOT NULL
);