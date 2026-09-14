use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use sqlx::postgres::PgPoolOptions;
use sqlx::PgPool;

use crate::models::{
    AlertChannelRow, AlertRow, AlertRuleRow, LendingPositionRow, LiquidationRiskBucket,
    PoolSnapshotRow, PoolTrendRaw, PoolWithLatest, TokenSnapshotRow, TokenWithLatest, TvlPoint,
    TvlPointRaw, WatchlistItemRow, WhaleTransactionRow,
};

pub async fn connect(database_url: &str) -> anyhow::Result<PgPool> {
    let pool = PgPoolOptions::new()
        .max_connections(10)
        .connect(database_url)
        .await?;
    Ok(pool)
}

/// Cheap connectivity check backing `/health`: a real round-trip query, not
/// just "is the pool object alive" (a pool can hold stale/broken connections
/// while still existing) — this API has no purpose without Postgres, so its
/// health check should say so honestly.
pub async fn ping(pool: &PgPool) -> anyhow::Result<()> {
    sqlx::query("SELECT 1").execute(pool).await?;
    Ok(())
}

pub async fn migrate(pool: &PgPool) -> anyhow::Result<()> {
    sqlx::migrate!("./migrations").run(pool).await?;
    Ok(())
}

// ---- ingestion checkpoints ----

/// Reads a persisted ingestion cursor (e.g. the last-processed Horizon
/// paging token). `None` means "never run" — callers decide how to seed.
pub async fn get_ingest_cursor(pool: &PgPool, key: &str) -> anyhow::Result<Option<String>> {
    let row: Option<(String,)> = sqlx::query_as("SELECT value FROM ingest_state WHERE key = $1")
        .bind(key)
        .fetch_optional(pool)
        .await?;
    Ok(row.map(|(v,)| v))
}

pub async fn set_ingest_cursor<'e, E>(executor: E, key: &str, value: &str) -> anyhow::Result<()>
where
    E: sqlx::PgExecutor<'e>,
{
    sqlx::query(
        r#"
        INSERT INTO ingest_state (key, value, updated_at)
        VALUES ($1, $2, now())
        ON CONFLICT (key) DO UPDATE SET value = EXCLUDED.value, updated_at = now()
        "#,
    )
    .bind(key)
    .bind(value)
    .execute(executor)
    .await?;
    Ok(())
}

// ---- writes (used by the ingestion service) ----
//
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

pub async fn insert_asset_price<'e, E>(
    executor: E,
    time: DateTime<Utc>,
    asset_code: &str,
    asset_issuer: &str,
    price_usd: Decimal,
    price_source: &str,
) -> anyhow::Result<()>
where
    E: sqlx::PgExecutor<'e>,
{
    sqlx::query(
        r#"
        INSERT INTO asset_prices (time, asset_code, asset_issuer, price_usd, price_source)
        VALUES ($1, $2, $3, $4, $5)
        "#,
    )
    .bind(time)
    .bind(asset_code)
    .bind(asset_issuer)
    .bind(price_usd)
    .bind(price_source)
    .execute(executor)
    .await?;
    Ok(())
}

// ---- reads (used by the REST API) ----

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

pub async fn insert_alert(
    pool: &PgPool,
    time: DateTime<Utc>,
    kind: &str,
    severity: &str,
    message: &str,
    details: &serde_json::Value,
) -> anyhow::Result<()> {
    sqlx::query(
        r#"
        INSERT INTO alerts (time, kind, severity, message, details)
        VALUES ($1, $2, $3, $4, $5)
        "#,
    )
    .bind(time)
    .bind(kind)
    .bind(severity)
    .bind(message)
    .bind(details)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn list_alerts(pool: &PgPool, limit: i64) -> anyhow::Result<Vec<AlertRow>> {
    let rows = sqlx::query_as::<_, AlertRow>(
        r#"
        SELECT time, kind, severity, message, details
        FROM alerts
        ORDER BY time DESC
        LIMIT $1
        "#,
    )
    .bind(limit)
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

// ---- live events (Postgres NOTIFY, see `events`) ----

/// Publishes `payload` on `channel` via `pg_notify`. Best-effort by design —
/// callers treat a failure here as non-fatal (see call sites in `alerts`):
/// every value that goes out this way is also sitting in a table a client
/// can poll, so a dropped notification delays a live update, it never loses
/// data.
pub async fn notify_event<'e, E>(executor: E, channel: &str, payload: &str) -> anyhow::Result<()>
where
    E: sqlx::PgExecutor<'e>,
{
    sqlx::query("SELECT pg_notify($1, $2)")
        .bind(channel)
        .bind(payload)
        .execute(executor)
        .await?;
    Ok(())
}

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

// ---- watchlist ----

const WATCHLIST_COLUMNS: &str = "id, item_type, item_key, label, created_at";

pub async fn list_watchlist_items(pool: &PgPool) -> anyhow::Result<Vec<WatchlistItemRow>> {
    let rows = sqlx::query_as::<_, WatchlistItemRow>(&format!(
        "SELECT {WATCHLIST_COLUMNS} FROM watchlist_items ORDER BY created_at DESC"
    ))
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

/// Upserts by `(item_type, item_key)` so pinning the same pool/token twice
/// (e.g. a double-click) just refreshes its label instead of erroring or
/// creating a duplicate row.
pub async fn upsert_watchlist_item(
    pool: &PgPool,
    item_type: &str,
    item_key: &str,
    label: &str,
) -> anyhow::Result<WatchlistItemRow> {
    let row = sqlx::query_as::<_, WatchlistItemRow>(&format!(
        r#"
        INSERT INTO watchlist_items (item_type, item_key, label)
        VALUES ($1, $2, $3)
        ON CONFLICT (item_type, item_key) DO UPDATE SET label = EXCLUDED.label
        RETURNING {WATCHLIST_COLUMNS}
        "#
    ))
    .bind(item_type)
    .bind(item_key)
    .bind(label)
    .fetch_one(pool)
    .await?;
    Ok(row)
}

pub async fn delete_watchlist_item(pool: &PgPool, id: i64) -> anyhow::Result<bool> {
    let result = sqlx::query("DELETE FROM watchlist_items WHERE id = $1")
        .bind(id)
        .execute(pool)
        .await?;
    Ok(result.rows_affected() > 0)
}
