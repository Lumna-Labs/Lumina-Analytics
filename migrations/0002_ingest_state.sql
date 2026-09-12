-- Generic key/value store for ingestion checkpoints (Horizon payment cursor,
-- per-contract Soroban event cursors, etc.) so restarts resume forward
-- instead of re-scanning or silently skipping the gap.
CREATE TABLE ingest_state (
    key        TEXT PRIMARY KEY,
    value      TEXT NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
