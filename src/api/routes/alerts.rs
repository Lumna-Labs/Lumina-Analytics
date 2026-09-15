use axum::extract::{Query, State};
use axum::response::{IntoResponse, Json};
use serde::Deserialize;

use crate::api::AppState;
use crate::db;

use super::err;

#[derive(Debug, Deserialize)]
pub(super) struct AlertsQuery {
    limit: Option<i64>,
}

pub async fn alerts(
    State(state): State<AppState>,
    Query(q): Query<AlertsQuery>,
) -> impl IntoResponse {
    let limit = q.limit.unwrap_or(100).clamp(1, 1000);
    match db::list_alerts(&state.db, limit).await {
        Ok(rows) => Json(rows).into_response(),
        Err(e) => err(e),
    }
}
