use chrono::{DateTime, Utc};

use crate::models::AlertRow;

pub async fn insert_alert(
    pool: &sqlx::PgPool,
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

/// Lists recent alerts, optionally narrowed to one `kind` (e.g.
/// `liquidity_drop`), one `pool_id` — matched against `details->>'pool_id'`,
/// present on pool-scoped alert kinds (`liquidity_drop`) so a pool's detail
/// page can show only alerts about itself instead of the global feed — one
/// `asset_code`/`asset_issuer` pair — matched against
/// `details->>'asset_code'`/`details->>'asset_issuer'`, present on
/// token-scoped alert kinds (`holder_drop`), same purpose for a token's
/// detail page — and, when `unacknowledged_only` is set, alerts that
/// haven't been acked yet. Every filter is skipped (matches everything) when
/// left at its default.
#[allow(clippy::too_many_arguments)]
pub async fn list_alerts(
    pool: &sqlx::PgPool,
    limit: i64,
    kind: Option<&str>,
    pool_id: Option<&str>,
    asset_code: Option<&str>,
    asset_issuer: Option<&str>,
    unacknowledged_only: bool,
) -> anyhow::Result<Vec<AlertRow>> {
    let rows = sqlx::query_as::<_, AlertRow>(
        r#"
        SELECT id, time, kind, severity, message, details, acknowledged_at
        FROM alerts
        WHERE ($2::text IS NULL OR kind = $2)
          AND ($3::text IS NULL OR details ->> 'pool_id' = $3)
          AND ($4::text IS NULL OR details ->> 'asset_code' = $4)
          AND ($5::text IS NULL OR details ->> 'asset_issuer' = $5)
          AND (NOT $6 OR acknowledged_at IS NULL)
        ORDER BY time DESC
        LIMIT $1
        "#,
    )
    .bind(limit)
    .bind(kind)
    .bind(pool_id)
    .bind(asset_code)
    .bind(asset_issuer)
    .bind(unacknowledged_only)
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

/// Marks one alert acknowledged (idempotent: re-acking just keeps the
/// original `acknowledged_at`). Returns `false` when `id` doesn't exist, so
/// the handler can 404 instead of reporting success for nothing.
pub async fn acknowledge_alert(pool: &sqlx::PgPool, id: i64) -> anyhow::Result<bool> {
    let result = sqlx::query(
        r#"
        UPDATE alerts
        SET acknowledged_at = COALESCE(acknowledged_at, now())
        WHERE id = $1
        "#,
    )
    .bind(id)
    .execute(pool)
    .await?;
    Ok(result.rows_affected() > 0)
}

/// Reverses `acknowledge_alert` (idempotent, same as `acknowledge_alert`) —
/// backs an "Undo" action for a mis-click on "Ack" or "Ack all visible".
/// Returns `false` when `id` doesn't exist, so the handler can 404.
pub async fn unacknowledge_alert(pool: &sqlx::PgPool, id: i64) -> anyhow::Result<bool> {
    let result = sqlx::query(
        r#"
        UPDATE alerts
        SET acknowledged_at = NULL
        WHERE id = $1
        "#,
    )
    .bind(id)
    .execute(pool)
    .await?;
    Ok(result.rows_affected() > 0)
}

/// Marks every alert in `ids` acknowledged in one round trip (idempotent,
/// same as `acknowledge_alert`) — backs the Alerts page's "Ack all visible"
/// button, which would otherwise need one request per row. Returns how many
/// rows were actually updated (already-acked ids don't change `updated_at`
/// but still count, since they end up acknowledged either way).
pub async fn acknowledge_alerts(pool: &sqlx::PgPool, ids: &[i64]) -> anyhow::Result<u64> {
    if ids.is_empty() {
        return Ok(0);
    }
    let result = sqlx::query(
        r#"
        UPDATE alerts
        SET acknowledged_at = COALESCE(acknowledged_at, now())
        WHERE id = ANY($1)
        "#,
    )
    .bind(ids)
    .execute(pool)
    .await?;
    Ok(result.rows_affected())
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
