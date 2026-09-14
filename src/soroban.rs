//! Soroban RPC client and a best-effort lending/liquidation ingester for
//! Blend-style pools.
//!
//! This is a fundamentally different kind of integration than the Horizon
//! client above: Horizon returns typed JSON for a protocol Stellar itself
//! defines, but here we're decoding a third-party smart contract's raw
//! emitted events from XDR with no compiled Rust type shared with Blend's
//! own contract source. Event field *extraction* is therefore heuristic —
//! it looks for an `Address` and the largest-magnitude `I128` anywhere in
//! the event's topics/payload rather than assuming a fixed struct layout.
//! Verify decoded amounts against a block explorer for your specific pool
//! contract before trusting this in production. If Blend changes its event
//! shape, this degrades to skipping unrecognized events (logged), not
//! corrupting stored positions.
//!
//! Because reserve assets are identified here by their Soroban Asset
//! Contract address (not a Horizon-style code/issuer pair), positions are
//! stored with that raw address as the asset identifier and `ltv`/
//! `health_factor` are intentionally left `NULL` — computing those needs a
//! USD price per asset *address*, which isn't wired up. That's a real gap,
//! not a rounding error: this module reports genuine on-chain borrow/supply
//! activity without inventing a risk score it can't back up.

use std::collections::HashMap;
use std::time::Duration;

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use serde_json::json;
use sqlx::PgPool;
use stellar_xdr::{Limits, ReadXdr, ScAddress, ScVal};

use crate::alert_rules::{self, LtvBands};
use crate::alerts;
use crate::config::Config;
use crate::db;
use crate::logic;

/// Soroban token amounts on Stellar use 7 decimal places, matching the
/// classic network's stroop precision — true for every asset routed through
/// a Stellar Asset Contract, which covers Blend's supported reserves.
const AMOUNT_SCALE: u32 = 7;
const EVENTS_CURSOR_KEY: &str = "blend_events_cursor";
const MAX_PAGES_PER_CYCLE: u32 = 10;
const PAGE_LIMIT: u32 = 200;
/// How far back from the chain tip to start on a cold start, in ledgers
/// (roughly 5s/ledger, so ~250 ledgers is ~20 minutes) — enough to not miss
/// events since the last deploy without risking a `startLedger` older than
/// the RPC node's retention window.
const COLD_START_LEDGER_LOOKBACK: u64 = 250;

#[derive(Debug, Serialize)]
struct RpcRequest<'a, P> {
    jsonrpc: &'a str,
    id: u32,
    method: &'a str,
    params: P,
}

#[derive(Debug, Deserialize)]
struct RpcResponse<T> {
    result: Option<T>,
    error: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
struct LatestLedgerResult {
    sequence: u64,
}

#[derive(Debug, Deserialize)]
struct GetEventsResult {
    #[serde(default)]
    events: Vec<RpcEvent>,
    /// The RPC's own pagination cursor for the next page — verified against
    /// a live response rather than assumed; it is *not* reliably identical
    /// to the last event's `id`, even though it happens to look that way for
    /// a full page (empty/short pages are where the two can diverge).
    cursor: Option<String>,
}

#[derive(Debug, Deserialize, Clone)]
struct RpcEvent {
    id: String,
    #[serde(rename = "contractId")]
    contract_id: String,
    topic: Vec<String>,
    value: RpcEventValue,
    #[serde(rename = "ledgerClosedAt")]
    ledger_closed_at: String,
}

/// Soroban RPC nests the value's XDR under `.xdr` in current versions but
/// historically returned it as a bare base64 string; accept either shape.
#[derive(Debug, Deserialize, Clone)]
#[serde(untagged)]
enum RpcEventValue {
    Nested { xdr: String },
    Bare(String),
}

impl RpcEventValue {
    fn xdr(&self) -> &str {
        match self {
            RpcEventValue::Nested { xdr } => xdr,
            RpcEventValue::Bare(s) => s,
        }
    }
}

pub struct SorobanClient {
    http: reqwest::Client,
    rpc_url: String,
}

impl SorobanClient {
    pub fn new(rpc_url: impl Into<String>) -> Self {
        Self {
            http: reqwest::Client::builder()
                .user_agent("lumina-analytics/0.1")
                .timeout(Duration::from_secs(30))
                .build()
                .expect("failed to build reqwest client"),
            rpc_url: rpc_url.into(),
        }
    }

    async fn call<P: Serialize, T: for<'de> Deserialize<'de>>(
        &self,
        method: &str,
        params: P,
    ) -> anyhow::Result<T> {
        let req = RpcRequest {
            jsonrpc: "2.0",
            id: 1,
            method,
            params,
        };
        let resp: RpcResponse<T> = self
            .http
            .post(&self.rpc_url)
            .json(&req)
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;
        if let Some(err) = resp.error {
            anyhow::bail!("soroban rpc `{method}` returned an error: {err}");
        }
        resp.result
            .ok_or_else(|| anyhow::anyhow!("soroban rpc `{method}` returned no result"))
    }

    pub async fn latest_ledger(&self) -> anyhow::Result<u64> {
        let r: LatestLedgerResult = self.call("getLatestLedger", json!({})).await?;
        Ok(r.sequence)
    }

    /// Fetches contract events for `contract_ids`, following the RPC's
    /// pagination `cursor` until caught up or `max_pages` is hit (a safety
    /// cap so one poll tick can't turn into an unbounded crawl through a
    /// large backlog). Exactly one of `start_ledger` (cold start) or
    /// `resume_cursor` (warm start) must be set — the RPC rejects a request
    /// that supplies both or neither.
    /// Returns the fetched events plus the RPC's own cursor after the last
    /// page read — the authoritative resume point for next time, distinct
    /// from (and not safely inferable from) any individual event's `id`.
    async fn get_events(
        &self,
        start_ledger: Option<u64>,
        resume_cursor: Option<&str>,
        contract_ids: &[String],
        max_pages: u32,
    ) -> anyhow::Result<(Vec<RpcEvent>, Option<String>)> {
        let mut out = Vec::new();
        let mut cursor = resume_cursor.map(str::to_string);

        for _ in 0..max_pages {
            let pagination = match &cursor {
                Some(c) => json!({ "cursor": c, "limit": PAGE_LIMIT }),
                None => json!({ "limit": PAGE_LIMIT }),
            };
            let mut params = json!({
                "filters": [{ "type": "contract", "contractIds": contract_ids }],
                "pagination": pagination,
            });
            if cursor.is_none() {
                params["startLedger"] =
                    json!(start_ledger
                        .expect("get_events requires either start_ledger or resume_cursor"));
            }

            let result: GetEventsResult = self.call("getEvents", params).await?;
            let got = result.events.len();
            out.extend(result.events);
            match result.cursor {
                Some(next) => cursor = Some(next),
                None => break,
            }
            if got < PAGE_LIMIT as usize {
                break;
            }
        }
        Ok((out, cursor))
    }
}

/// One poll cycle of the lending ingester: fetch new Blend pool events since
/// the last cursor, decode what we confidently can, and persist position
/// updates. Errors are returned to the caller, which logs and moves on —
/// a bad cycle here must never take down payment/pool ingestion.
pub async fn ingest_lending_cycle(
    pool: &PgPool,
    http: &reqwest::Client,
    config: &Config,
    contract_ids: &[String],
) -> anyhow::Result<()> {
    let client = SorobanClient::new(config.soroban_rpc_url.clone());
    let prices = config.blend_asset_prices_usd();
    let rules = db::list_alert_rules(pool).await.unwrap_or_default();
    let bands = alert_rules::resolve_ltv_bands(&rules);

    let cursor = db::get_ingest_cursor(pool, EVENTS_CURSOR_KEY).await?;
    let start_ledger = match &cursor {
        Some(_) => None, // a cursor takes precedence; the RPC rejects both being set.
        None => Some(
            client
                .latest_ledger()
                .await?
                .saturating_sub(COLD_START_LEDGER_LOOKBACK),
        ),
    };

    let (events, next_cursor) = client
        .get_events(
            start_ledger,
            cursor.as_deref(),
            contract_ids,
            MAX_PAGES_PER_CYCLE,
        )
        .await?;

    if !events.is_empty() {
        tracing::info!("fetched {} Blend contract event(s)", events.len());
    }

    let mut processed = 0;
    for event in &events {
        match process_event(pool, http, config, &prices, &bands, event).await {
            Ok(true) => processed += 1,
            Ok(false) => {}
            Err(e) => tracing::debug!("skipping unparseable Blend event {}: {e:?}", event.id),
        }
    }
    if processed > 0 {
        tracing::info!("recorded {processed} lending position update(s) this cycle");
    }

    if let Some(next_cursor) = next_cursor {
        db::set_ingest_cursor(pool, EVENTS_CURSOR_KEY, &next_cursor).await?;
    }
    Ok(())
}

/// The four Blend request types that move collateral or debt. Everything
/// else (e.g. plain `supply`/`withdraw` of non-collateral liquidity,
/// interest accrual, admin events) is intentionally ignored: it doesn't
/// change a borrower's liquidation risk.
enum PositionEvent {
    SupplyCollateral,
    WithdrawCollateral,
    Borrow,
    Repay,
}

fn classify(event_name: &str) -> Option<PositionEvent> {
    match event_name {
        "supply_collateral" => Some(PositionEvent::SupplyCollateral),
        "withdraw_collateral" => Some(PositionEvent::WithdrawCollateral),
        "borrow" => Some(PositionEvent::Borrow),
        "repay" => Some(PositionEvent::Repay),
        _ => None,
    }
}

/// Decodes one event and, if it's a recognized position-affecting event with
/// enough information extracted, writes an updated position snapshot.
/// Returns `Ok(true)` if a position was written, `Ok(false)` if the event
/// was recognized-but-skippable (e.g. wrong kind), and `Err` if decoding
/// failed outright.
async fn process_event(
    pool: &PgPool,
    http: &reqwest::Client,
    config: &Config,
    prices: &HashMap<String, Decimal>,
    bands: &LtvBands,
    event: &RpcEvent,
) -> anyhow::Result<bool> {
    let topics: Vec<ScVal> = event
        .topic
        .iter()
        .map(|t| ScVal::from_xdr_base64(t, Limits::none()))
        .collect::<Result<_, _>>()
        .map_err(|e| anyhow::anyhow!("bad topic xdr: {e:?}"))?;
    let value = ScVal::from_xdr_base64(event.value.xdr(), Limits::none())
        .map_err(|e| anyhow::anyhow!("bad value xdr: {e:?}"))?;

    let Some(event_name) = topics.first().and_then(sc_val_symbol) else {
        return Ok(false);
    };
    let Some(kind) = classify(&event_name) else {
        return Ok(false);
    };

    // Blend's pool events conventionally carry the reserve asset's contract
    // address as the second topic; fall back to "unknown" rather than
    // guessing if that's not what we find.
    let reserve_asset = topics
        .get(1)
        .and_then(sc_val_address_string)
        .unwrap_or_else(|| "unknown".to_string());

    let mut addresses = Vec::new();
    let mut amounts = Vec::new();
    for t in topics.iter().skip(1) {
        walk_sc_val(t, &mut addresses, &mut amounts);
    }
    walk_sc_val(&value, &mut addresses, &mut amounts);

    let Some(account) = addresses.into_iter().find(|a| *a != reserve_asset) else {
        return Ok(false);
    };
    let Some(raw_amount) = amounts.into_iter().max() else {
        return Ok(false);
    };
    let amount = Decimal::from_i128_with_scale(raw_amount, AMOUNT_SCALE);
    let time: DateTime<Utc> = event
        .ledger_closed_at
        .parse()
        .map_err(|e| anyhow::anyhow!("bad ledgerClosedAt: {e:?}"))?;

    let latest = db::latest_lending_position(pool, "blend", &event.contract_id, &account).await?;
    let previous_risk_level = latest
        .as_ref()
        .and_then(|l| l.ltv)
        .map(|v| alert_rules::risk_level_with_bands(v, bands));
    let mut collateral_asset = latest
        .as_ref()
        .map(|l| l.collateral_asset.clone())
        .unwrap_or_default();
    let mut debt_asset = latest
        .as_ref()
        .map(|l| l.debt_asset.clone())
        .unwrap_or_default();
    let mut collateral_amount = latest
        .as_ref()
        .map_or(Decimal::ZERO, |l| l.collateral_amount);
    let mut debt_amount = latest.as_ref().map_or(Decimal::ZERO, |l| l.debt_amount);

    match kind {
        PositionEvent::SupplyCollateral => {
            collateral_asset = reserve_asset;
            collateral_amount += amount;
        }
        PositionEvent::WithdrawCollateral => {
            collateral_amount = (collateral_amount - amount).max(Decimal::ZERO);
        }
        PositionEvent::Borrow => {
            debt_asset = reserve_asset;
            debt_amount += amount;
        }
        PositionEvent::Repay => {
            debt_amount = (debt_amount - amount).max(Decimal::ZERO);
        }
    }

    let (ltv, health_factor) = logic::compute_lending_risk(
        collateral_amount,
        prices.get(&collateral_asset).copied(),
        debt_amount,
        prices.get(&debt_asset).copied(),
    );

    db::insert_lending_position(
        pool,
        time,
        "blend",
        &event.contract_id,
        &account,
        &collateral_asset,
        collateral_amount,
        &debt_asset,
        debt_amount,
        ltv,
        health_factor,
    )
    .await?;

    if let Some(ltv_value) = ltv {
        let new_level = alert_rules::risk_level_with_bands(ltv_value, bands);
        let escalated = matches!(new_level, "HIGH" | "CRITICAL")
            && previous_risk_level
                .is_none_or(|prev| logic::risk_rank(new_level) > logic::risk_rank(prev));
        if escalated {
            let severity = alerts::severity_for_risk_level(new_level);
            let message = format!(
                "Blend position {} in pool {} crossed into {} risk (LTV {:.1}%)",
                short(&account),
                short(&event.contract_id),
                new_level,
                ltv_value,
            );
            let details = json!({
                "account": account,
                "pool_contract": event.contract_id,
                "ltv": ltv_value.to_string(),
                "health_factor": health_factor.map(|h| h.to_string()),
                "risk_level": new_level,
            });
            if let Err(e) = alerts::record(
                pool,
                http,
                config,
                "liquidation_risk",
                severity,
                message,
                details,
            )
            .await
            {
                tracing::warn!("failed to record liquidation-risk alert: {e:?}");
            }
        }
    }

    Ok(true)
}

fn short(s: &str) -> String {
    if s.len() <= 12 {
        s.to_string()
    } else {
        format!("{}…{}", &s[..6], &s[s.len() - 4..])
    }
}

fn sc_val_symbol(val: &ScVal) -> Option<String> {
    match val {
        ScVal::Symbol(sym) => String::from_utf8((*sym.0).clone()).ok(),
        _ => None,
    }
}

fn sc_val_address_string(val: &ScVal) -> Option<String> {
    match val {
        ScVal::Address(addr) => Some(format_sc_address(addr)),
        _ => None,
    }
}

fn format_sc_address(addr: &ScAddress) -> String {
    match addr {
        ScAddress::Account(account_id) => {
            let stellar_xdr::PublicKey::PublicKeyTypeEd25519(bytes) = &account_id.0;
            stellar_strkey::ed25519::PublicKey(bytes.0).to_string()
        }
        ScAddress::Contract(contract_id) => stellar_strkey::Contract(contract_id.0 .0).to_string(),
        _ => "unsupported-address-kind".to_string(),
    }
}

/// Recursively collects every address and I128 amount found anywhere inside
/// an `ScVal`, since Blend's exact event payload layout (struct field order,
/// map keys) isn't something this crate has a typed definition for.
fn walk_sc_val(val: &ScVal, addresses: &mut Vec<String>, amounts: &mut Vec<i128>) {
    match val {
        ScVal::Address(addr) => addresses.push(format_sc_address(addr)),
        ScVal::I128(parts) => amounts.push(((parts.hi as i128) << 64) | (parts.lo as i128)),
        ScVal::Vec(Some(vec)) => {
            for v in vec.0.iter() {
                walk_sc_val(v, addresses, amounts);
            }
        }
        ScVal::Map(Some(map)) => {
            for entry in map.0.iter() {
                walk_sc_val(&entry.key, addresses, amounts);
                walk_sc_val(&entry.val, addresses, amounts);
            }
        }
        _ => {}
    }
}
