use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::Serialize;

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
