//! USD pricing for Stellar assets.
//!
//! There is no single oracle for "the" USD price of an arbitrary Stellar
//! asset, so this derives one the same way an on-chain observer would:
//! 1. fetch XLM/USD from an external feed (CoinGecko by default), and
//! 2. for any asset that trades against native XLM in a classic AMM pool,
//!    derive its USD price from that pool's reserve ratio.
//!
//! Assets that never pair with XLM in a pool go unpriced (`None`) rather
//! than guessed — matching this codebase's existing preference for an
//! honest gap over an invented number.

use rust_decimal::Decimal;
use serde::Deserialize;
use std::str::FromStr;

/// A Stellar asset identity as used across this crate's persisted tables:
/// `"XLM"` / `""` for native, or the issued asset's code/issuer otherwise.
pub type AssetId = (String, String);

/// Parses Horizon's pool-reserve asset representation (`"native"` or
/// `"CODE:ISSUER"`) into `(code, issuer)`, matching the convention used in
/// the `tokens`/`asset_prices` tables (native = `("XLM", "")`).
pub fn parse_pool_asset(asset: &str) -> AssetId {
    if asset == "native" {
        return ("XLM".to_string(), String::new());
    }
    match asset.split_once(':') {
        Some((code, issuer)) => (code.to_string(), issuer.to_string()),
        None => (asset.to_string(), String::new()),
    }
}

/// Given one pool's two reserve legs, derives a USD price for whichever leg
/// isn't native XLM, along with the native-side reserve depth (used by
/// callers to prefer the deepest pool when an asset trades in several).
/// Returns `None` for pools with no native leg — pricing those would
/// require routing through another priced asset, which isn't implemented.
pub fn price_from_native_pair(
    asset_a: &str,
    reserve_a: Decimal,
    asset_b: &str,
    reserve_b: Decimal,
    xlm_usd: Decimal,
) -> Option<(AssetId, Decimal, Decimal)> {
    if asset_a == "native" && asset_b == "native" {
        return None;
    }
    if asset_a == "native" {
        if reserve_b.is_zero() {
            return None;
        }
        let price = (reserve_a / reserve_b) * xlm_usd;
        return Some((parse_pool_asset(asset_b), price, reserve_a));
    }
    if asset_b == "native" {
        if reserve_a.is_zero() {
            return None;
        }
        let price = (reserve_b / reserve_a) * xlm_usd;
        return Some((parse_pool_asset(asset_a), price, reserve_b));
    }
    None
}

#[derive(Debug, Deserialize)]
struct CoinGeckoStellar {
    usd: f64,
}

#[derive(Debug, Deserialize)]
struct CoinGeckoResponse {
    stellar: CoinGeckoStellar,
}

/// Fetches the current XLM/USD spot price from CoinGecko's free public API.
pub async fn fetch_xlm_usd(http: &reqwest::Client, api_url: &str) -> anyhow::Result<Decimal> {
    let resp: CoinGeckoResponse = http
        .get(api_url)
        .query(&[("ids", "stellar"), ("vs_currencies", "usd")])
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    Decimal::from_str(&resp.stellar.usd.to_string()).map_err(Into::into)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    fn dec(s: &str) -> Decimal {
        Decimal::from_str(s).unwrap()
    }

    #[test]
    fn parses_native_asset() {
        assert_eq!(
            parse_pool_asset("native"),
            ("XLM".to_string(), String::new())
        );
    }

    #[test]
    fn parses_issued_asset() {
        assert_eq!(
            parse_pool_asset("USDC:GISSUER123"),
            ("USDC".to_string(), "GISSUER123".to_string())
        );
    }

    #[test]
    fn prices_asset_b_against_native_a() {
        // Pool: 1000 XLM / 200 USDC, XLM = $0.10 => USDC priced at $0.50.
        let result =
            price_from_native_pair("native", dec("1000"), "USDC:GISS", dec("200"), dec("0.10"));
        let (asset, price, weight) = result.unwrap();
        assert_eq!(asset, ("USDC".to_string(), "GISS".to_string()));
        assert_eq!(price, dec("0.50"));
        assert_eq!(weight, dec("1000"));
    }

    #[test]
    fn prices_asset_a_against_native_b() {
        let result =
            price_from_native_pair("USDC:GISS", dec("200"), "native", dec("1000"), dec("0.10"));
        let (asset, price, weight) = result.unwrap();
        assert_eq!(asset, ("USDC".to_string(), "GISS".to_string()));
        assert_eq!(price, dec("0.50"));
        assert_eq!(weight, dec("1000"));
    }

    #[test]
    fn no_native_leg_returns_none() {
        assert_eq!(
            price_from_native_pair(
                "USDC:GISS",
                dec("200"),
                "AQUA:GISS2",
                dec("300"),
                dec("0.10")
            ),
            None
        );
    }

    #[test]
    fn both_native_returns_none() {
        assert_eq!(
            price_from_native_pair("native", dec("100"), "native", dec("100"), dec("0.10")),
            None
        );
    }

    #[test]
    fn zero_reserve_returns_none() {
        assert_eq!(
            price_from_native_pair("native", dec("100"), "USDC:GISS", dec("0"), dec("0.10")),
            None
        );
    }
}
