use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

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
