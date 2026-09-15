use axum::extract::{Query, State};
use axum::response::{IntoResponse, Json};
use rust_decimal::Decimal;
use serde::Deserialize;

use crate::api::AppState;
use crate::db;

use super::err;

#[derive(Debug, Deserialize)]
pub(super) struct WhaleQuery {
    min_amount: Option<f64>,
    limit: Option<i64>,
    /// Restricts to payments where this account was the source or
    /// destination — powers the per-account Activity page.
    account: Option<String>,
}

pub async fn whale_transactions(
    State(state): State<AppState>,
    Query(q): Query<WhaleQuery>,
) -> impl IntoResponse {
    let min_amount = Decimal::try_from(q.min_amount.unwrap_or(10_000.0)).unwrap_or_default();
    let limit = q.limit.unwrap_or(100).clamp(1, 1000);
    match db::list_whale_transactions(&state.db, min_amount, limit, q.account.as_deref()).await {
        Ok(rows) => Json(rows).into_response(),
        Err(e) => err(e),
    }
}
