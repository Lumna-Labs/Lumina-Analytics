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

pub async fn list_alerts(pool: &sqlx::PgPool, limit: i64) -> anyhow::Result<Vec<AlertRow>> {
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
