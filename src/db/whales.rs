use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use sqlx::PgPool;

use crate::models::WhaleTransactionRow;

/// Returns `true` if a new row was inserted, `false` if this payment
/// (identified by `op_id`+`time`) had already been recorded — used by the
/// caller to alert on genuinely new whale payments only, not on cursor
/// replays.
#[allow(clippy::too_many_arguments)]
pub async fn insert_whale_transaction<'e, E>(
    executor: E,
    time: DateTime<Utc>,
    op_id: &str,
    tx_hash: &str,
    source_account: &str,
    dest_account: Option<&str>,
    asset_code: &str,
    asset_issuer: Option<&str>,
    amount: Decimal,
) -> anyhow::Result<bool>
where
    E: sqlx::PgExecutor<'e>,
{
    let result = sqlx::query(
        r#"
        INSERT INTO whale_transactions
            (time, op_id, tx_hash, source_account, dest_account, asset_code, asset_issuer, amount)
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
        ON CONFLICT (op_id, time) DO NOTHING
        "#,
    )
    .bind(time)
    .bind(op_id)
    .bind(tx_hash)
    .bind(source_account)
    .bind(dest_account)
    .bind(asset_code)
    .bind(asset_issuer)
    .bind(amount)
    .execute(executor)
    .await?;
    Ok(result.rows_affected() > 0)
}

/// Lists whale payments at or above `min_amount`, most recent first,
/// optionally restricted to those where `account` was the source or
/// destination — powers both the Whale Tracker page (`account: None`) and
/// the per-account Activity page (see `/accounts/:address`).
pub async fn list_whale_transactions(
    pool: &PgPool,
    min_amount: Decimal,
    limit: i64,
    account: Option<&str>,
) -> anyhow::Result<Vec<WhaleTransactionRow>> {
    let rows = sqlx::query_as::<_, WhaleTransactionRow>(
        r#"
        SELECT
            wt.time, wt.tx_hash, wt.source_account, wt.dest_account,
            wt.asset_code, wt.asset_issuer, wt.amount,
            (wt.amount * ap.price_usd) AS amount_usd
        FROM whale_transactions wt
        LEFT JOIN LATERAL (
            SELECT price_usd
            FROM asset_prices
            WHERE asset_prices.asset_code = wt.asset_code
              AND asset_prices.asset_issuer = COALESCE(wt.asset_issuer, '')
              AND asset_prices.time <= wt.time
            ORDER BY time DESC
            LIMIT 1
        ) ap ON true
        WHERE wt.amount >= $1
          AND ($3::text IS NULL OR wt.source_account = $3 OR wt.dest_account = $3)
        ORDER BY wt.time DESC
        LIMIT $2
        "#,
    )
    .bind(min_amount)
    .bind(limit)
    .bind(account)
    .fetch_all(pool)
    .await?;
    Ok(rows)
}
