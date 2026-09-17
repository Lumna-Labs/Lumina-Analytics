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
    /// Narrow to alerts about one token (matched against
    /// `details.asset_code`/`details.asset_issuer`, present on token-scoped
    /// alert kinds) — powers the Token Detail page's "Recent Alerts" panel.
    /// Both must be given together to narrow by asset.
    asset_code: Option<String>,
    asset_issuer: Option<String>,
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
        q.asset_code.as_deref(),
        q.asset_issuer.as_deref(),
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

/// Reverses `acknowledge_alert` — lets an operator undo a mis-click on
/// "Ack" or "Ack all visible" without waiting for the next qualifying event
/// to re-raise the alert. Same admin-key gate as acknowledging.
pub async fn unacknowledge_alert(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<i64>,
) -> impl IntoResponse {
    if let Some(resp) = auth::guard(&state, &headers) {
        return resp;
    }
    match db::unacknowledge_alert(&state.db, id).await {
        Ok(true) => StatusCode::NO_CONTENT.into_response(),
        Ok(false) => StatusCode::NOT_FOUND.into_response(),
        Err(e) => err(e),
    }
}

#[derive(Debug, Deserialize)]
pub(super) struct AckBulkBody {
    ids: Vec<i64>,
}

/// Marks every alert in the request body acknowledged in one round trip —
/// backs the Alerts page's "Ack all visible" button, which acks exactly the
/// rows currently shown under the active severity/kind filters rather than
/// re-deriving them server-side. Same admin-key gate and idempotence as
/// `acknowledge_alert`. Caps the batch at 1000 ids, matching the max
/// `/alerts?limit=` page size, since the UI never has more rows visible than
/// that to ack at once.
pub async fn acknowledge_alerts_bulk(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<AckBulkBody>,
) -> impl IntoResponse {
    if let Some(resp) = auth::guard(&state, &headers) {
        return resp;
    }
    if body.ids.len() > 1000 {
        return super::bad_request("at most 1000 ids per request");
    }
    match db::acknowledge_alerts(&state.db, &body.ids).await {
        Ok(count) => Json(serde_json::json!({ "acknowledged": count })).into_response(),
        Err(e) => err(e),
    }
}
