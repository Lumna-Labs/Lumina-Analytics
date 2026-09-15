use axum::extract::State;
use axum::response::{IntoResponse, Json};

use crate::alert_rules;
use crate::api::AppState;
use crate::db;

use super::err;

pub async fn liquidations(State(state): State<AppState>) -> impl IntoResponse {
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
