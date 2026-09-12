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
    fn risk_level_bands() {
        assert_eq!(risk_level(dec("96")), "CRITICAL");
        assert_eq!(risk_level(dec("95")), "CRITICAL");
        assert_eq!(risk_level(dec("90")), "HIGH");
        assert_eq!(risk_level(dec("75")), "MEDIUM");
        assert_eq!(risk_level(dec("50")), "LOW");
    }
}
