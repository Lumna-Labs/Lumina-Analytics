use std::env;

#[derive(Clone, Debug)]
pub struct Config {
    pub database_url: String,
    pub horizon_url: String,
    pub api_bind: String,
    pub poll_interval_secs: u64,
    /// Naive whale-payment threshold, expressed in whatever unit the asset's
    /// amount is denominated in. Good enough for an MVP; whale transactions
    /// are separately enriched with a USD estimate at read time when a price
    /// is known (see `pricing`).
    pub whale_threshold: f64,
    /// Optional Redis connection string used to cache hot read endpoints.
    /// When unset (or unreachable at runtime) the API falls back to serving
    /// straight from Postgres — caching is a performance optimization, not a
    /// correctness dependency.
    pub redis_url: Option<String>,
    /// TTL, in seconds, for cached API responses.
    pub cache_ttl_secs: u64,
    /// Whether to fetch an XLM/USD price each ingest cycle. Off by default
    /// makes local/offline dev deterministic; enable for real USD figures.
    pub price_feed_enabled: bool,
    pub coingecko_api_url: String,
    /// Soroban RPC endpoint used for the optional lending/liquidation
    /// ingester. The default points at the SDF-operated public mainnet RPC;
    /// override for testnet or a different provider.
    pub soroban_rpc_url: String,
    /// Comma-separated Soroban contract IDs of the Blend lending pools to
    /// index. Left empty by default: unlike Horizon, there's no single
    /// well-known "the pools" endpoint for a third-party Soroban protocol,
    /// so guessing addresses risks silently indexing the wrong (or stale)
    /// contracts. Populate with real, verified pool contract IDs for the
    /// network you're pointed at to turn on liquidation-risk ingestion.
    pub blend_pool_ids_raw: String,
}

impl Config {
    pub fn from_env() -> Self {
        let _ = dotenvy::dotenv();

        Self {
            database_url: env::var("DATABASE_URL")
                .unwrap_or_else(|_| "postgres://lumina:lumina@localhost:5432/lumina".into()),
            horizon_url: env::var("HORIZON_URL")
                .unwrap_or_else(|_| "https://horizon.stellar.org".into()),
            api_bind: env::var("API_BIND").unwrap_or_else(|_| "0.0.0.0:8080".into()),
            poll_interval_secs: env::var("POLL_INTERVAL_SECS")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(60),
            whale_threshold: env::var("WHALE_THRESHOLD")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(10_000.0),
            redis_url: env::var("REDIS_URL").ok().filter(|s| !s.is_empty()),
            cache_ttl_secs: env::var("CACHE_TTL_SECS")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(15),
            price_feed_enabled: env::var("PRICE_FEED_ENABLED")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(false),
            coingecko_api_url: env::var("COINGECKO_API_URL")
                .unwrap_or_else(|_| "https://api.coingecko.com/api/v3/simple/price".into()),
            soroban_rpc_url: env::var("SOROBAN_RPC_URL")
                .unwrap_or_else(|_| "https://mainnet.sorobanrpc.com".into()),
            blend_pool_ids_raw: env::var("BLEND_POOL_IDS").unwrap_or_default(),
        }
    }

    pub fn blend_pool_ids(&self) -> Vec<String> {
        self.blend_pool_ids_raw
            .split(',')
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_string)
            .collect()
    }
}
