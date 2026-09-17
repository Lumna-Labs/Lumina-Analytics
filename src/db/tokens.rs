use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use sqlx::PgPool;

use crate::models::{TokenSnapshotRow, TokenWithLatest};

pub async fn upsert_token<'e, E>(
    executor: E,
    asset_code: &str,
    asset_issuer: &str,
) -> anyhow::Result<()>
where
    E: sqlx::PgExecutor<'e>,
{
    sqlx::query(
        r#"
        INSERT INTO tokens (asset_code, asset_issuer)
        VALUES ($1, $2)
        ON CONFLICT (asset_code, asset_issuer) DO NOTHING
        "#,
    )
    .bind(asset_code)
    .bind(asset_issuer)
    .execute(executor)
    .await?;
    Ok(())
}

/// The `num_accounts` (holder count) of an asset's most recent snapshot, if
/// any — used to detect a sudden holder-count drop by comparing against the
/// snapshot about to be inserted (see `ingest_token` in `bin/ingest.rs`).
/// `None` for an asset seen for the first time this cycle, since there's
/// nothing to compare against yet.
pub async fn latest_token_num_accounts<'e, E>(
    executor: E,
    asset_code: &str,
    asset_issuer: &str,
) -> anyhow::Result<Option<i32>>
where
    E: sqlx::PgExecutor<'e>,
{
    let row: Option<(i32,)> = sqlx::query_as(
        r#"
        SELECT num_accounts
        FROM token_snapshots
        WHERE asset_code = $1 AND asset_issuer = $2
        ORDER BY time DESC
        LIMIT 1
        "#,
    )
    .bind(asset_code)
    .bind(asset_issuer)
    .fetch_optional(executor)
    .await?;
    Ok(row.map(|(v,)| v))
}

pub async fn insert_token_snapshot<'e, E>(
    executor: E,
    time: DateTime<Utc>,
    asset_code: &str,
    asset_issuer: &str,
    amount: Decimal,
    num_accounts: i32,
    num_claimable_balances: i32,
) -> anyhow::Result<()>
where
    E: sqlx::PgExecutor<'e>,
{
    sqlx::query(
        r#"
        INSERT INTO token_snapshots (time, asset_code, asset_issuer, amount, num_accounts, num_claimable_balances)
        VALUES ($1, $2, $3, $4, $5, $6)
        "#,
    )
    .bind(time)
    .bind(asset_code)
    .bind(asset_issuer)
    .bind(amount)
    .bind(num_accounts)
    .bind(num_claimable_balances)
    .execute(executor)
    .await?;
    Ok(())
}

pub async fn list_tokens_with_latest(
    pool: &PgPool,
    limit: i64,
    offset: i64,
) -> anyhow::Result<Vec<TokenWithLatest>> {
    let rows = sqlx::query_as::<_, TokenWithLatest>(
        r#"
        SELECT
            t.asset_code, t.asset_issuer,
            s.time, s.amount, s.num_accounts, s.num_claimable_balances
        FROM tokens t
        LEFT JOIN LATERAL (
            SELECT time, amount, num_accounts, num_claimable_balances
            FROM token_snapshots
            WHERE token_snapshots.asset_code = t.asset_code
              AND token_snapshots.asset_issuer = t.asset_issuer
            ORDER BY time DESC
            LIMIT 1
        ) s ON true
        ORDER BY s.num_accounts DESC NULLS LAST
        LIMIT $1 OFFSET $2
        "#,
    )
    .bind(limit)
    .bind(offset)
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

pub async fn count_tokens(pool: &PgPool) -> anyhow::Result<i64> {
    let (count,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM tokens")
        .fetch_one(pool)
        .await?;
    Ok(count)
}

pub async fn token_history(
    pool: &PgPool,
    asset_code: &str,
    asset_issuer: &str,
    since: DateTime<Utc>,
) -> anyhow::Result<Vec<TokenSnapshotRow>> {
    let rows = sqlx::query_as::<_, TokenSnapshotRow>(
        r#"
        SELECT time, amount, num_accounts, num_claimable_balances
        FROM token_snapshots
        WHERE asset_code = $1 AND asset_issuer = $2 AND time >= $3
        ORDER BY time ASC
        "#,
    )
    .bind(asset_code)
    .bind(asset_issuer)
    .bind(since)
    .fetch_all(pool)
    .await?;
    Ok(rows)
}
