//! HTTP route handlers, split into one module per resource so no single file
//! grows unbounded as endpoints are added. `router()` is the only symbol
//! submodules don't own themselves — it wires every handler into one
//! `axum::Router`; everything else here is helpers shared across more than
//! one submodule (pagination, error responses).

use axum::middleware::from_fn_with_state;
use axum::response::{IntoResponse, Json};
use axum::routing::{delete, get, patch};
use axum::Router;
use serde::Deserialize;
use tower_http::cors::CorsLayer;
use tower_http::trace::TraceLayer;

use crate::metrics;
use crate::ratelimit;

use super::AppState;

mod alert_config;
mod alerts;
mod events;
mod health;
mod liquidations;
mod pools;
mod search;
mod tokens;
mod watchlist;
mod whales;

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/health", get(health::health))
        .route("/metrics", get(metrics::metrics_handler))
        .route("/tvl", get(pools::tvl))
        .route("/search", get(search::search))
        .route("/pools", get(pools::pools))
        .route("/pools/:pool_id/history", get(pools::pool_history))
        .route("/pools/trending", get(pools::pools_trending))
        .route("/tokens", get(tokens::tokens))
        .route("/tokens/trending", get(tokens::tokens_trending))
        .route(
            "/tokens/:asset_code/:asset_issuer/history",
            get(tokens::token_history),
        )
        .route("/transactions/whales", get(whales::whale_transactions))
        .route("/liquidations", get(liquidations::liquidations))
        .route("/alerts", get(alerts::alerts))
        .route(
            "/alerts/:id/ack",
            patch(alerts::acknowledge_alert).delete(alerts::unacknowledge_alert),
        )
        .route("/alerts/ack-bulk", patch(alerts::acknowledge_alerts_bulk))
        .route(
            "/alert-channels",
            get(alert_config::list_alert_channels).post(alert_config::create_alert_channel),
        )
        .route(
            "/alert-channels/:id",
            delete(alert_config::delete_alert_channel),
        )
        .route(
            "/alert-rules",
            get(alert_config::list_alert_rules).post(alert_config::create_alert_rule),
        )
        .route(
            "/alert-rules/:id",
            patch(alert_config::update_alert_rule).delete(alert_config::delete_alert_rule),
        )
        .route(
            "/watchlist",
            get(watchlist::list_watchlist).post(watchlist::create_watchlist_item),
        )
        .route("/watchlist/:id", delete(watchlist::delete_watchlist_item))
        .route("/events", get(events::events_stream))
        .route_layer(from_fn_with_state(state.clone(), ratelimit::enforce))
        .route_layer(from_fn_with_state(state.clone(), metrics::track))
        .layer(CorsLayer::permissive())
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}

/// Default and maximum page size for list endpoints. A generous default
/// keeps existing integrations (which don't pass `limit`) working
/// unchanged for realistic dataset sizes; the max cap keeps one request
/// from being able to force an unbounded table scan.
const DEFAULT_PAGE_SIZE: i64 = 500;
const MAX_PAGE_SIZE: i64 = 2000;

#[derive(Debug, Deserialize)]
pub(super) struct PageQuery {
    limit: Option<i64>,
    offset: Option<i64>,
}

impl PageQuery {
    pub(super) fn limit(&self) -> i64 {
        self.limit
            .unwrap_or(DEFAULT_PAGE_SIZE)
            .clamp(1, MAX_PAGE_SIZE)
    }

    pub(super) fn offset(&self) -> i64 {
        self.offset.unwrap_or(0).max(0)
    }
}

#[derive(Debug, Deserialize)]
pub(super) struct RangeQuery {
    pub(super) hours: Option<i64>,
}

pub(super) fn bad_request(message: &str) -> axum::response::Response {
    (
        axum::http::StatusCode::BAD_REQUEST,
        Json(serde_json::json!({ "error": message })),
    )
        .into_response()
}

pub(super) fn err(e: anyhow::Error) -> axum::response::Response {
    tracing::error!("request failed: {e:?}");
    (
        axum::http::StatusCode::INTERNAL_SERVER_ERROR,
        Json(serde_json::json!({ "error": e.to_string() })),
    )
        .into_response()
}
