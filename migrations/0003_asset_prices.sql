-- USD prices derived from XLM/USD (external feed) and, for issued assets,
-- classic-AMM pool reserve ratios against native XLM. Native XLM itself is
-- stored as asset_code='XLM', asset_issuer=''.
CREATE TABLE asset_prices (
    time         TIMESTAMPTZ NOT NULL,
    asset_code   TEXT NOT NULL,
    asset_issuer TEXT NOT NULL DEFAULT '',
    price_usd    NUMERIC NOT NULL,
    price_source TEXT NOT NULL
);
SELECT create_hypertable('asset_prices', 'time');
CREATE INDEX ON asset_prices (asset_code, asset_issuer, time DESC);
