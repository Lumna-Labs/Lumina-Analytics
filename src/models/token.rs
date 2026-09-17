use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

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

#[derive(Debug, sqlx::FromRow)]
pub struct TokenTrendRaw {
    pub asset_code: String,
    pub asset_issuer: String,
    pub first_time: DateTime<Utc>,
    pub last_time: DateTime<Utc>,
    pub amount_before: Decimal,
    pub amount_now: Decimal,
    pub holders_before: i32,
    pub holders_now: i32,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TokenTrend {
    pub asset_code: String,
    pub asset_issuer: String,
    pub first_time: DateTime<Utc>,
    pub last_time: DateTime<Utc>,
    pub amount_before: Decimal,
    pub amount_now: Decimal,
    pub holders_before: i32,
    pub holders_now: i32,
    /// Percent change in holder count over the window; None if the starting
    /// count was zero (undefined growth rate).
    pub holders_change_pct: Option<Decimal>,
}
