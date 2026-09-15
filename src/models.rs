use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct PoolWithLatest {
    pub pool_id: String,
    pub asset_a: String,
    pub asset_b: String,
    pub fee_bp: i32,
    pub time: Option<DateTime<Utc>>,
    pub reserve_a: Option<Decimal>,
    pub reserve_b: Option<Decimal>,
    pub total_shares: Option<Decimal>,
    pub trustline_count: Option<i32>,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct PoolSnapshotRow {
    pub time: DateTime<Utc>,
    pub reserve_a: Decimal,
    pub reserve_b: Decimal,
    pub total_shares: Decimal,
    pub trustline_count: i32,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct TokenSnapshotRow {
    pub time: DateTime<Utc>,
    pub amount: Decimal,
    pub num_accounts: i32,
    pub num_claimable_balances: i32,
}

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct TokenWithLatest {
    pub asset_code: String,
    pub asset_issuer: String,
    pub time: Option<DateTime<Utc>>,
    pub amount: Option<Decimal>,
    pub num_accounts: Option<i32>,
    pub num_claimable_balances: Option<i32>,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct WhaleTransactionRow {
    pub time: DateTime<Utc>,
    pub tx_hash: String,
    pub source_account: String,
    pub dest_account: Option<String>,
    pub asset_code: String,
    pub asset_issuer: Option<String>,
    pub amount: Decimal,
    /// USD estimate, when the asset has a known price (see `pricing`); `None`
    /// if the asset has never traded against native XLM in a tracked pool.
    pub amount_usd: Option<Decimal>,
}

#[derive(Debug, sqlx::FromRow)]
pub struct TvlPointRaw {
    pub bucket: DateTime<Utc>,
    pub pool_count: i64,
    pub total_reserve_native: Decimal,
    pub total_reserve_usd_raw: Decimal,
    pub has_price: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TvlPoint {
    pub bucket: DateTime<Utc>,
    pub pool_count: i64,
    pub total_reserve_native: Decimal,
    /// `None` for buckets before the price feed had recorded an XLM/USD rate.
    pub total_reserve_usd: Option<Decimal>,
}

impl From<TvlPointRaw> for TvlPoint {
    fn from(r: TvlPointRaw) -> Self {
        Self {
            bucket: r.bucket,
            pool_count: r.pool_count,
            total_reserve_native: r.total_reserve_native,
            total_reserve_usd: r.has_price.then_some(r.total_reserve_usd_raw),
        }
    }
}

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct LendingPositionRow {
    pub time: DateTime<Utc>,
    pub protocol: String,
    pub pool_contract: String,
    pub account: String,
    pub collateral_asset: String,
    pub collateral_amount: Decimal,
    pub debt_asset: String,
    pub debt_amount: Decimal,
    pub ltv: Option<Decimal>,
    pub health_factor: Option<Decimal>,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct LiquidationRiskBucket {
    pub risk_level: String,
    pub position_count: i64,
    pub total_debt: Decimal,
}

#[derive(Debug, sqlx::FromRow)]
pub struct PoolTrendRaw {
    pub pool_id: String,
    pub asset_a: String,
    pub asset_b: String,
    pub fee_bp: i32,
    pub first_time: DateTime<Utc>,
    pub last_time: DateTime<Utc>,
    pub shares_before: Decimal,
    pub shares_now: Decimal,
    pub reserve_a_now: Decimal,
    pub reserve_b_now: Decimal,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct AlertRow {
    pub time: DateTime<Utc>,
    pub kind: String,
    pub severity: String,
    pub message: String,
    pub details: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct AlertChannelRow {
    pub id: i64,
    pub name: String,
    pub kind: String,
    pub url: String,
    pub min_severity: String,
    pub enabled: bool,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct AlertRuleRow {
    pub id: i64,
    pub name: String,
    pub rule_type: String,
    pub asset_code: Option<String>,
    pub asset_issuer: Option<String>,
    pub threshold: Decimal,
    pub enabled: bool,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct WatchlistItemRow {
    pub id: i64,
    pub item_type: String,
    pub item_key: String,
    pub label: String,
    pub created_at: DateTime<Utc>,
}

/// One hit from a global search across pools, tokens, and whale-payment
/// accounts (see `db::search`). `key` is the opaque identifier the frontend
/// uses to build the right detail-page link for `result_type`.
#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct SearchResultRow {
    pub result_type: String,
    pub title: String,
    pub subtitle: Option<String>,
    pub key: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PoolTrend {
    pub pool_id: String,
    pub asset_a: String,
    pub asset_b: String,
    pub fee_bp: i32,
    pub first_time: DateTime<Utc>,
    pub last_time: DateTime<Utc>,
    pub shares_before: Decimal,
    pub shares_now: Decimal,
    pub reserve_a_now: Decimal,
    pub reserve_b_now: Decimal,
    /// Percent change in pool total_shares over the window; None if the
    /// starting value was zero (undefined growth rate).
    pub shares_change_pct: Option<Decimal>,
}
