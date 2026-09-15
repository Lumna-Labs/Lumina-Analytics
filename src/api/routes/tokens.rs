use axum::extract::{Path, Query, State};
use axum::response::{IntoResponse, Json};
use chrono::{Duration, Utc};

use crate::api::AppState;
use crate::db;

use super::{err, PageQuery, RangeQuery};

pub async fn tokens(
    State(state): State<AppState>,
    Query(q): Query<PageQuery>,
) -> impl IntoResponse {
    let (limit, offset) = (q.limit(), q.offset());
    let key = format!("tokens:{limit}:{offset}");
    match state
        .cache
        .get_or_compute(&key, || {
            db::list_tokens_with_latest(&state.db, limit, offset)
        })
        .await
    {
        Ok(rows) => Json(rows).into_response(),
        Err(e) => err(e),
    }
}

pub async fn token_history(
    State(state): State<AppState>,
    Path((asset_code, asset_issuer)): Path<(String, String)>,
    Query(q): Query<RangeQuery>,
) -> impl IntoResponse {
    let since = Utc::now() - Duration::hours(q.hours.unwrap_or(24));
    match db::token_history(&state.db, &asset_code, &asset_issuer, since).await {
        Ok(rows) => Json(rows).into_response(),
        Err(e) => err(e),
    }
}
