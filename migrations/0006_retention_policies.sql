-- pool_snapshots and token_snapshots grow at a fixed rate every poll cycle
-- (default 60s) for as long as the ingester runs, with no natural cap the
-- way event-driven tables (whale_transactions, alerts, lending_positions)
-- have. Left alone, raw snapshot storage grows without bound. A 180-day
-- retention window keeps ~6 months of full-resolution history, which is
-- generous for the trend/history windows this API exposes (max 30d in the
-- frontend) while preventing unbounded disk growth.
--
-- This is a default, not a mandate: adjust or drop the policy for your own
-- retention needs, e.g.:
--   SELECT remove_retention_policy('pool_snapshots');
--   SELECT add_retention_policy('pool_snapshots', INTERVAL '2 years');
SELECT add_retention_policy('pool_snapshots', INTERVAL '180 days');
SELECT add_retention_policy('token_snapshots', INTERVAL '180 days');
