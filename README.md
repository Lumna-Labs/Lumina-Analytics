# Lumina Analytics

Real-time analytics for the Stellar network: classic-AMM liquidity pools, issued-asset supply/holder
counts, large ("whale") payments, and — when configured — Blend lending-pool liquidation risk.
Backend in Rust (Axum + TimescaleDB), frontend in React.

## Architecture

```
                 ┌──────────────────┐
                 │  Stellar Horizon │  (public REST API)
                 └────────┬─────────┘
                          │ polls every POLL_INTERVAL_SECS
                          ▼
┌─────────────┐   ┌───────────────┐   ┌──────────────┐
│ Soroban RPC │──▶│ lumina-ingest │──▶│ TimescaleDB  │
│ (optional,  │   │  (Rust)       │   │ (Postgres +  │
│  Blend)     │   └───────────────┘   │  hypertables)│
└─────────────┘                       └──────┬───────┘
                                              │
                    ┌─────────────────────────┘
                    ▼
             ┌─────────────┐        ┌──────────┐
             │ lumina-api  │◀──────▶│  Redis   │  (optional response cache)
             │  (Axum)     │        └──────────┘
             └──────┬──────┘
                    │ REST + JSON
                    ▼
             ┌─────────────┐
             │  frontend   │  (React + Vite + Recharts)
             └─────────────┘
```

Two binaries share one crate:

- **`lumina-ingest`** polls Horizon on a timer, writes pool/token/whale-payment snapshots into
  TimescaleDB, and (if enabled) fetches an XLM/USD price and Blend Soroban events.
- **`lumina-api`** serves read-only REST endpoints over that data, optionally cached in Redis.

Redis and a USD price feed are both optional and fail open: if either is unreachable or disabled,
the API and ingester keep working from Postgres alone (Redis absence just means every request hits
Postgres directly; no price feed means USD fields are `null`, never fabricated).

## Quickstart (Docker)

```bash
cp .env.example .env      # adjust if you want a price feed or lending ingestion
docker compose up --build
```

- Frontend: http://localhost:5173
- API: http://localhost:8080 (try `/health`, `/pools`, `/tvl`)
- Postgres: `localhost:5432` (`lumina`/`lumina`)

## Local development (without Docker)

Requires Rust (stable), Node 20+, and a running Postgres/TimescaleDB + optionally Redis:

```bash
docker compose up -d timescaledb redis   # just the datastores
cp .env.example .env

cargo run --bin lumina-ingest   # in one terminal
cargo run --bin lumina-api      # in another

cd frontend
npm install
npm run dev                     # http://localhost:5173
```

The API and ingester both run `sqlx::migrate!` on startup, so migrations in `migrations/` apply
automatically — no separate migration step needed.

## Testing

```bash
cargo test                      # Rust unit tests (pure decision logic, no DB required)
cargo clippy --all-targets --all-features -- -D warnings
cargo fmt --all -- --check

cd frontend
npm run test                    # Vitest, formatting/display-logic unit tests
npm run lint
npm run build                   # type-checks + production build
```

CI (`.github/workflows/ci.yml`) runs all of the above on every push/PR.

## Configuration

All variables live in `.env` (see `.env.example`); every one has a sane default except where noted.

| Variable | Default | Notes |
|---|---|---|
| `DATABASE_URL` | `postgres://lumina:lumina@localhost:5432/lumina` | |
| `HORIZON_URL` | `https://horizon.stellar.org` | Point at a testnet Horizon to index testnet instead |
| `API_BIND` | `0.0.0.0:8080` | |
| `POLL_INTERVAL_SECS` | `60` | Ingest cycle interval |
| `WHALE_THRESHOLD` | `10000` | In the payment asset's own native units |
| `REDIS_URL` | unset | API response caching; unset = no caching, not an error |
| `CACHE_TTL_SECS` | `15` | |
| `PRICE_FEED_ENABLED` | `false` | Fetches XLM/USD from CoinGecko each ingest cycle |
| `COINGECKO_API_URL` | CoinGecko's public `simple/price` endpoint | |
| `SOROBAN_RPC_URL` | SDF's public mainnet RPC | Override for testnet or a different provider |
| `BLEND_POOL_IDS` | empty | Comma-separated Blend pool contract IDs; empty = lending ingestion off |
| `BLEND_ASSET_PRICES_USD` | empty | Comma-separated `contract:price` overrides; turns on `ltv`/`health_factor` for those reserves |
| `ALERT_WEBHOOK_URL` | unset | Slack-compatible webhook for whale/risk alerts; alerts are recorded either way |
| `ALERT_MIN_SEVERITY` | `WARNING` | Minimum severity (`INFO`/`WARNING`/`CRITICAL`) that triggers the webhook |
| `RATE_LIMIT_RPS` | `0` (disabled) | Per-client-IP requests/sec on the API; `0` disables rate limiting |
| `RATE_LIMIT_BURST` | `40` | Token-bucket burst capacity when rate limiting is enabled |

## API

All responses are JSON; Decimal/NUMERIC fields are serialized as strings to avoid float precision
loss.

| Endpoint | Description |
|---|---|
| `GET /health` | Liveness check |
| `GET /tvl?hours=24` | Hourly native-XLM reserve totals across pools, plus USD when priced |
| `GET /pools` | All tracked pools with their latest snapshot |
| `GET /pools/:pool_id/history?hours=24` | One pool's snapshot history |
| `GET /pools/trending?hours=24` | Pools ranked by `total_shares` growth over the window |
| `GET /tokens` | All tracked issued assets with their latest snapshot |
| `GET /tokens/:asset_code/:asset_issuer/history?hours=24` | One asset's snapshot history |
| `GET /transactions/whales?min_amount=10000&limit=100` | Large payments, enriched with a USD estimate where known |
| `GET /liquidations` | Blend lending positions + risk-bucket summary (empty until `BLEND_POOL_IDS` is configured; `ltv`/`health_factor` stay `null` until `BLEND_ASSET_PRICES_USD` is too) |
| `GET /alerts?limit=100` | Recently detected alert-worthy events (outsized whale payments, lending positions crossing into a higher risk band) |
| `GET /metrics` | Prometheus-format request counters and cumulative latency, by route |

`/pools` and `/tokens` accept `limit` (default 500, max 2000) and `offset` for pagination.

## Alerting

`lumina-ingest` detects two kinds of alert-worthy events and always records them to the `alerts`
table (visible on the dashboard's Alerts page), independent of any external configuration:

- **Outsized whale payments** — every newly-recorded whale payment, severity-banded by how many
  multiples of `WHALE_THRESHOLD` it cleared (≥5x = `WARNING`, ≥20x = `CRITICAL`, otherwise `INFO`).
- **Lending risk escalation** — a Blend position crossing *upward* into `HIGH` or `CRITICAL` risk
  (only possible once `BLEND_ASSET_PRICES_USD` makes `ltv` computable for that position).

Set `ALERT_WEBHOOK_URL` to also push `WARNING`+ alerts (configurable via `ALERT_MIN_SEVERITY`) to a
Slack-compatible incoming webhook. Webhook delivery is best-effort: a failure is logged and never
affects ingestion or the recorded alert.

## Operational hardening

- **Rate limiting**: set `RATE_LIMIT_RPS` (and optionally `RATE_LIMIT_BURST`) to cap requests per
  client IP with a simple in-process token bucket. Disabled (`0`) by default.
- **Metrics**: `GET /metrics` exposes per-route request counts and cumulative latency in
  Prometheus text format — point a Prometheus scrape config at it.
- **Retention**: `pool_snapshots` and `token_snapshots` (the two tables that grow every poll cycle
  indefinitely) have a 180-day TimescaleDB retention policy by default (see
  `migrations/0006_retention_policies.sql` for how to change or remove it). Event-driven tables
  (`whale_transactions`, `lending_positions`, `alerts`) aren't pruned automatically.

## Known limitations (by design, not oversight)

This project favors an honest gap over an invented number — see inline comments at each spot below
for the reasoning:

- **USD pricing** covers every asset reachable from native XLM through a chain of tracked classic
  pools (direct XLM pairs, plus assets only paired with an already-priced asset, up to 4 hops —
  see `pricing::price_from_known_leg`). An asset with no pool-reserve path back to XLM stays `null`
  rather than guessed.
- **Liquidation risk** (`ltv`/`health_factor`) is `null` unless `BLEND_ASSET_PRICES_USD` supplies a
  USD price for both the position's collateral and debt reserve (identified by Soroban contract
  address, which no automatic feed in this crate covers — see `.env.example`). Collateral/debt
  *amounts* are always real, decoded from live Blend events, once `BLEND_POOL_IDS` is set. Even
  with prices configured, `health_factor` here is a simplified collateral/debt coverage ratio, not
  Blend's own protocol formula (which additionally weights each reserve by its own risk
  parameters) — see `logic::compute_lending_risk`.
- **Blend event decoding is heuristic** (`src/soroban.rs`): this crate has no typed definition of
  Blend's own event schema, so it scans each event for an address and the largest `I128` rather
  than assuming a fixed field layout. Verify against a block explorer before trusting it in
  production; unrecognized event shapes are skipped and logged, not misparsed.
- **`BLEND_POOL_IDS` ships empty.** Unlike Horizon's pools (a first-party Stellar endpoint), Blend
  pool contracts are a third-party deployment with no single "list all pools" endpoint this project
  trusts itself to guess correctly.
