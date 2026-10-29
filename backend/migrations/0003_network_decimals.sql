-- 0003_network_decimals.sql — Multi-network support + position-decimals fix.
--
-- Two changes:
--   1. Add a `network` (chain) dimension to market data and tokens so the
--      terminal can research and paper-trade tokens across many chains
--      (solana, ethereum, base, arbitrum, bsc, ...). DexScreener reports a
--      chain_id per pair; we persist it alongside the snapshot.
--   2. Fix a valuation bug: positions previously implied 9 decimals for every
--      token, mispricing non-9-decimal tokens (BONK=5, USDC=6). Now each
--      position records the token's actual decimals, and valuations use it.

ALTER TABLE market_snapshots
    ADD COLUMN IF NOT EXISTS network TEXT NOT NULL DEFAULT 'solana';

ALTER TABLE token_meta
    ADD COLUMN IF NOT EXISTS network TEXT NOT NULL DEFAULT 'solana';

ALTER TABLE paper_orders
    ADD COLUMN IF NOT EXISTS network TEXT NOT NULL DEFAULT 'solana';

ALTER TABLE paper_fills
    ADD COLUMN IF NOT EXISTS network TEXT NOT NULL DEFAULT 'solana';

ALTER TABLE strategy_signals
    ADD COLUMN IF NOT EXISTS network TEXT NOT NULL DEFAULT 'solana';

ALTER TABLE positions
    ADD COLUMN IF NOT EXISTS decimals INT NOT NULL DEFAULT 9;

CREATE INDEX IF NOT EXISTS idx_snapshots_network_ts
    ON market_snapshots (network, observed_at DESC);

CREATE INDEX IF NOT EXISTS idx_orders_network
    ON paper_orders (network, created_at DESC);
