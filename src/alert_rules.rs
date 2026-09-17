//! Pure decision logic for operator-configurable alert-rule overrides
//! (`alert_rules` table, managed via `/alert-rules`), layered on top of the
//! global `WHALE_THRESHOLD` config value and the previously-hardcoded LTV
//! risk bands. Kept dependency-free (just the row type) so it's unit
//! testable without a live Postgres instance, matching `logic`.

use rust_decimal::Decimal;

use crate::models::AlertRuleRow;

/// Picks the whale-payment threshold for one asset: an enabled
/// `whale_threshold` rule matching this exact asset (code + issuer, where
/// `None` issuer means native XLM) overrides `default`. If more than one
/// enabled rule matches the same asset, the most recently created one wins
/// — an operator fixing a typo by adding a second rule shouldn't need to
/// remember to delete the first.
pub fn resolve_whale_threshold(
    rules: &[AlertRuleRow],
    asset_code: &str,
    asset_issuer: Option<&str>,
    default: Decimal,
) -> Decimal {
    rules
        .iter()
        .filter(|r| r.enabled && r.rule_type == "whale_threshold")
        .filter(|r| r.asset_code.as_deref() == Some(asset_code))
        .filter(|r| r.asset_issuer.as_deref() == asset_issuer)
        .max_by_key(|r| r.id)
        .map(|r| r.threshold)
        .unwrap_or(default)
}

/// Picks the holder-drop alert threshold (a percent) for one asset: an
/// enabled `holder_drop_pct` rule matching this exact asset (code + issuer,
/// where `None` issuer means native XLM — though XLM has no meaningful
/// "holder count" via this path in practice) overrides `default`. Same
/// most-recent-wins tiebreak as `resolve_whale_threshold`.
pub fn resolve_holder_drop_threshold_pct(
    rules: &[AlertRuleRow],
    asset_code: &str,
    asset_issuer: Option<&str>,
    default: Decimal,
) -> Decimal {
    rules
        .iter()
        .filter(|r| r.enabled && r.rule_type == "holder_drop_pct")
        .filter(|r| r.asset_code.as_deref() == Some(asset_code))
        .filter(|r| r.asset_issuer.as_deref() == asset_issuer)
        .max_by_key(|r| r.id)
        .map(|r| r.threshold)
        .unwrap_or(default)
}

/// LTV percentage cutoffs for each risk band. Defaults match the values
/// `logic::risk_level` used to hardcode (70/85/95) before this module
/// existed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LtvBands {
    pub medium: Decimal,
    pub high: Decimal,
    pub critical: Decimal,
}

impl Default for LtvBands {
    fn default() -> Self {
        Self {
            medium: Decimal::from(70),
            high: Decimal::from(85),
            critical: Decimal::from(95),
        }
    }
}

/// Applies enabled `ltv_band` rules (named `MEDIUM`/`HIGH`/`CRITICAL`) on top
/// of the hardcoded defaults, so an operator can tune liquidation-risk
/// banding without a code change. An unrecognized `name` is ignored rather
/// than erroring, so one bad rule can't break banding for the others.
pub fn resolve_ltv_bands(rules: &[AlertRuleRow]) -> LtvBands {
    let mut bands = LtvBands::default();
    for r in rules
        .iter()
        .filter(|r| r.enabled && r.rule_type == "ltv_band")
    {
        match r.name.to_ascii_uppercase().as_str() {
            "MEDIUM" => bands.medium = r.threshold,
            "HIGH" => bands.high = r.threshold,
            "CRITICAL" => bands.critical = r.threshold,
            _ => {}
        }
    }
    bands
}

/// Same banding as `logic::risk_level` but against configurable cutoffs.
pub fn risk_level_with_bands(ltv: Decimal, bands: &LtvBands) -> &'static str {
    if ltv >= bands.critical {
        "CRITICAL"
    } else if ltv >= bands.high {
        "HIGH"
    } else if ltv >= bands.medium {
        "MEDIUM"
    } else {
        "LOW"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use std::str::FromStr;

    fn dec(s: &str) -> Decimal {
        Decimal::from_str(s).unwrap()
    }

    fn whale_rule(
        id: i64,
        code: &str,
        issuer: Option<&str>,
        threshold: &str,
        enabled: bool,
    ) -> AlertRuleRow {
        AlertRuleRow {
            id,
            name: format!("rule-{id}"),
            rule_type: "whale_threshold".to_string(),
            asset_code: Some(code.to_string()),
            asset_issuer: issuer.map(str::to_string),
            threshold: dec(threshold),
            enabled,
            created_at: Utc::now(),
        }
    }

    fn holder_drop_rule(
        id: i64,
        code: &str,
        issuer: Option<&str>,
        threshold: &str,
        enabled: bool,
    ) -> AlertRuleRow {
        AlertRuleRow {
            id,
            name: format!("rule-{id}"),
            rule_type: "holder_drop_pct".to_string(),
            asset_code: Some(code.to_string()),
            asset_issuer: issuer.map(str::to_string),
            threshold: dec(threshold),
            enabled,
            created_at: Utc::now(),
        }
    }

    fn band_rule(id: i64, name: &str, threshold: &str, enabled: bool) -> AlertRuleRow {
        AlertRuleRow {
            id,
            name: name.to_string(),
            rule_type: "ltv_band".to_string(),
            asset_code: None,
            asset_issuer: None,
            threshold: dec(threshold),
            enabled,
            created_at: Utc::now(),
        }
    }

    #[test]
    fn falls_back_to_default_with_no_matching_rule() {
        let rules = vec![whale_rule(1, "USDC", Some("GISSUER"), "5000", true)];
        assert_eq!(
            resolve_whale_threshold(&rules, "XLM", None, dec("10000")),
            dec("10000")
        );
    }

    #[test]
    fn matching_rule_overrides_default() {
        let rules = vec![whale_rule(1, "USDC", Some("GISSUER"), "5000", true)];
        assert_eq!(
            resolve_whale_threshold(&rules, "USDC", Some("GISSUER"), dec("10000")),
            dec("5000")
        );
    }

    #[test]
    fn disabled_rule_is_ignored() {
        let rules = vec![whale_rule(1, "USDC", Some("GISSUER"), "5000", false)];
        assert_eq!(
            resolve_whale_threshold(&rules, "USDC", Some("GISSUER"), dec("10000")),
            dec("10000")
        );
    }

    #[test]
    fn different_issuer_does_not_match() {
        let rules = vec![whale_rule(1, "USDC", Some("GISSUER_A"), "5000", true)];
        assert_eq!(
            resolve_whale_threshold(&rules, "USDC", Some("GISSUER_B"), dec("10000")),
            dec("10000")
        );
    }

    #[test]
    fn native_asset_matches_none_issuer() {
        let rules = vec![whale_rule(1, "XLM", None, "50000", true)];
        assert_eq!(
            resolve_whale_threshold(&rules, "XLM", None, dec("10000")),
            dec("50000")
        );
    }

    #[test]
    fn most_recent_matching_rule_wins() {
        let rules = vec![
            whale_rule(1, "USDC", Some("G"), "1000", true),
            whale_rule(2, "USDC", Some("G"), "2000", true),
        ];
        assert_eq!(
            resolve_whale_threshold(&rules, "USDC", Some("G"), dec("10000")),
            dec("2000")
        );
    }

    #[test]
    fn holder_drop_falls_back_to_default_with_no_matching_rule() {
        let rules = vec![holder_drop_rule(1, "USDC", Some("GISSUER"), "10", true)];
        assert_eq!(
            resolve_holder_drop_threshold_pct(&rules, "SHIB", Some("GOTHER"), dec("20")),
            dec("20")
        );
    }

    #[test]
    fn holder_drop_matching_rule_overrides_default() {
        let rules = vec![holder_drop_rule(1, "USDC", Some("GISSUER"), "10", true)];
        assert_eq!(
            resolve_holder_drop_threshold_pct(&rules, "USDC", Some("GISSUER"), dec("20")),
            dec("10")
        );
    }

    #[test]
    fn holder_drop_disabled_rule_is_ignored() {
        let rules = vec![holder_drop_rule(1, "USDC", Some("GISSUER"), "10", false)];
        assert_eq!(
            resolve_holder_drop_threshold_pct(&rules, "USDC", Some("GISSUER"), dec("20")),
            dec("20")
        );
    }

    #[test]
    fn holder_drop_does_not_match_a_whale_threshold_rule_on_the_same_asset() {
        let rules = vec![whale_rule(1, "USDC", Some("GISSUER"), "5000", true)];
        assert_eq!(
            resolve_holder_drop_threshold_pct(&rules, "USDC", Some("GISSUER"), dec("20")),
            dec("20")
        );
    }

    #[test]
    fn ltv_bands_default_when_no_rules() {
        assert_eq!(resolve_ltv_bands(&[]), LtvBands::default());
    }

    #[test]
    fn ltv_bands_apply_enabled_overrides() {
        let rules = vec![
            band_rule(1, "HIGH", "80", true),
            band_rule(2, "CRITICAL", "90", true),
            band_rule(3, "MEDIUM", "60", false), // disabled, should not apply
        ];
        let bands = resolve_ltv_bands(&rules);
        assert_eq!(bands.high, dec("80"));
        assert_eq!(bands.critical, dec("90"));
        assert_eq!(bands.medium, dec("70")); // default, since that rule is disabled
    }

    #[test]
    fn ltv_bands_ignores_unknown_name() {
        let rules = vec![band_rule(1, "BOGUS", "10", true)];
        assert_eq!(resolve_ltv_bands(&rules), LtvBands::default());
    }

    #[test]
    fn risk_level_with_bands_matches_defaults() {
        let bands = LtvBands::default();
        assert_eq!(risk_level_with_bands(dec("96"), &bands), "CRITICAL");
        assert_eq!(risk_level_with_bands(dec("90"), &bands), "HIGH");
        assert_eq!(risk_level_with_bands(dec("75"), &bands), "MEDIUM");
        assert_eq!(risk_level_with_bands(dec("50"), &bands), "LOW");
    }

    #[test]
    fn risk_level_with_custom_bands() {
        let bands = LtvBands {
            medium: dec("50"),
            high: dec("60"),
            critical: dec("70"),
        };
        assert_eq!(risk_level_with_bands(dec("65"), &bands), "HIGH");
        assert_eq!(risk_level_with_bands(dec("40"), &bands), "LOW");
    }
}
