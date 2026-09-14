-- Named alert delivery destinations beyond the single legacy
-- ALERT_WEBHOOK_URL env var: any number of Slack/Discord/generic-webhook
-- endpoints, each with its own minimum-severity gate, managed via the
-- /alert-channels API instead of a redeploy. The env var keeps working
-- unchanged (delivered in addition to whatever's configured here), so an
-- existing deployment doesn't need to migrate anything to keep alerting.
CREATE TABLE alert_channels (
    id           BIGSERIAL PRIMARY KEY,
    name         TEXT NOT NULL UNIQUE,
    kind         TEXT NOT NULL, -- 'slack' | 'discord' | 'generic'
    url          TEXT NOT NULL,
    min_severity TEXT NOT NULL DEFAULT 'WARNING',
    enabled      BOOLEAN NOT NULL DEFAULT true,
    created_at   TIMESTAMPTZ NOT NULL DEFAULT now()
);
