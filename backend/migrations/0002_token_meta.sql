-- 0002_token_meta.sql — Token metadata cache (untrusted external data).

CREATE TABLE IF NOT EXISTS token_meta (
    mint           TEXT PRIMARY KEY,
    symbol         TEXT NULL,
    name           TEXT NULL,
    decimals       INT NULL,
    first_seen_at  BIGINT NOT NULL,
    last_seen_at   BIGINT NOT NULL
);