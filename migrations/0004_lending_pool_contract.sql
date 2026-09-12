-- Distinguishes positions across multiple configured Blend pools under the
-- same protocol ("blend"); previously only (protocol, account) was unique,
-- which would have collapsed the same account's positions in two different
-- pools into one.
ALTER TABLE lending_positions ADD COLUMN pool_contract TEXT NOT NULL DEFAULT '';
