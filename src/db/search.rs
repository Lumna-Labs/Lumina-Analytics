use sqlx::PgPool;

use crate::models::SearchResultRow;

/// Global search across pools (by asset code or pool ID), tokens (by asset
/// code or issuer), and accounts seen as a source/destination in recorded
/// whale payments — powers the sidebar search bar. `pattern` is a
/// pre-escaped ILIKE pattern (see `logic::search_like_pattern`); `limit`
/// caps each of the three categories independently, so a noisy category
/// (e.g. many matching tokens) can't crowd the others out of the results.
pub async fn search(
    pool: &PgPool,
    pattern: &str,
    limit: i64,
) -> anyhow::Result<Vec<SearchResultRow>> {
    let rows = sqlx::query_as::<_, SearchResultRow>(
        r#"
        (
            SELECT 'pool' AS result_type,
                   (asset_a || '/' || asset_b) AS title,
                   pool_id AS subtitle,
                   pool_id AS key
            FROM pools
            WHERE asset_a ILIKE $1 ESCAPE '\' OR asset_b ILIKE $1 ESCAPE '\'
               OR pool_id ILIKE $1 ESCAPE '\'
            ORDER BY pool_id
            LIMIT $2
        )
        UNION ALL
        (
            SELECT 'token' AS result_type,
                   asset_code AS title,
                   asset_issuer AS subtitle,
                   (asset_code || ':' || asset_issuer) AS key
            FROM tokens
            WHERE asset_code ILIKE $1 ESCAPE '\' OR asset_issuer ILIKE $1 ESCAPE '\'
            ORDER BY asset_code
            LIMIT $2
        )
        UNION ALL
        (
            SELECT 'account' AS result_type,
                   account AS title,
                   NULL::text AS subtitle,
                   account AS key
            FROM (
                SELECT DISTINCT source_account AS account
                FROM whale_transactions
                WHERE source_account ILIKE $1 ESCAPE '\'
                UNION
                SELECT DISTINCT dest_account AS account
                FROM whale_transactions
                WHERE dest_account ILIKE $1 ESCAPE '\'
            ) accounts
            ORDER BY account
            LIMIT $2
        )
        "#,
    )
    .bind(pattern)
    .bind(limit)
    .fetch_all(pool)
    .await?;
    Ok(rows)
}
