use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use sqlx::PgPool;

use crate::models::{PoolSnapshotRow, PoolTrendRaw, PoolWithLatest, TvlPoint, TvlPointRaw};

// These take a generic `PgExecutor` (either `&PgPool` or `&mut PgConnection`
// from an open `Transaction`) rather than a plain `&PgPool`, so a whole
// ingest cycle's writes can be batched into a single transaction. That's not
// just a performance nicety: without it, e.g. a crash between writing a
// batch of payments and persisting the new cursor would replay already-
// recorded rows into the append-only *_snapshots tables on the next cycle,
// since those tables have no dedup constraint (unlike pools/tokens/whale_tx,
// which are idempotent via ON CONFLICT).

pub async fn upsert_pool<'e, E>(
    executor: E,
    pool_id: &str,
    asset_a: &str,
    asset_b: &str,
    fee_bp: i32,
) -> anyhow::Result<()>
where
    E: sqlx::PgExecutor<'e>,
{
    sqlx::query(
        r#"
        INSERT INTO pools (pool_id, asset_a, asset_b, fee_bp)
        VALUES ($1, $2, $3, $4)
        ON CONFLICT (pool_id) DO NOTHING
        "#,
    )
    .bind(pool_id)
    .bind(asset_a)
    .bind(asset_b)
    .bind(fee_bp)
    .execute(executor)
    .await?;
    Ok(())
}

pub async fn insert_pool_snapshot<'e, E>(
    executor: E,
    time: DateTime<Utc>,
    pool_id: &str,
    reserve_a: Decimal,
    reserve_b: Decimal,
    total_shares: Decimal,
    trustline_count: i32,
) -> anyhow::Result<()>
where
    E: sqlx::PgExecutor<'e>,
{
    sqlx::query(
        r#"
        INSERT INTO pool_snapshots (time, pool_id, reserve_a, reserve_b, total_shares, trustline_count)
        VALUES ($1, $2, $3, $4, $5, $6)
        "#,
    )
    .bind(time)
    .bind(pool_id)
    .bind(reserve_a)
    .bind(reserve_b)
    .bind(total_shares)
    .bind(trustline_count)
    .execute(executor)
    .await?;
    Ok(())
}

pub async fn list_pools_with_latest(
    pool: &PgPool,
    limit: i64,
    offset: i64,
) -> anyhow::Result<Vec<PoolWithLatest>> {
    let rows = sqlx::query_as::<_, PoolWithLatest>(
        r#"
        SELECT
            p.pool_id, p.asset_a, p.asset_b, p.fee_bp,
            s.time, s.reserve_a, s.reserve_b, s.total_shares, s.trustline_count
        FROM pools p
        LEFT JOIN LATERAL (
            SELECT time, reserve_a, reserve_b, total_shares, trustline_count
            FROM pool_snapshots
            WHERE pool_snapshots.pool_id = p.pool_id
            ORDER BY time DESC
            LIMIT 1
        ) s ON true
        ORDER BY s.total_shares DESC NULLS LAST
        LIMIT $1 OFFSET $2
        "#,
    )
    .bind(limit)
    .bind(offset)
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

pub async fn count_pools(pool: &PgPool) -> anyhow::Result<i64> {
    let (count,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM pools")
        .fetch_one(pool)
        .await?;
    Ok(count)
}

pub async fn pool_history(
    pool: &PgPool,
    pool_id: &str,
    since: DateTime<Utc>,
) -> anyhow::Result<Vec<PoolSnapshotRow>> {
    let rows = sqlx::query_as::<_, PoolSnapshotRow>(
        r#"
        SELECT time, reserve_a, reserve_b, total_shares, trustline_count
        FROM pool_snapshots
        WHERE pool_id = $1 AND time >= $2
        ORDER BY time ASC
        "#,
    )
    .bind(pool_id)
    .bind(since)
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

/// Growth in each pool's total_shares between the first and last snapshot
/// observed within the window, used to surface "trending" pools.
pub async fn pool_trends(pool: &PgPool, since: DateTime<Utc>) -> anyhow::Result<Vec<PoolTrendRaw>> {
    let rows = sqlx::query_as::<_, PoolTrendRaw>(
        r#"
        WITH earliest AS (
            SELECT DISTINCT ON (pool_id) pool_id, time, reserve_a, reserve_b, total_shares
            FROM pool_snapshots
            WHERE time >= $1
            ORDER BY pool_id, time ASC
        ),
        latest AS (
            SELECT DISTINCT ON (pool_id) pool_id, time, reserve_a, reserve_b, total_shares
            FROM pool_snapshots
            WHERE time >= $1
            ORDER BY pool_id, time DESC
        )
        SELECT
            p.pool_id, p.asset_a, p.asset_b, p.fee_bp,
            e.time AS first_time, l.time AS last_time,
            e.total_shares AS shares_before, l.total_shares AS shares_now,
            l.reserve_a AS reserve_a_now, l.reserve_b AS reserve_b_now
        FROM pools p
        JOIN earliest e ON e.pool_id = p.pool_id
        JOIN latest l ON l.pool_id = p.pool_id
        WHERE l.time > e.time
        "#,
    )
    .bind(since)
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

/// TVL: total native-XLM reserves locked across all pools, bucketed hourly,
/// plus a USD figure derived from the XLM/USD price recorded nearest (at or
/// before) each snapshot. `total_reserve_usd` is `None` for buckets that
/// predate the price feed being enabled.
pub async fn tvl_series(pool: &PgPool, since: DateTime<Utc>) -> anyhow::Result<Vec<TvlPoint>> {
    let rows = sqlx::query_as::<_, TvlPointRaw>(
        r#"
        SELECT
            time_bucket('1 hour', time) AS bucket,
            COUNT(DISTINCT pool_id) AS pool_count,
            SUM(native_amount) AS total_reserve_native,
            SUM(native_amount * COALESCE(price_usd, 0)) AS total_reserve_usd_raw,
            BOOL_OR(price_usd IS NOT NULL) AS has_price
        FROM (
            SELECT
                pool_snapshots.time, pool_snapshots.pool_id,
                (CASE WHEN pa.asset_a = 'native' THEN reserve_a ELSE 0 END) +
                (CASE WHEN pa.asset_b = 'native' THEN reserve_b ELSE 0 END) AS native_amount,
                xp.price_usd
            FROM pool_snapshots
            JOIN pools pa ON pa.pool_id = pool_snapshots.pool_id
            LEFT JOIN LATERAL (
                SELECT price_usd
                FROM asset_prices
                WHERE asset_code = 'XLM' AND asset_issuer = '' AND time <= pool_snapshots.time
                ORDER BY time DESC
                LIMIT 1
            ) xp ON true
            WHERE pool_snapshots.time >= $1
        ) x
        GROUP BY bucket
        ORDER BY bucket ASC
        "#,
    )
    .bind(since)
    .fetch_all(pool)
    .await?;
    Ok(rows.into_iter().map(TvlPoint::from).collect())
}
