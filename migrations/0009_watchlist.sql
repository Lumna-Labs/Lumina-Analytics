-- Pinned pools/tokens/lending positions for a dedicated at-a-glance view.
-- A single global list, not per-account: this project has no multi-user
-- auth, consistent with the rest of the dashboard being a shared read
-- surface over one deployment's indexed data.
CREATE TABLE watchlist_items (
    id         BIGSERIAL PRIMARY KEY,
    item_type  TEXT NOT NULL, -- 'pool' | 'token' | 'lending_position'
    item_key   TEXT NOT NULL, -- pool_id | "code:issuer" | "protocol:pool_contract:account"
    label      TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (item_type, item_key)
);
