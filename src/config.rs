use std::collections::HashMap;
use std::env;
use std::str::FromStr;

use rust_decimal::Decimal;

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
    /// Optional, comma-separated `contract_address:usd_price` pairs, operator-
    /// supplied. Blend identifies reserve assets by Soroban Asset Contract
    /// address, not the code/issuer pairs Horizon and CoinGecko use, so
    /// there's no automatic way to price them the way classic-network assets
    /// are priced. Left empty, `ltv`/`health_factor` stay `NULL` (an honest
    /// gap); populate it with real prices for your pool's reserves (from
    /// whatever oracle or spot source you trust) to turn on risk banding.
    /// This is a coarse, static override — not a live feed — by design: a
    /// wrong or spoofed price here would fabricate risk numbers, so this
    /// crate never fetches one automatically on your behalf.
    pub blend_asset_prices_usd_raw: String,
    /// Optional webhook URL (Slack-compatible incoming-webhook JSON: a POST
    /// body of `{"text": "..."}`) that gets notified for alert-worthy events
    /// (outsized whale payments, lending positions crossing into a higher
    /// risk band). Alerts are always recorded to the `alerts` table
    /// regardless of this setting; the webhook is an optional extra delivery
    /// channel, off by default so this crate never talks to a third-party
    /// endpoint without the operator opting in.
    pub alert_webhook_url: Option<String>,
    /// Minimum severity ("INFO", "WARNING", "CRITICAL") that triggers a
    /// webhook notification; every severity is still recorded to `alerts`.
    pub alert_min_severity: String,
    /// Requests allowed per second per client IP on the API, using a simple
    /// token-bucket. `0` disables rate limiting entirely (the default) —
    /// this is a defensive measure for a publicly exposed deployment, not a
    /// correctness requirement, so it fails open rather than risk blocking
    /// legitimate traffic in local/dev use.
    pub rate_limit_rps: u32,
    /// Burst capacity for the same token-bucket (max requests in a short
    /// spike before the per-second rate applies).
    pub rate_limit_burst: u32,
    /// Optional shared secret that gates the API's mutating endpoints (alert
    /// rules/channels, watchlist) behind an `X-Admin-Key` header. Unset by
    /// default so local/dev and single-operator deployments need no extra
    /// setup — this is a defensive measure for a publicly exposed
    /// deployment, not a correctness dependency, so it fails open (no key
    /// configured = no gate) rather than lock an operator out by accident.
    pub admin_api_key: Option<String>,
    /// Minimum percent drop in a pool's `total_shares` between consecutive
    /// ingest cycles that's worth alerting on. Pool liquidity fluctuates
    /// constantly in normal operation, so this is deliberately a "sudden,
    /// large" threshold rather than firing on any decrease — see
    /// `alerts::severity_for_liquidity_drop`.
    pub liquidity_drop_threshold_pct: Decimal,
    /// Minimum percent drop in a token's holder count (`num_accounts`)
    /// between consecutive ingest cycles that's worth alerting on. Same
    /// "sudden, large" rationale as `liquidity_drop_threshold_pct` — see
    /// `alerts::severity_for_holder_drop`.
    pub holder_drop_threshold_pct: Decimal,
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
            blend_asset_prices_usd_raw: env::var("BLEND_ASSET_PRICES_USD").unwrap_or_default(),
            alert_webhook_url: env::var("ALERT_WEBHOOK_URL").ok().filter(|s| !s.is_empty()),
            alert_min_severity: env::var("ALERT_MIN_SEVERITY")
                .unwrap_or_else(|_| "WARNING".to_string()),
            rate_limit_rps: env::var("RATE_LIMIT_RPS")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(0),
            rate_limit_burst: env::var("RATE_LIMIT_BURST")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(40),
            admin_api_key: env::var("ADMIN_API_KEY").ok().filter(|s| !s.is_empty()),
            liquidity_drop_threshold_pct: env::var("LIQUIDITY_DROP_THRESHOLD_PCT")
                .ok()
                .and_then(|v| Decimal::from_str(&v).ok())
                .unwrap_or(Decimal::from(30)),
            holder_drop_threshold_pct: env::var("HOLDER_DROP_THRESHOLD_PCT")
                .ok()
                .and_then(|v| Decimal::from_str(&v).ok())
                .unwrap_or(Decimal::from(20)),
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

    /// Parses `BLEND_ASSET_PRICES_USD` into a contract-address -> USD-price
    /// map. Malformed entries (bad decimal, missing `:`) are logged and
    /// skipped rather than failing startup over one typo.
    pub fn blend_asset_prices_usd(&self) -> HashMap<String, Decimal> {
        let mut out = HashMap::new();
        for entry in self.blend_asset_prices_usd_raw.split(',') {
            let entry = entry.trim();
            if entry.is_empty() {
                continue;
            }
            let Some((addr, price)) = entry.split_once(':') else {
                tracing::warn!("ignoring malformed BLEND_ASSET_PRICES_USD entry: {entry}");
                continue;
            };
            match Decimal::from_str(price.trim()) {
                Ok(price) => {
                    out.insert(addr.trim().to_string(), price);
                }
                Err(e) => {
                    tracing::warn!("ignoring malformed BLEND_ASSET_PRICES_USD entry {entry}: {e}")
                }
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_valid_blend_asset_prices() {
        let config = Config {
            blend_asset_prices_usd_raw: "CABC:1.00, CDEF:0.5001".to_string(),
            ..test_config()
        };
        let prices = config.blend_asset_prices_usd();
        assert_eq!(
            prices.get("CABC"),
            Some(&Decimal::from_str("1.00").unwrap())
        );
        assert_eq!(
            prices.get("CDEF"),
            Some(&Decimal::from_str("0.5001").unwrap())
        );
        assert_eq!(prices.len(), 2);
    }

    #[test]
    fn skips_malformed_blend_asset_price_entries() {
        let config = Config {
            blend_asset_prices_usd_raw: "CABC:1.00,not-a-pair,CDEF:oops,,".to_string(),
            ..test_config()
        };
        let prices = config.blend_asset_prices_usd();
        assert_eq!(prices.len(), 1);
        assert_eq!(
            prices.get("CABC"),
            Some(&Decimal::from_str("1.00").unwrap())
        );
    }

    #[test]
    fn empty_blend_asset_prices_is_empty_map() {
        let config = test_config();
        assert!(config.blend_asset_prices_usd().is_empty());
    }

    fn test_config() -> Config {
        Config {
            database_url: String::new(),
            horizon_url: String::new(),
            api_bind: String::new(),
            poll_interval_secs: 60,
            whale_threshold: 0.0,
            redis_url: None,
            cache_ttl_secs: 15,
            price_feed_enabled: false,
            coingecko_api_url: String::new(),
            soroban_rpc_url: String::new(),
            blend_pool_ids_raw: String::new(),
            blend_asset_prices_usd_raw: String::new(),
            alert_webhook_url: None,
            alert_min_severity: "WARNING".to_string(),
            rate_limit_rps: 0,
            rate_limit_burst: 40,
            admin_api_key: None,
            liquidity_drop_threshold_pct: Decimal::from(30),
            holder_drop_threshold_pct: Decimal::from(20),
        }
    }
}
