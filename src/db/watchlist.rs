use sqlx::PgPool;

use crate::models::WatchlistItemRow;

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
