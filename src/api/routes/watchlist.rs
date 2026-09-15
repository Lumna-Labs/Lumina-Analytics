use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Json};
use serde::Deserialize;

use crate::api::AppState;
use crate::db;

use super::{bad_request, err};

#[derive(Debug, Deserialize)]
pub(super) struct NewWatchlistItem {
    item_type: String,
    item_key: String,
    label: String,
}

pub async fn list_watchlist(State(state): State<AppState>) -> impl IntoResponse {
    match db::list_watchlist_items(&state.db).await {
        Ok(rows) => Json(rows).into_response(),
        Err(e) => err(e),
    }
}

pub async fn create_watchlist_item(
    State(state): State<AppState>,
    Json(body): Json<NewWatchlistItem>,
) -> impl IntoResponse {
    if !["pool", "token", "lending_position"].contains(&body.item_type.as_str()) {
        return bad_request("item_type must be one of: pool, token, lending_position");
    }
    match db::upsert_watchlist_item(&state.db, &body.item_type, &body.item_key, &body.label).await {
        Ok(row) => (StatusCode::CREATED, Json(row)).into_response(),
        Err(e) => err(e),
    }
}

pub async fn delete_watchlist_item(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> impl IntoResponse {
    match db::delete_watchlist_item(&state.db, id).await {
        Ok(true) => StatusCode::NO_CONTENT.into_response(),
        Ok(false) => StatusCode::NOT_FOUND.into_response(),
        Err(e) => err(e),
    }
}
