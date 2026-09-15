use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::Serialize;

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
