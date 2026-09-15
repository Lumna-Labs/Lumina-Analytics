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
