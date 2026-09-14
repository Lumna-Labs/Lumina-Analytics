-- Operator-configurable overrides layered on top of the global
-- WHALE_THRESHOLD env var and the hardcoded LTV risk bands
-- (previously logic::risk_level's fixed 70/85/95 cutoffs):
--
--   'whale_threshold' rules override the whale-payment threshold for one
--   specific asset (asset_code + asset_issuer, NULL issuer = native XLM);
--   an asset with no matching rule keeps using WHALE_THRESHOLD.
--
--   'ltv_band' rules override one named risk band's percentage cutoff —
--   `name` must be 'MEDIUM', 'HIGH', or 'CRITICAL' — so an operator can tune
--   liquidation-risk banding without a code change.
--
-- Managed via the /alert-rules API rather than requiring a redeploy.
CREATE TABLE alert_rules (
    id           BIGSERIAL PRIMARY KEY,
    name         TEXT NOT NULL,
    rule_type    TEXT NOT NULL, -- 'whale_threshold' | 'ltv_band'
    asset_code   TEXT,
    asset_issuer TEXT,
    threshold    NUMERIC NOT NULL,
    enabled      BOOLEAN NOT NULL DEFAULT true,
    created_at   TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX ON alert_rules (rule_type, enabled);
