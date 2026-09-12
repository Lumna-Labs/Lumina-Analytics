use lumina::api::{build_router, AppState};
use lumina::cache::Cache;
use lumina::config::Config;
use lumina::db;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let config = Config::from_env();
    let pool = db::connect(&config.database_url).await?;
    db::migrate(&pool).await?;

    let cache = Cache::connect(config.redis_url.as_deref(), config.cache_ttl_secs).await;
    let state = AppState { db: pool, cache };
    let app = build_router(state);

    tracing::info!("lumina-api listening on {}", config.api_bind);
    let listener = tokio::net::TcpListener::bind(&config.api_bind).await?;
    axum::serve(listener, app).await?;

    Ok(())
}
