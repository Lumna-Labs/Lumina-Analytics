use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Json};

use crate::api::AppState;
use crate::db;

/// A real readiness check, not a static 200: pings Postgres, since this API
/// has no purpose without it. Redis is deliberately not checked here — it's
/// documented as optional/fail-open (see `Cache`), so its absence shouldn't
/// make an orchestrator think the API is unhealthy.
pub async fn health(State(state): State<AppState>) -> impl IntoResponse {
    match db::ping(&state.db).await {
        Ok(()) => Json(serde_json::json!({ "status": "ok" })).into_response(),
        Err(e) => {
            tracing::error!("health check failed: {e:?}");
            (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(serde_json::json!({ "status": "error" })),
            )
                .into_response()
        }
    }
}
