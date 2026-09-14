use std::convert::Infallible;
use std::time::Duration as StdDuration;

use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::middleware::from_fn_with_state;
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::response::{IntoResponse, Json};
use axum::routing::{delete, get, patch};
use axum::Router;
use chrono::{Duration, Utc};
use futures_util::stream::Stream;
use futures_util::StreamExt;
use rust_decimal::Decimal;
use serde::Deserialize;
use tokio_stream::wrappers::BroadcastStream;
use tower_http::cors::CorsLayer;
use tower_http::trace::TraceLayer;

use crate::alert_rules;
use crate::db;
use crate::metrics;
use crate::ratelimit;

use super::AppState;

/// Default and maximum page size for list endpoints. A generous default
/// keeps existing integrations (which don't pass `limit`) working
/// unchanged for realistic dataset sizes; the max cap keeps one request
/// from being able to force an unbounded table scan.
const DEFAULT_PAGE_SIZE: i64 = 500;
const MAX_PAGE_SIZE: i64 = 2000;

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/health", get(health))
        .route("/metrics", get(metrics::metrics_handler))
        .route("/tvl", get(tvl))
        .route("/pools", get(pools))
        .route("/pools/:pool_id/history", get(pool_history))
        .route("/pools/trending", get(pools_trending))
        .route("/tokens", get(tokens))
        .route(
            "/tokens/:asset_code/:asset_issuer/history",
            get(token_history),
        )
        .route("/transactions/whales", get(whale_transactions))
        .route("/liquidations", get(liquidations))
        .route("/alerts", get(alerts))
        .route(
            "/alert-channels",
            get(list_alert_channels).post(create_alert_channel),
        )
        .route("/alert-channels/:id", delete(delete_alert_channel))
        .route(
            "/alert-rules",
            get(list_alert_rules).post(create_alert_rule),
        )
        .route(
            "/alert-rules/:id",
            patch(update_alert_rule).delete(delete_alert_rule),
        )
        .route(
            "/watchlist",
            get(list_watchlist).post(create_watchlist_item),
        )
        .route("/watchlist/:id", delete(delete_watchlist_item))
        .route("/events", get(events_stream))
        .route_layer(from_fn_with_state(state.clone(), ratelimit::enforce))
        .route_layer(from_fn_with_state(state.clone(), metrics::track))
        .layer(CorsLayer::permissive())
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}

#[derive(Debug, Deserialize)]
struct PageQuery {
    limit: Option<i64>,
    offset: Option<i64>,
}

impl PageQuery {
    fn limit(&self) -> i64 {
        self.limit
            .unwrap_or(DEFAULT_PAGE_SIZE)
            .clamp(1, MAX_PAGE_SIZE)
    }

    fn offset(&self) -> i64 {
        self.offset.unwrap_or(0).max(0)
    }
}

/// A real readiness check, not a static 200: pings Postgres, since this API
/// has no purpose without it. Redis is deliberately not checked here — it's
/// documented as optional/fail-open (see `Cache`), so its absence shouldn't
/// make an orchestrator think the API is unhealthy.
async fn health(State(state): State<AppState>) -> impl IntoResponse {
    match db::ping(&state.db).await {
        Ok(()) => Json(serde_json::json!({ "status": "ok" })).into_response(),
        Err(e) => {
            tracing::error!("health check failed: {e:?}");
            (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(serde_json::json!({ "status": "error" })),
            )
                .into_response()
        }
    }
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

async fn pools(State(state): State<AppState>, Query(q): Query<PageQuery>) -> impl IntoResponse {
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

async fn tokens(State(state): State<AppState>, Query(q): Query<PageQuery>) -> impl IntoResponse {
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
    /// Restricts to payments where this account was the source or
    /// destination — powers the per-account Activity page.
    account: Option<String>,
}

async fn whale_transactions(
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

async fn liquidations(State(state): State<AppState>) -> impl IntoResponse {
    let rules = match db::list_alert_rules(&state.db).await {
        Ok(r) => r,
        Err(e) => return err(e),
    };
    let bands = alert_rules::resolve_ltv_bands(&rules);
    let summary =
        match db::liquidation_risk_summary(&state.db, bands.medium, bands.high, bands.critical)
            .await
        {
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

#[derive(Debug, Deserialize)]
struct AlertsQuery {
    limit: Option<i64>,
}

async fn alerts(State(state): State<AppState>, Query(q): Query<AlertsQuery>) -> impl IntoResponse {
    let limit = q.limit.unwrap_or(100).clamp(1, 1000);
    match db::list_alerts(&state.db, limit).await {
        Ok(rows) => Json(rows).into_response(),
        Err(e) => err(e),
    }
}

// ---- alert channels ----

#[derive(Debug, Deserialize)]
struct NewAlertChannel {
    name: String,
    kind: String,
    url: String,
    #[serde(default = "default_min_severity")]
    min_severity: String,
    #[serde(default = "default_true")]
    enabled: bool,
}

fn default_min_severity() -> String {
    "WARNING".to_string()
}

fn default_true() -> bool {
    true
}

async fn list_alert_channels(State(state): State<AppState>) -> impl IntoResponse {
    match db::list_alert_channels(&state.db).await {
        Ok(rows) => Json(rows).into_response(),
        Err(e) => err(e),
    }
}

async fn create_alert_channel(
    State(state): State<AppState>,
    Json(body): Json<NewAlertChannel>,
) -> impl IntoResponse {
    if !["slack", "discord", "generic"].contains(&body.kind.as_str()) {
        return bad_request("kind must be one of: slack, discord, generic");
    }
    if crate::alerts::Severity::parse(&body.min_severity).is_none() {
        return bad_request("min_severity must be one of: INFO, WARNING, CRITICAL");
    }
    match db::insert_alert_channel(
        &state.db,
        &body.name,
        &body.kind,
        &body.url,
        &body.min_severity,
        body.enabled,
    )
    .await
    {
        Ok(row) => (StatusCode::CREATED, Json(row)).into_response(),
        Err(e) => err(e),
    }
}

async fn delete_alert_channel(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> impl IntoResponse {
    match db::delete_alert_channel(&state.db, id).await {
        Ok(true) => StatusCode::NO_CONTENT.into_response(),
        Ok(false) => StatusCode::NOT_FOUND.into_response(),
        Err(e) => err(e),
    }
}

// ---- alert rules ----

#[derive(Debug, Deserialize)]
struct NewAlertRule {
    name: String,
    rule_type: String,
    asset_code: Option<String>,
    asset_issuer: Option<String>,
    threshold: Decimal,
    #[serde(default = "default_true")]
    enabled: bool,
}

#[derive(Debug, Deserialize)]
struct UpdateAlertRule {
    threshold: Decimal,
    enabled: bool,
}

async fn list_alert_rules(State(state): State<AppState>) -> impl IntoResponse {
    match db::list_alert_rules(&state.db).await {
        Ok(rows) => Json(rows).into_response(),
        Err(e) => err(e),
    }
}

async fn create_alert_rule(
    State(state): State<AppState>,
    Json(body): Json<NewAlertRule>,
) -> impl IntoResponse {
    match body.rule_type.as_str() {
        "whale_threshold" => {
            if body.asset_code.is_none() {
                return bad_request("whale_threshold rules require asset_code");
            }
        }
        "ltv_band" => {
            if !["MEDIUM", "HIGH", "CRITICAL"].contains(&body.name.to_ascii_uppercase().as_str()) {
                return bad_request("ltv_band rules must be named MEDIUM, HIGH, or CRITICAL");
            }
        }
        _ => return bad_request("rule_type must be one of: whale_threshold, ltv_band"),
    }
    match db::insert_alert_rule(
        &state.db,
        &body.name,
        &body.rule_type,
        body.asset_code.as_deref(),
        body.asset_issuer.as_deref(),
        body.threshold,
        body.enabled,
    )
    .await
    {
        Ok(row) => (StatusCode::CREATED, Json(row)).into_response(),
        Err(e) => err(e),
    }
}

async fn update_alert_rule(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    Json(body): Json<UpdateAlertRule>,
) -> impl IntoResponse {
    match db::update_alert_rule(&state.db, id, body.threshold, body.enabled).await {
        Ok(Some(row)) => Json(row).into_response(),
        Ok(None) => StatusCode::NOT_FOUND.into_response(),
        Err(e) => err(e),
    }
}

async fn delete_alert_rule(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> impl IntoResponse {
    match db::delete_alert_rule(&state.db, id).await {
        Ok(true) => StatusCode::NO_CONTENT.into_response(),
        Ok(false) => StatusCode::NOT_FOUND.into_response(),
        Err(e) => err(e),
    }
}

// ---- watchlist ----

#[derive(Debug, Deserialize)]
struct NewWatchlistItem {
    item_type: String,
    item_key: String,
    label: String,
}

async fn list_watchlist(State(state): State<AppState>) -> impl IntoResponse {
    match db::list_watchlist_items(&state.db).await {
        Ok(rows) => Json(rows).into_response(),
        Err(e) => err(e),
    }
}

async fn create_watchlist_item(
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

async fn delete_watchlist_item(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> impl IntoResponse {
    match db::delete_watchlist_item(&state.db, id).await {
        Ok(true) => StatusCode::NO_CONTENT.into_response(),
        Ok(false) => StatusCode::NOT_FOUND.into_response(),
        Err(e) => err(e),
    }
}

// ---- live events (SSE) ----

/// Streams live-event notifications (new alerts, which cover whale payments
/// and lending-risk escalations — see `alerts::record`) as they're published
/// via Postgres NOTIFY (see `events`). A best-effort nicety: every event
/// pushed here also landed in a table a client can poll, so a client that
/// connects late, misses a broadcast (`Lagged`), or never opens this stream
/// at all still sees everything on its next regular poll — nothing is only
/// available here.
async fn events_stream(
    State(state): State<AppState>,
) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    let receiver = state.events.subscribe();
    let stream = BroadcastStream::new(receiver).filter_map(|msg| async move {
        match msg {
            Ok(payload) => Some(Ok(Event::default().event("alert").data(payload))),
            // A slow/absent-for-a-while subscriber dropped some messages;
            // skip them rather than ending the stream, since a client that
            // just missed some history still catches up via polling.
            Err(_lagged) => None,
        }
    });
    Sse::new(stream).keep_alive(KeepAlive::new().interval(StdDuration::from_secs(15)))
}

fn bad_request(message: &str) -> axum::response::Response {
    (
        StatusCode::BAD_REQUEST,
        Json(serde_json::json!({ "error": message })),
    )
        .into_response()
}

fn err(e: anyhow::Error) -> axum::response::Response {
    tracing::error!("request failed: {e:?}");
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(serde_json::json!({ "error": e.to_string() })),
    )
        .into_response()
}
