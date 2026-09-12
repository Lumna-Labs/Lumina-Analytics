use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Json};
use axum::routing::get;
use axum::Router;
use chrono::{Duration, Utc};
use rust_decimal::Decimal;
use serde::Deserialize;
use tower_http::cors::CorsLayer;
use tower_http::trace::TraceLayer;

use crate::db;

use super::AppState;

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/health", get(health))
        .route("/tvl", get(tvl))
        .route("/pools", get(pools))
        .route("/pools/:pool_id/history", get(pool_history))
        .route("/tokens", get(tokens))
        .route(
            "/tokens/:asset_code/:asset_issuer/history",
            get(token_history),
        )
        .route("/transactions/whales", get(whale_transactions))
        .route("/liquidations", get(liquidations))
        .route("/pools/trending", get(pools_trending))
        .layer(CorsLayer::permissive())
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}

async fn health() -> impl IntoResponse {
    Json(serde_json::json!({ "status": "ok" }))
}

#[derive(Debug, Deserialize)]
struct RangeQuery {
    hours: Option<i64>,
}

async fn tvl(State(state): State<AppState>, Query(q): Query<RangeQuery>) -> impl IntoResponse {
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

async fn pools(State(state): State<AppState>) -> impl IntoResponse {
    match state
        .cache
        .get_or_compute("pools", || db::list_pools_with_latest(&state.db))
        .await
    {
        Ok(rows) => Json(rows).into_response(),
        Err(e) => err(e),
    }
}

async fn pool_history(
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

async fn tokens(State(state): State<AppState>) -> impl IntoResponse {
    match state
        .cache
        .get_or_compute("tokens", || db::list_tokens_with_latest(&state.db))
        .await
    {
        Ok(rows) => Json(rows).into_response(),
        Err(e) => err(e),
    }
}

async fn token_history(
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

#[derive(Debug, Deserialize)]
struct WhaleQuery {
    min_amount: Option<f64>,
    limit: Option<i64>,
}

async fn whale_transactions(
    State(state): State<AppState>,
    Query(q): Query<WhaleQuery>,
) -> impl IntoResponse {
    let min_amount = Decimal::try_from(q.min_amount.unwrap_or(10_000.0)).unwrap_or_default();
    let limit = q.limit.unwrap_or(100).clamp(1, 1000);
    match db::list_whale_transactions(&state.db, min_amount, limit).await {
        Ok(rows) => Json(rows).into_response(),
        Err(e) => err(e),
    }
}

async fn liquidations(State(state): State<AppState>) -> impl IntoResponse {
    let summary = match db::liquidation_risk_summary(&state.db).await {
        Ok(s) => s,
        Err(e) => return err(e),
    };
    let mut positions = match db::list_lending_positions(&state.db).await {
        Ok(p) => p,
        Err(e) => return err(e),
    };
    positions.sort_by_key(|p| std::cmp::Reverse(p.ltv));

    let note = if positions.is_empty() {
        "No lending positions ingested yet. Set BLEND_POOL_IDS to real Blend pool contract \
         addresses to turn on the Soroban event ingester."
    } else if summary.is_empty() {
        "Positions below are tracked from real on-chain Blend events (collateral/debt amounts \
         are live). Risk bucketing (LTV/health factor) isn't computed yet — that needs a USD \
         price per Soroban asset-contract address, which isn't wired up, so those columns show \
         no invented numbers rather than a guess."
    } else {
        "Live data."
    };

    Json(serde_json::json!({
        "summary": summary,
        "positions": positions,
        "note": note,
    }))
    .into_response()
}

async fn pools_trending(
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

fn err(e: anyhow::Error) -> axum::response::Response {
    tracing::error!("request failed: {e:?}");
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(serde_json::json!({ "error": e.to_string() })),
    )
        .into_response()
}
