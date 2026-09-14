pub mod routes;

use std::sync::Arc;

use axum::Router;
use sqlx::PgPool;

use crate::cache::Cache;
use crate::metrics::Metrics;
use crate::ratelimit::RateLimiter;

#[derive(Clone)]
pub struct AppState {
    pub db: PgPool,
    pub cache: Cache,
    pub metrics: Arc<Metrics>,
    pub rate_limiter: Arc<RateLimiter>,
}

pub fn build_router(state: AppState) -> Router {
    routes::router(state)
}
