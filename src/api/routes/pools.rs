use axum::extract::{Path, Query, State};
use axum::response::{IntoResponse, Json};
use chrono::{Duration, Utc};

use crate::api::AppState;
use crate::db;

use super::{err, PageQuery, RangeQuery};

pub async fn tvl(State(state): State<AppState>, Query(q): Query<RangeQuery>) -> impl IntoResponse {
    let hours = q.hours.unwrap_or(24);
    let since = Utc::now() - Duration::hours(hours);
    let key = format!("tvl:{hours}");
    match state
        .cache
        .get_or_compute(&key, || db::tvl_series(&state.db, since))
        .await
    {
        Ok(series) => Json(series).into_response(),
        Err(e) => err(e),
    }
}

pub async fn pools(State(state): State<AppState>, Query(q): Query<PageQuery>) -> impl IntoResponse {
    let (limit, offset) = (q.limit(), q.offset());
    let key = format!("pools:{limit}:{offset}");
    match state
        .cache
        .get_or_compute(&key, || {
            db::list_pools_with_latest(&state.db, limit, offset)
        })
        .await
    {
        Ok(rows) => Json(rows).into_response(),
        Err(e) => err(e),
    }
}

pub async fn pool_history(
    State(state): State<AppState>,
    Path(pool_id): Path<String>,
    Query(q): Query<RangeQuery>,
) -> impl IntoResponse {
    let since = Utc::now() - Duration::hours(q.hours.unwrap_or(24));
    match db::pool_history(&state.db, &pool_id, since).await {
        Ok(rows) => Json(rows).into_response(),
        Err(e) => err(e),
    }
}

pub async fn pools_trending(
    State(state): State<AppState>,
    Query(q): Query<RangeQuery>,
) -> impl IntoResponse {
    let hours = q.hours.unwrap_or(24);
    let since = Utc::now() - Duration::hours(hours);
    let key = format!("pools_trending:{hours}");

    let compute = || async {
        let raw = db::pool_trends(&state.db, since).await?;
        let mut trends: Vec<crate::models::PoolTrend> = raw
            .into_iter()
            .map(|r| {
                let shares_change_pct = crate::logic::percent_change(r.shares_before, r.shares_now);
                crate::models::PoolTrend {
                    pool_id: r.pool_id,
                    asset_a: r.asset_a,
                    asset_b: r.asset_b,
                    fee_bp: r.fee_bp,
                    first_time: r.first_time,
                    last_time: r.last_time,
                    shares_before: r.shares_before,
                    shares_now: r.shares_now,
                    reserve_a_now: r.reserve_a_now,
                    reserve_b_now: r.reserve_b_now,
                    shares_change_pct,
                }
            })
            .collect();
        trends.sort_by_key(|t| std::cmp::Reverse(t.shares_change_pct));
        Ok(trends)
    };

    match state.cache.get_or_compute(&key, compute).await {
        Ok(trends) => Json(trends).into_response(),
        Err(e) => err(e),
    }
}
