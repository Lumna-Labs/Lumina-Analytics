-- Significant on-chain events worth surfacing proactively: outsized whale
-- payments and lending positions that cross into a higher liquidation-risk
-- band. Populated by the ingester regardless of whether ALERT_WEBHOOK_URL is
-- set, so the dashboard has something to show even with no webhook
-- configured; the webhook (when set) is just an additional delivery channel.
CREATE TABLE alerts (
    id         BIGSERIAL,
    time       TIMESTAMPTZ NOT NULL,
    kind       TEXT NOT NULL,
    severity   TEXT NOT NULL,
    message    TEXT NOT NULL,
    details    JSONB NOT NULL DEFAULT '{}',
    PRIMARY KEY (id, time)
);
SELECT create_hypertable('alerts', 'time');
CREATE INDEX ON alerts (time DESC);
CREATE INDEX ON alerts (severity, time DESC);
