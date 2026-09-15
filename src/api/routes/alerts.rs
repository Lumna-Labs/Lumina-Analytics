use axum::extract::{Query, State};
use axum::response::{IntoResponse, Json};
use serde::Deserialize;

use crate::api::AppState;
use crate::db;

use super::err;

#[derive(Debug, Deserialize)]
pub(super) struct AlertsQuery {
    limit: Option<i64>,
    /// Narrow to one alert kind, e.g. `liquidity_drop`.
    kind: Option<String>,
    /// Narrow to alerts about one pool (matched against `details.pool_id`,
    /// present on pool-scoped alert kinds) — powers the Pool Detail page's
    /// "Recent Alerts" panel.
    pool_id: Option<String>,
}

pub async fn alerts(
    State(state): State<AppState>,
    Query(q): Query<AlertsQuery>,
) -> impl IntoResponse {
    let limit = q.limit.unwrap_or(100).clamp(1, 1000);
    match db::list_alerts(&state.db, limit, q.kind.as_deref(), q.pool_id.as_deref()).await {
        Ok(rows) => Json(rows).into_response(),
        Err(e) => err(e),
    }
}
