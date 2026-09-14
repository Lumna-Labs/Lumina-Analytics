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

/// Records an alert and, if a webhook is configured and this alert meets the
/// configured minimum severity, best-effort delivers it there. A webhook
/// failure (timeout, non-2xx, DNS) is logged and never propagated — alerting
/// must never be able to take down ingestion.
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

    let Some(webhook_url) = &config.alert_webhook_url else {
        return Ok(());
    };
    let min_severity = Severity::parse(&config.alert_min_severity).unwrap_or(Severity::Warning);
    if severity < min_severity {
        return Ok(());
    }

    let body = json!({ "text": format!("[{}] {}", severity.as_str(), message) });
    if let Err(e) = http
        .post(webhook_url)
        .json(&body)
        .send()
        .await
        .and_then(|r| r.error_for_status())
    {
        tracing::warn!("alert webhook delivery failed: {e}");
    }
    Ok(())
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
}
