use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use sqlx::PgPool;

use crate::models::{LendingPositionRow, LiquidationRiskBucket};

/// Latest known state (by protocol+pool+account) for every lending position
/// ever observed. Populated once a lending ingester (currently: the optional
/// Blend/Soroban event ingester, gated on `BLEND_POOL_IDS`) writes to
/// `lending_positions`; empty schema-ready-but-dataless otherwise.
pub async fn list_lending_positions(pool: &PgPool) -> anyhow::Result<Vec<LendingPositionRow>> {
    let rows = sqlx::query_as::<_, LendingPositionRow>(
        r#"
        SELECT DISTINCT ON (protocol, pool_contract, account)
            time, protocol, pool_contract, account, collateral_asset, collateral_amount,
            debt_asset, debt_amount, ltv, health_factor
        FROM lending_positions
        ORDER BY protocol, pool_contract, account, time DESC
        "#,
    )
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

/// The most recent snapshot for one specific (protocol, pool, account)
/// triple, used by the lending ingester to compute the next running total
/// when a new on-chain event arrives.
pub async fn latest_lending_position(
    pool: &PgPool,
    protocol: &str,
    pool_contract: &str,
    account: &str,
) -> anyhow::Result<Option<LendingPositionRow>> {
    let row = sqlx::query_as::<_, LendingPositionRow>(
        r#"
        SELECT time, protocol, pool_contract, account, collateral_asset, collateral_amount,
               debt_asset, debt_amount, ltv, health_factor
        FROM lending_positions
        WHERE protocol = $1 AND pool_contract = $2 AND account = $3
        ORDER BY time DESC
        LIMIT 1
        "#,
    )
    .bind(protocol)
    .bind(pool_contract)
    .bind(account)
    .fetch_optional(pool)
    .await?;
    Ok(row)
}

#[allow(clippy::too_many_arguments)]
pub async fn insert_lending_position(
    pool: &PgPool,
    time: DateTime<Utc>,
    protocol: &str,
    pool_contract: &str,
    account: &str,
    collateral_asset: &str,
    collateral_amount: Decimal,
    debt_asset: &str,
    debt_amount: Decimal,
    ltv: Option<Decimal>,
    health_factor: Option<Decimal>,
) -> anyhow::Result<()> {
    sqlx::query(
        r#"
        INSERT INTO lending_positions
            (time, protocol, pool_contract, account, collateral_asset, collateral_amount,
             debt_asset, debt_amount, ltv, health_factor)
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
        "#,
    )
    .bind(time)
    .bind(protocol)
    .bind(pool_contract)
    .bind(account)
    .bind(collateral_asset)
    .bind(collateral_amount)
    .bind(debt_asset)
    .bind(debt_amount)
    .bind(ltv)
    .bind(health_factor)
    .execute(pool)
    .await?;
    Ok(())
}

/// Position count and total debt bucketed by LTV risk band, using each
/// account's latest snapshot. Empty until lending-protocol ingestion exists.
/// Band cutoffs are passed in (see `alert_rules::resolve_ltv_bands`) rather
/// than hardcoded, so an operator's `ltv_band` alert rules apply here too —
/// the dashboard's risk buckets and the alerting that escalates on them stay
/// in sync.
pub async fn liquidation_risk_summary(
    pool: &PgPool,
    medium: Decimal,
    high: Decimal,
    critical: Decimal,
) -> anyhow::Result<Vec<LiquidationRiskBucket>> {
    let rows = sqlx::query_as::<_, LiquidationRiskBucket>(
        r#"
        WITH latest AS (
            SELECT DISTINCT ON (protocol, pool_contract, account) account, protocol, debt_amount, ltv
            FROM lending_positions
            ORDER BY protocol, pool_contract, account, time DESC
        )
        SELECT
            CASE
                WHEN ltv >= $3 THEN 'CRITICAL'
                WHEN ltv >= $2 THEN 'HIGH'
                WHEN ltv >= $1 THEN 'MEDIUM'
                ELSE 'LOW'
            END AS risk_level,
            COUNT(*) AS position_count,
            SUM(debt_amount) AS total_debt
        FROM latest
        WHERE ltv IS NOT NULL
        GROUP BY risk_level
        ORDER BY risk_level
        "#,
    )
    .bind(medium)
    .bind(high)
    .bind(critical)
    .fetch_all(pool)
    .await?;
    Ok(rows)
}
