//! Alert recording and optional webhook delivery for events worth surfacing
//! proactively: outsized whale payments and lending positions crossing into
//! a higher liquidation-risk band.
//!
//! Every alert is written to the `alerts` table regardless of configuration,
//! so the dashboard has something to show even with no webhook set up. The
//! webhook (`ALERT_WEBHOOK_URL`) is an additional, optional delivery channel
//! gated by a minimum severity — never a requirement for the feature to do
//! anything.

use chrono::Utc;
use rust_decimal::Decimal;
use serde_json::{json, Value};
use sqlx::PgPool;

use crate::config::Config;
use crate::db;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Severity {
    Info,
    Warning,
    Critical,
}

impl Severity {
    pub fn as_str(&self) -> &'static str {
        match self {
            Severity::Info => "INFO",
            Severity::Warning => "WARNING",
            Severity::Critical => "CRITICAL",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s.to_ascii_uppercase().as_str() {
            "INFO" => Some(Severity::Info),
            "WARNING" => Some(Severity::Warning),
            "CRITICAL" => Some(Severity::Critical),
            _ => None,
        }
    }
}

/// Maps a liquidation risk-band string (see `logic::risk_level`) to an alert
/// severity. `LOW`/`MEDIUM` don't warrant an alert on their own — only
/// escalation into `HIGH`/`CRITICAL` does (see `ingest_lending_cycle`'s
/// caller, which only calls this path on an upward crossing).
pub fn severity_for_risk_level(risk_level: &str) -> Severity {
    match risk_level {
        "CRITICAL" => Severity::Critical,
        "HIGH" => Severity::Warning,
        _ => Severity::Info,
    }
}

/// Sizes a whale payment's alert severity relative to the configured
/// threshold it cleared, since "large" is relative (10,000 XLM and a
/// 10,000-unit micro-cap token payment both clear the same native-unit
/// threshold, but one is far more likely to matter). A payment right at the
/// threshold is informational; one 20x+ the threshold is critical.
pub fn severity_for_whale_multiple(amount: Decimal, threshold: Decimal) -> Severity {
    if threshold.is_zero() {
        return Severity::Info;
    }
    let ratio = amount / threshold;
    if ratio >= Decimal::from(20) {
        Severity::Critical
    } else if ratio >= Decimal::from(5) {
        Severity::Warning
    } else {
        Severity::Info
    }
}

/// Severity for a pool liquidity drop, given how far past the configured
/// threshold the drop went. Callers only invoke this once a drop has already
/// cleared `threshold_pct` (see `logic::percent_change` +
/// `bin/ingest.rs::ingest_pool`), so unlike `severity_for_whale_multiple`
/// there's no `Info` case here — a sub-threshold drop isn't alert-worthy at
/// all, since pool liquidity fluctuates constantly in normal operation.
pub fn severity_for_liquidity_drop(drop_pct: Decimal, threshold_pct: Decimal) -> Severity {
    if drop_pct >= threshold_pct * Decimal::from(2) {
        Severity::Critical
    } else {
        Severity::Warning
    }
}

/// Records an alert, best-effort delivers it to the legacy single
/// `ALERT_WEBHOOK_URL` (if configured) and every DB-configured alert channel
/// (`alert_channels`) that meets its own minimum severity, and publishes a
/// live-event notification for `/events` SSE subscribers. Every delivery
/// step is best-effort and independent: a failure in one (bad URL, timeout,
/// Postgres NOTIFY hiccup) is logged and never propagated or allowed to
/// block another — alerting must never be able to take down ingestion, and
/// the row is already durably recorded regardless.
pub async fn record(
    pool: &PgPool,
    http: &reqwest::Client,
    config: &Config,
    kind: &str,
    severity: Severity,
    message: String,
    details: Value,
) -> anyhow::Result<()> {
    let now = Utc::now();
    db::insert_alert(pool, now, kind, severity.as_str(), &message, &details).await?;

    deliver_legacy_webhook(http, config, severity, &message).await;
    deliver_to_channels(pool, http, severity, &message).await;

    let event = json!({
        "type": "alert",
        "kind": kind,
        "severity": severity.as_str(),
        "message": message,
        "time": now,
    });
    if let Err(e) = db::notify_event(pool, crate::events::CHANNEL, &event.to_string()).await {
        tracing::debug!("failed to publish live-event notification: {e}");
    }

    Ok(())
}

async fn deliver_legacy_webhook(
    http: &reqwest::Client,
    config: &Config,
    severity: Severity,
    message: &str,
) {
    let Some(webhook_url) = &config.alert_webhook_url else {
        return;
    };
    let min_severity = Severity::parse(&config.alert_min_severity).unwrap_or(Severity::Warning);
    if severity < min_severity {
        return;
    }
    let body = json!({ "text": format!("[{}] {}", severity.as_str(), message) });
    if let Err(e) = http
        .post(webhook_url)
        .json(&body)
        .send()
        .await
        .and_then(|r| r.error_for_status())
    {
        tracing::warn!("legacy alert webhook delivery failed: {e}");
    }
}

/// Delivers to every enabled, DB-configured alert channel whose
/// `min_severity` this alert meets. One channel's failure never blocks the
/// others.
async fn deliver_to_channels(
    pool: &PgPool,
    http: &reqwest::Client,
    severity: Severity,
    message: &str,
) {
    let channels = match db::list_enabled_alert_channels(pool).await {
        Ok(c) => c,
        Err(e) => {
            tracing::warn!("failed to load alert channels: {e}");
            return;
        }
    };
    for channel in channels {
        let min_severity = Severity::parse(&channel.min_severity).unwrap_or(Severity::Warning);
        if severity < min_severity {
            continue;
        }
        let body = payload_for_channel(&channel.kind, severity, message);
        if let Err(e) = http
            .post(&channel.url)
            .json(&body)
            .send()
            .await
            .and_then(|r| r.error_for_status())
        {
            tracing::warn!("alert channel '{}' delivery failed: {e}", channel.name);
        }
    }
}

/// Shapes the outbound JSON body per channel kind: Slack/generic incoming
/// webhooks expect `{"text": ...}`; Discord's expect `{"content": ...}`. An
/// unrecognized kind falls back to the Slack/generic shape rather than
/// failing outright — most "generic" webhook receivers (including many
/// alerting/chat tools) accept that field name too.
fn payload_for_channel(kind: &str, severity: Severity, message: &str) -> Value {
    let text = format!("[{}] {}", severity.as_str(), message);
    match kind {
        "discord" => json!({ "content": text }),
        _ => json!({ "text": text }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn severity_ordering_lets_min_severity_gate_work() {
        assert!(Severity::Critical > Severity::Warning);
        assert!(Severity::Warning > Severity::Info);
    }

    #[test]
    fn parses_known_severities_case_insensitively() {
        assert_eq!(Severity::parse("warning"), Some(Severity::Warning));
        assert_eq!(Severity::parse("CRITICAL"), Some(Severity::Critical));
        assert_eq!(Severity::parse("bogus"), None);
    }

    #[test]
    fn risk_level_severity_mapping() {
        assert_eq!(severity_for_risk_level("CRITICAL"), Severity::Critical);
        assert_eq!(severity_for_risk_level("HIGH"), Severity::Warning);
        assert_eq!(severity_for_risk_level("MEDIUM"), Severity::Info);
        assert_eq!(severity_for_risk_level("LOW"), Severity::Info);
    }

    #[test]
    fn whale_multiple_severity_bands() {
        let threshold = Decimal::from(10_000);
        assert_eq!(
            severity_for_whale_multiple(Decimal::from(10_000), threshold),
            Severity::Info
        );
        assert_eq!(
            severity_for_whale_multiple(Decimal::from(60_000), threshold),
            Severity::Warning
        );
        assert_eq!(
            severity_for_whale_multiple(Decimal::from(250_000), threshold),
            Severity::Critical
        );
    }

    #[test]
    fn whale_multiple_zero_threshold_is_info() {
        assert_eq!(
            severity_for_whale_multiple(Decimal::from(1), Decimal::ZERO),
            Severity::Info
        );
    }

    #[test]
    fn liquidity_drop_at_threshold_is_warning() {
        assert_eq!(
            severity_for_liquidity_drop(Decimal::from(30), Decimal::from(30)),
            Severity::Warning
        );
    }

    #[test]
    fn liquidity_drop_at_double_threshold_is_critical() {
        assert_eq!(
            severity_for_liquidity_drop(Decimal::from(60), Decimal::from(30)),
            Severity::Critical
        );
        assert_eq!(
            severity_for_liquidity_drop(Decimal::from(90), Decimal::from(30)),
            Severity::Critical
        );
    }

    #[test]
    fn discord_payload_uses_content_field() {
        let body = payload_for_channel("discord", Severity::Critical, "test message");
        assert_eq!(body["content"], "[CRITICAL] test message");
        assert!(body.get("text").is_none());
    }

    #[test]
    fn slack_and_generic_payloads_use_text_field() {
        for kind in ["slack", "generic", "unknown-kind"] {
            let body = payload_for_channel(kind, Severity::Warning, "hi");
            assert_eq!(body["text"], "[WARNING] hi");
        }
    }
}
