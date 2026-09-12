CREATE EXTENSION IF NOT EXISTS timescaledb;

-- Liquidity pools (dimension table)
CREATE TABLE pools (
    pool_id     TEXT PRIMARY KEY,
    asset_a     TEXT NOT NULL,
    asset_b     TEXT NOT NULL,
    fee_bp      INT NOT NULL,
    first_seen  TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- Pool state over time (60s snapshots)
CREATE TABLE pool_snapshots (
    time             TIMESTAMPTZ NOT NULL,
    pool_id          TEXT NOT NULL REFERENCES pools(pool_id),
    reserve_a        NUMERIC NOT NULL,
    reserve_b        NUMERIC NOT NULL,
    total_shares     NUMERIC NOT NULL,
    trustline_count  INT NOT NULL
);
SELECT create_hypertable('pool_snapshots', 'time');
CREATE INDEX ON pool_snapshots (pool_id, time DESC);

-- Assets (dimension table)
CREATE TABLE tokens (
    asset_code   TEXT NOT NULL,
    asset_issuer TEXT NOT NULL,
    PRIMARY KEY (asset_code, asset_issuer)
);

-- Asset supply/holder state over time
CREATE TABLE token_snapshots (
    time                    TIMESTAMPTZ NOT NULL,
    asset_code              TEXT NOT NULL,
    asset_issuer            TEXT NOT NULL,
    amount                  NUMERIC NOT NULL,
    num_accounts            INT NOT NULL,
    num_claimable_balances  INT NOT NULL DEFAULT 0
);
SELECT create_hypertable('token_snapshots', 'time');
CREATE INDEX ON token_snapshots (asset_code, asset_issuer, time DESC);

-- Large ("whale") payments observed on the network
CREATE TABLE whale_transactions (
    id             BIGSERIAL,
    time           TIMESTAMPTZ NOT NULL,
    op_id          TEXT NOT NULL,
    tx_hash        TEXT NOT NULL,
    source_account TEXT NOT NULL,
    dest_account   TEXT,
    asset_code     TEXT NOT NULL,
    asset_issuer   TEXT,
    amount         NUMERIC NOT NULL,
    PRIMARY KEY (id, time)
);
SELECT create_hypertable('whale_transactions', 'time');
CREATE UNIQUE INDEX ON whale_transactions (op_id, time);
CREATE INDEX ON whale_transactions (time DESC);

-- Reserved for the lending/liquidation dashboard (Week 2+): ingestion for this
-- table requires parsing a specific Soroban lending protocol (e.g. Blend) via
-- Soroban RPC and is not implemented yet. Schema is here so the API/frontend
-- can be built against a stable shape.
CREATE TABLE lending_positions (
    time              TIMESTAMPTZ NOT NULL,
    protocol          TEXT NOT NULL,
    account           TEXT NOT NULL,
    collateral_asset  TEXT NOT NULL,
    collateral_amount NUMERIC NOT NULL,
    debt_asset        TEXT NOT NULL,
    debt_amount       NUMERIC NOT NULL,
    ltv               NUMERIC,
    health_factor     NUMERIC
);
SELECT create_hypertable('lending_positions', 'time');
