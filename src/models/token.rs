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
