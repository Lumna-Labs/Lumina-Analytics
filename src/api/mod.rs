pub mod routes;

use axum::Router;
use sqlx::PgPool;

use crate::cache::Cache;

#[derive(Clone)]
pub struct AppState {
    pub db: PgPool,
    pub cache: Cache,
}

pub fn build_router(state: AppState) -> Router {
    routes::router(state)
}
