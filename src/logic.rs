//! Pure, DB-free decision logic pulled out of the ingestion/API code paths so
//! it can be unit tested without a live Postgres instance.

use rust_decimal::Decimal;

/// Resolves a Horizon payment's asset into `(code, issuer)`, collapsing the
/// native-XLM case to the conventional "XLM" display code with no issuer.
pub fn resolve_payment_asset(
    asset_type: Option<&str>,
    asset_code: Option<&str>,
    asset_issuer: Option<&str>,
) -> (String, Option<String>) {
    match asset_type {
        Some("native") => ("XLM".to_string(), None),
        _ => (
            asset_code.unwrap_or("UNKNOWN").to_string(),
            asset_issuer.map(str::to_string),
        ),
    }
}

/// Whether a Horizon operation record represents a payment that meets the
/// whale threshold. Returns the parsed amount when it does.
pub fn whale_amount(op_type: &str, amount: Option<Decimal>, threshold: Decimal) -> Option<Decimal> {
    if op_type != "payment" {
        return None;
    }
    let amount = amount?;
    if amount < threshold {
        return None;
    }
    Some(amount)
}

/// Percent change in a monotonically-tracked quantity (e.g. pool
/// `total_shares`) between two points in time. `None` when the starting
/// value is zero, since percent growth from zero is undefined.
pub fn percent_change(before: Decimal, now: Decimal) -> Option<Decimal> {
    if before.is_zero() {
        return None;
    }
    Some((now - before) / before * Decimal::from(100))
}

/// Derives `(ltv, health_factor)` for a lending position from its collateral
/// and debt amounts plus a USD price for each asset, when both are known
/// (see `Config::blend_asset_prices_usd`). `ltv` is debt/collateral as a
/// percentage; `health_factor` here is a simplified collateral/debt
/// coverage ratio — a real proxy for risk, but not Blend's own protocol
/// formula, which additionally weights each reserve by its own
/// collateral/liability factor (risk parameters this crate doesn't have a
/// source for). Returns `(None, None)` when either asset's price is
/// unknown, or when collateral is zero (LTV is undefined, not infinite/0).
/// When debt is zero, `ltv` is `Some(0)` but `health_factor` is `None`
/// (no debt to be at risk of, so a coverage ratio doesn't apply).
pub fn compute_lending_risk(
    collateral_amount: Decimal,
    collateral_price_usd: Option<Decimal>,
    debt_amount: Decimal,
    debt_price_usd: Option<Decimal>,
) -> (Option<Decimal>, Option<Decimal>) {
    let (Some(collateral_price), Some(debt_price)) = (collateral_price_usd, debt_price_usd) else {
        return (None, None);
    };
    let collateral_usd = collateral_amount * collateral_price;
    let debt_usd = debt_amount * debt_price;
    if collateral_usd.is_zero() {
        return (None, None);
    }
    let ltv = (debt_usd / collateral_usd) * Decimal::from(100);
    let health_factor = if debt_usd.is_zero() {
        None
    } else {
        Some(collateral_usd / debt_usd)
    };
    (Some(ltv), health_factor)
}

/// Loan-to-value risk banding used by the liquidation dashboard. `ltv` is a
/// percentage (0-100+, values above 100 are already technically insolvent).
pub fn risk_level(ltv: Decimal) -> &'static str {
    if ltv >= Decimal::from(95) {
        "CRITICAL"
    } else if ltv >= Decimal::from(85) {
        "HIGH"
    } else if ltv >= Decimal::from(70) {
        "MEDIUM"
    } else {
        "LOW"
    }
}

/// Total order over `risk_level`'s output, used to detect whether a
/// position's risk band moved *up* (worth alerting on) vs. sideways/down.
pub fn risk_rank(level: &str) -> u8 {
    match level {
        "CRITICAL" => 3,
        "HIGH" => 2,
        "MEDIUM" => 1,
        _ => 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    fn dec(s: &str) -> Decimal {
        Decimal::from_str(s).unwrap()
    }

    #[test]
    fn native_asset_resolves_to_xlm() {
        assert_eq!(
            resolve_payment_asset(Some("native"), None, None),
            ("XLM".to_string(), None)
        );
    }

    #[test]
    fn issued_asset_resolves_to_code_and_issuer() {
        assert_eq!(
            resolve_payment_asset(Some("credit_alphanum4"), Some("USDC"), Some("GISSUER")),
            ("USDC".to_string(), Some("GISSUER".to_string()))
        );
    }

    #[test]
    fn missing_asset_code_falls_back_to_unknown() {
        assert_eq!(
            resolve_payment_asset(Some("credit_alphanum4"), None, Some("GISSUER")),
            ("UNKNOWN".to_string(), Some("GISSUER".to_string()))
        );
    }

    #[test]
    fn whale_amount_rejects_non_payment_ops() {
        assert_eq!(
            whale_amount("create_account", Some(dec("999999")), dec("100")),
            None
        );
    }

    #[test]
    fn whale_amount_rejects_below_threshold() {
        assert_eq!(whale_amount("payment", Some(dec("50")), dec("100")), None);
    }

    #[test]
    fn whale_amount_accepts_at_or_above_threshold() {
        assert_eq!(
            whale_amount("payment", Some(dec("100")), dec("100")),
            Some(dec("100"))
        );
        assert_eq!(
            whale_amount("payment", Some(dec("150")), dec("100")),
            Some(dec("150"))
        );
    }

    #[test]
    fn whale_amount_handles_missing_amount() {
        assert_eq!(whale_amount("payment", None, dec("100")), None);
    }

    #[test]
    fn percent_change_zero_baseline_is_undefined() {
        assert_eq!(percent_change(dec("0"), dec("100")), None);
    }

    #[test]
    fn percent_change_computes_growth() {
        assert_eq!(percent_change(dec("100"), dec("150")), Some(dec("50")));
    }

    #[test]
    fn percent_change_computes_shrinkage() {
        assert_eq!(percent_change(dec("200"), dec("100")), Some(dec("-50")));
    }

    #[test]
    fn lending_risk_none_when_either_price_unknown() {
        assert_eq!(
            compute_lending_risk(dec("100"), None, dec("50"), Some(dec("1"))),
            (None, None)
        );
        assert_eq!(
            compute_lending_risk(dec("100"), Some(dec("1")), dec("50"), None),
            (None, None)
        );
    }

    #[test]
    fn lending_risk_computes_ltv_and_health_factor() {
        // 100 collateral @ $1 = $100; 50 debt @ $1 = $50 => LTV 50%, HF 2.0.
        let (ltv, hf) = compute_lending_risk(dec("100"), Some(dec("1")), dec("50"), Some(dec("1")));
        assert_eq!(ltv, Some(dec("50")));
        assert_eq!(hf, Some(dec("2")));
    }

    #[test]
    fn lending_risk_zero_collateral_is_undefined() {
        assert_eq!(
            compute_lending_risk(dec("0"), Some(dec("1")), dec("50"), Some(dec("1"))),
            (None, None)
        );
    }

    #[test]
    fn lending_risk_zero_debt_has_no_health_factor() {
        let (ltv, hf) = compute_lending_risk(dec("100"), Some(dec("1")), dec("0"), Some(dec("1")));
        assert_eq!(ltv, Some(dec("0")));
        assert_eq!(hf, None);
    }

    #[test]
    fn risk_level_bands() {
        assert_eq!(risk_level(dec("96")), "CRITICAL");
        assert_eq!(risk_level(dec("95")), "CRITICAL");
        assert_eq!(risk_level(dec("90")), "HIGH");
        assert_eq!(risk_level(dec("75")), "MEDIUM");
        assert_eq!(risk_level(dec("50")), "LOW");
    }

    #[test]
    fn risk_rank_orders_bands_low_to_critical() {
        assert!(risk_rank("CRITICAL") > risk_rank("HIGH"));
        assert!(risk_rank("HIGH") > risk_rank("MEDIUM"));
        assert!(risk_rank("MEDIUM") > risk_rank("LOW"));
    }
}
