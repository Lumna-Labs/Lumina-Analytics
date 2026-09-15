use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Json};
use serde::Deserialize;

use crate::api::AppState;
use crate::auth;
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
    /// When `true`, only alerts that haven't been acknowledged yet.
    #[serde(default)]
    unacknowledged: bool,
}

pub async fn alerts(
    State(state): State<AppState>,
    Query(q): Query<AlertsQuery>,
) -> impl IntoResponse {
    let limit = q.limit.unwrap_or(100).clamp(1, 1000);
    match db::list_alerts(
        &state.db,
        limit,
        q.kind.as_deref(),
        q.pool_id.as_deref(),
        q.unacknowledged,
    )
    .await
    {
        Ok(rows) => Json(rows).into_response(),
        Err(e) => err(e),
    }
}

/// Marks one alert acknowledged. Gated by the same `ADMIN_API_KEY` as other
/// writes (see `auth::guard`) so a publicly-exposed dashboard can't have its
/// alert feed silenced by an anonymous visitor.
pub async fn acknowledge_alert(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<i64>,
) -> impl IntoResponse {
    if let Some(resp) = auth::guard(&state, &headers) {
        return resp;
    }
    match db::acknowledge_alert(&state.db, id).await {
        Ok(true) => StatusCode::NO_CONTENT.into_response(),
        Ok(false) => StatusCode::NOT_FOUND.into_response(),
        Err(e) => err(e),
    }
}
