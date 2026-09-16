//! Handlers for the two operator-editable alerting resources: delivery
//! channels and threshold/band override rules (see `db::alert_config`).

use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Json};
use rust_decimal::Decimal;
use serde::Deserialize;

use crate::api::AppState;
use crate::auth;
use crate::db;

use super::{bad_request, err};

// ---- alert channels ----

#[derive(Debug, Deserialize)]
pub(super) struct NewAlertChannel {
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

pub async fn list_alert_channels(State(state): State<AppState>) -> impl IntoResponse {
    match db::list_alert_channels(&state.db).await {
        Ok(rows) => Json(rows).into_response(),
        Err(e) => err(e),
    }
}

pub async fn create_alert_channel(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<NewAlertChannel>,
) -> impl IntoResponse {
    if let Some(resp) = auth::guard(&state, &headers) {
        return resp;
    }
    if body.name.trim().is_empty() {
        return bad_request("name must not be empty");
    }
    if !["slack", "discord", "generic"].contains(&body.kind.as_str()) {
        return bad_request("kind must be one of: slack, discord, generic");
    }
    if !(body.url.starts_with("http://") || body.url.starts_with("https://")) {
        return bad_request("url must start with http:// or https://");
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

pub async fn delete_alert_channel(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<i64>,
) -> impl IntoResponse {
    if let Some(resp) = auth::guard(&state, &headers) {
        return resp;
    }
    match db::delete_alert_channel(&state.db, id).await {
        Ok(true) => StatusCode::NO_CONTENT.into_response(),
        Ok(false) => StatusCode::NOT_FOUND.into_response(),
        Err(e) => err(e),
    }
}

// ---- alert rules ----

#[derive(Debug, Deserialize)]
pub(super) struct NewAlertRule {
    name: String,
    rule_type: String,
    asset_code: Option<String>,
    asset_issuer: Option<String>,
    threshold: Decimal,
    #[serde(default = "default_true")]
    enabled: bool,
}

#[derive(Debug, Deserialize)]
pub(super) struct UpdateAlertRule {
    threshold: Decimal,
    enabled: bool,
}

pub async fn list_alert_rules(State(state): State<AppState>) -> impl IntoResponse {
    match db::list_alert_rules(&state.db).await {
        Ok(rows) => Json(rows).into_response(),
        Err(e) => err(e),
    }
}

pub async fn create_alert_rule(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<NewAlertRule>,
) -> impl IntoResponse {
    if let Some(resp) = auth::guard(&state, &headers) {
        return resp;
    }
    if body.name.trim().is_empty() {
        return bad_request("name must not be empty");
    }
    if body.threshold <= Decimal::ZERO {
        return bad_request("threshold must be greater than zero");
    }
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

pub async fn update_alert_rule(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<i64>,
    Json(body): Json<UpdateAlertRule>,
) -> impl IntoResponse {
    if let Some(resp) = auth::guard(&state, &headers) {
        return resp;
    }
    if body.threshold <= Decimal::ZERO {
        return bad_request("threshold must be greater than zero");
    }
    match db::update_alert_rule(&state.db, id, body.threshold, body.enabled).await {
        Ok(Some(row)) => Json(row).into_response(),
        Ok(None) => StatusCode::NOT_FOUND.into_response(),
        Err(e) => err(e),
    }
}

pub async fn delete_alert_rule(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<i64>,
) -> impl IntoResponse {
    if let Some(resp) = auth::guard(&state, &headers) {
        return resp;
    }
    match db::delete_alert_rule(&state.db, id).await {
        Ok(true) => StatusCode::NO_CONTENT.into_response(),
        Ok(false) => StatusCode::NOT_FOUND.into_response(),
        Err(e) => err(e),
    }
}
