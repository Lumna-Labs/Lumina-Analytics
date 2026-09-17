use axum::extract::{Path, Query, State};
use axum::response::{IntoResponse, Json};
use chrono::{Duration, Utc};
use rust_decimal::Decimal;

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

pub async fn tokens_trending(
    State(state): State<AppState>,
    Query(q): Query<RangeQuery>,
) -> impl IntoResponse {
    let hours = q.hours.unwrap_or(24);
    let since = Utc::now() - Duration::hours(hours);
    let key = format!("tokens_trending:{hours}");

    let compute = || async {
        let raw = db::token_trends(&state.db, since).await?;
        let mut trends: Vec<crate::models::TokenTrend> = raw
            .into_iter()
            .map(|r| {
                let holders_change_pct = crate::logic::percent_change(
                    Decimal::from(r.holders_before),
                    Decimal::from(r.holders_now),
                );
                crate::models::TokenTrend {
                    asset_code: r.asset_code,
                    asset_issuer: r.asset_issuer,
                    first_time: r.first_time,
                    last_time: r.last_time,
                    amount_before: r.amount_before,
                    amount_now: r.amount_now,
                    holders_before: r.holders_before,
                    holders_now: r.holders_now,
                    holders_change_pct,
                }
            })
            .collect();
        trends.sort_by_key(|t| std::cmp::Reverse(t.holders_change_pct));
        Ok(trends)
    };

    match state.cache.get_or_compute(&key, compute).await {
        Ok(trends) => Json(trends).into_response(),
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
