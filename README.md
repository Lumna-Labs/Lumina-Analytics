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
| `GET /liquidations` | Blend lending positions + risk-bucket summary (empty/unpriced until `BLEND_POOL_IDS` and a price source are configured) |

## Known limitations (by design, not oversight)

This project favors an honest gap over an invented number — see inline comments at each spot below
for the reasoning:

- **USD pricing** only covers assets that trade directly against native XLM in a tracked classic
  pool; anything else is `null` rather than guessed via multi-hop routing.
- **Liquidation risk** (`ltv`/`health_factor`) is always `null`: Blend's on-chain events identify
  reserves by Soroban contract address, and there's no price feed keyed by contract address wired
  up yet. Collateral/debt *amounts* are real, decoded from live Blend events, once `BLEND_POOL_IDS`
  is set.
- **Blend event decoding is heuristic** (`src/soroban.rs`): this crate has no typed definition of
  Blend's own event schema, so it scans each event for an address and the largest `I128` rather
  than assuming a fixed field layout. Verify against a block explorer before trusting it in
  production; unrecognized event shapes are skipped and logged, not misparsed.
- **`BLEND_POOL_IDS` ships empty.** Unlike Horizon's pools (a first-party Stellar endpoint), Blend
  pool contracts are a third-party deployment with no single "list all pools" endpoint this project
  trusts itself to guess correctly.
