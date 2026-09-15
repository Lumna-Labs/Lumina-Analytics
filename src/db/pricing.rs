use chrono::{DateTime, Utc};
use rust_decimal::Decimal;

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
