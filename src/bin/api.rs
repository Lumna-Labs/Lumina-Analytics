use std::sync::Arc;

use lumina::api::{build_router, AppState};
use lumina::cache::Cache;
use lumina::config::Config;
use lumina::db;
use lumina::events;
use lumina::metrics::Metrics;
use lumina::ratelimit::RateLimiter;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let config = Config::from_env();
    let pool = db::connect(&config.database_url).await?;
    db::migrate(&pool).await?;

    let cache = Cache::connect(config.redis_url.as_deref(), config.cache_ttl_secs).await;
    if config.rate_limit_rps > 0 {
        tracing::info!(
            "rate limiting enabled: {} req/s, burst {}",
            config.rate_limit_rps,
            config.rate_limit_burst
        );
    }
    let event_bus = events::new_bus();
    tokio::spawn(events::listen_and_forward(
        config.database_url.clone(),
        event_bus.clone(),
    ));

    let state = AppState {
        db: pool,
        cache,
        metrics: Arc::new(Metrics::new()),
        rate_limiter: Arc::new(RateLimiter::new(
            config.rate_limit_rps,
            config.rate_limit_burst,
        )),
        events: event_bus,
    };
    let app = build_router(state);

    tracing::info!("lumina-api listening on {}", config.api_bind);
    let listener = tokio::net::TcpListener::bind(&config.api_bind).await?;
    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<std::net::SocketAddr>(),
    )
    .await?;

    Ok(())
}
