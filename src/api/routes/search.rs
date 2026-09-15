use axum::extract::{Query, State};
use axum::response::{IntoResponse, Json};
use serde::Deserialize;

use crate::api::AppState;
use crate::db;

use super::err;

#[derive(Debug, Deserialize)]
pub(super) struct SearchQuery {
    q: Option<String>,
    limit: Option<i64>,
}

/// Global search across pools, tokens, and whale-payment accounts, powering
/// the sidebar search bar. Not cached (unlike the list endpoints in
/// `pools`/`tokens`) — query strings are effectively unbounded, so caching
/// would mostly just grow the cache without saving repeat hits.
pub async fn search(
    State(state): State<AppState>,
    Query(q): Query<SearchQuery>,
) -> impl IntoResponse {
    let Some(pattern) = crate::logic::search_like_pattern(q.q.as_deref().unwrap_or("")) else {
        return Json(Vec::<crate::models::SearchResultRow>::new()).into_response();
    };
    let limit = q.limit.unwrap_or(8).clamp(1, 25);
    match db::search(&state.db, &pattern, limit).await {
        Ok(rows) => Json(rows).into_response(),
        Err(e) => err(e),
    }
}
