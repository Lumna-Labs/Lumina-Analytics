//! Database access layer. Split into one module per domain (pools, tokens,
//! whale payments, lending, alerting, watchlist, search) so no single file
//! grows unbounded as features are added; everything is re-exported flat
//! here so call sites keep using `db::whatever(...)` regardless of which
//! submodule actually implements it.

use sqlx::postgres::PgPoolOptions;
use sqlx::PgPool;

mod alert_config;
mod alerts;
mod lending;
mod pools;
mod pricing;
mod search;
mod tokens;
mod watchlist;
mod whales;

pub use alert_config::*;
pub use alerts::*;
pub use lending::*;
pub use pools::*;
pub use pricing::*;
pub use search::*;
pub use tokens::*;
pub use watchlist::*;
pub use whales::*;

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
