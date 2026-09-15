//! CRUD for the two operator-editable alerting knobs backing the "Manage
//! rules & channels" panel: delivery channels and per-asset/per-band
//! threshold overrides. Kept together since they're both small, closely
//! related config tables with near-identical CRUD shapes.

use rust_decimal::Decimal;
use sqlx::PgPool;

use crate::models::{AlertChannelRow, AlertRuleRow};

// ---- alert channels ----

const ALERT_CHANNEL_COLUMNS: &str = "id, name, kind, url, min_severity, enabled, created_at";

pub async fn list_alert_channels(pool: &PgPool) -> anyhow::Result<Vec<AlertChannelRow>> {
    let rows = sqlx::query_as::<_, AlertChannelRow>(&format!(
        "SELECT {ALERT_CHANNEL_COLUMNS} FROM alert_channels ORDER BY id"
    ))
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

/// Channels actually eligible to receive a delivery. Split out from
/// `list_alert_channels` so the read path used on every alert doesn't have
/// to filter disabled rows itself.
pub async fn list_enabled_alert_channels(pool: &PgPool) -> anyhow::Result<Vec<AlertChannelRow>> {
    let rows = sqlx::query_as::<_, AlertChannelRow>(&format!(
        "SELECT {ALERT_CHANNEL_COLUMNS} FROM alert_channels WHERE enabled ORDER BY id"
    ))
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

pub async fn insert_alert_channel(
    pool: &PgPool,
    name: &str,
    kind: &str,
    url: &str,
    min_severity: &str,
    enabled: bool,
) -> anyhow::Result<AlertChannelRow> {
    let row = sqlx::query_as::<_, AlertChannelRow>(&format!(
        r#"
        INSERT INTO alert_channels (name, kind, url, min_severity, enabled)
        VALUES ($1, $2, $3, $4, $5)
        RETURNING {ALERT_CHANNEL_COLUMNS}
        "#
    ))
    .bind(name)
    .bind(kind)
    .bind(url)
    .bind(min_severity)
    .bind(enabled)
    .fetch_one(pool)
    .await?;
    Ok(row)
}

pub async fn update_alert_channel(
    pool: &PgPool,
    id: i64,
    min_severity: &str,
    enabled: bool,
) -> anyhow::Result<Option<AlertChannelRow>> {
    let row = sqlx::query_as::<_, AlertChannelRow>(&format!(
        r#"
        UPDATE alert_channels SET min_severity = $2, enabled = $3
        WHERE id = $1
        RETURNING {ALERT_CHANNEL_COLUMNS}
        "#
    ))
    .bind(id)
    .bind(min_severity)
    .bind(enabled)
    .fetch_optional(pool)
    .await?;
    Ok(row)
}

pub async fn delete_alert_channel(pool: &PgPool, id: i64) -> anyhow::Result<bool> {
    let result = sqlx::query("DELETE FROM alert_channels WHERE id = $1")
        .bind(id)
        .execute(pool)
        .await?;
    Ok(result.rows_affected() > 0)
}

// ---- alert rules ----

const ALERT_RULE_COLUMNS: &str =
    "id, name, rule_type, asset_code, asset_issuer, threshold, enabled, created_at";

pub async fn list_alert_rules(pool: &PgPool) -> anyhow::Result<Vec<AlertRuleRow>> {
    let rows = sqlx::query_as::<_, AlertRuleRow>(&format!(
        "SELECT {ALERT_RULE_COLUMNS} FROM alert_rules ORDER BY id"
    ))
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

#[allow(clippy::too_many_arguments)]
pub async fn insert_alert_rule(
    pool: &PgPool,
    name: &str,
    rule_type: &str,
    asset_code: Option<&str>,
    asset_issuer: Option<&str>,
    threshold: Decimal,
    enabled: bool,
) -> anyhow::Result<AlertRuleRow> {
    let row = sqlx::query_as::<_, AlertRuleRow>(&format!(
        r#"
        INSERT INTO alert_rules (name, rule_type, asset_code, asset_issuer, threshold, enabled)
        VALUES ($1, $2, $3, $4, $5, $6)
        RETURNING {ALERT_RULE_COLUMNS}
        "#
    ))
    .bind(name)
    .bind(rule_type)
    .bind(asset_code)
    .bind(asset_issuer)
    .bind(threshold)
    .bind(enabled)
    .fetch_one(pool)
    .await?;
    Ok(row)
}

pub async fn update_alert_rule(
    pool: &PgPool,
    id: i64,
    threshold: Decimal,
    enabled: bool,
) -> anyhow::Result<Option<AlertRuleRow>> {
    let row = sqlx::query_as::<_, AlertRuleRow>(&format!(
        r#"
        UPDATE alert_rules SET threshold = $2, enabled = $3
        WHERE id = $1
        RETURNING {ALERT_RULE_COLUMNS}
        "#
    ))
    .bind(id)
    .bind(threshold)
    .bind(enabled)
    .fetch_optional(pool)
    .await?;
    Ok(row)
}

pub async fn delete_alert_rule(pool: &PgPool, id: i64) -> anyhow::Result<bool> {
    let result = sqlx::query("DELETE FROM alert_rules WHERE id = $1")
        .bind(id)
        .execute(pool)
        .await?;
    Ok(result.rows_affected() > 0)
}
