-- Lets an operator mark an alert as acknowledged (seen/handled) from the
-- dashboard, so the Alerts page can be filtered down to what's still
-- outstanding instead of accumulating forever. NULL (the default) means
-- unacknowledged; every alert is inserted unacknowledged and stays that way
-- until explicitly acked.
ALTER TABLE alerts ADD COLUMN acknowledged_at TIMESTAMPTZ;

-- Powers "unacknowledged only" filtering without a full-table scan; alerts
-- are always queried most-recent-first, so time is included for ordering.
CREATE INDEX ON alerts (acknowledged_at, time DESC);
