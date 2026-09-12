//! Minimal client for the public Stellar Horizon HTTP API.
//! Docs: https://developers.stellar.org/docs/data/horizon

use std::time::Duration;

use serde::Deserialize;

const MAX_RETRIES: u32 = 5;
const INITIAL_BACKOFF: Duration = Duration::from_millis(250);

pub struct HorizonClient {
    http: reqwest::Client,
    base_url: String,
}

#[derive(Debug, Deserialize)]
pub struct Page<T> {
    #[serde(rename = "_links", default)]
    pub links: Links,
    #[serde(rename = "_embedded")]
    pub embedded: Embedded<T>,
}

#[derive(Debug, Deserialize, Default)]
pub struct Links {
    pub next: Option<Link>,
}

#[derive(Debug, Deserialize)]
pub struct Link {
    pub href: String,
}

#[derive(Debug, Deserialize)]
pub struct Embedded<T> {
    pub records: Vec<T>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct PoolReserve {
    pub asset: String,
    pub amount: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct LiquidityPool {
    pub id: String,
    pub fee_bp: i32,
    pub total_trustlines: String,
    pub total_shares: String,
    pub reserves: Vec<PoolReserve>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct AssetAccounts {
    pub authorized: i32,
}

#[derive(Debug, Deserialize, Clone)]
pub struct AssetBalances {
    pub authorized: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct AssetRecord {
    pub asset_code: String,
    pub asset_issuer: String,
    pub asset_type: String,
    pub accounts: AssetAccounts,
    pub balances: AssetBalances,
    #[serde(default)]
    pub num_claimable_balances: i32,
}

#[derive(Debug, Deserialize, Clone)]
pub struct PaymentRecord {
    pub id: String,
    pub paging_token: String,
    #[serde(rename = "type")]
    pub op_type: String,
    pub transaction_hash: String,
    #[serde(default)]
    pub from: Option<String>,
    #[serde(default)]
    pub to: Option<String>,
    #[serde(default)]
    pub source_account: Option<String>,
    #[serde(default)]
    pub asset_type: Option<String>,
    #[serde(default)]
    pub asset_code: Option<String>,
    #[serde(default)]
    pub asset_issuer: Option<String>,
    #[serde(default)]
    pub amount: Option<String>,
}

impl HorizonClient {
    pub fn new(base_url: impl Into<String>) -> Self {
        Self {
            http: reqwest::Client::builder()
                .user_agent("lumina-analytics/0.1")
                .timeout(Duration::from_secs(30))
                .build()
                .expect("failed to build reqwest client"),
            base_url: base_url.into(),
        }
    }

    /// Fetches one page and returns both the records and the `next` link, so
    /// callers can decide whether/how to keep paginating.
    async fn get_page<T: for<'de> Deserialize<'de>>(&self, url: &str) -> anyhow::Result<Page<T>> {
        let mut attempt = 0u32;
        let mut backoff = INITIAL_BACKOFF;
        loop {
            attempt += 1;
            let result = self.http.get(url).send().await;
            match result {
                Ok(resp) if resp.status().is_success() => {
                    return Ok(resp.json::<Page<T>>().await?);
                }
                Ok(resp) => {
                    let status = resp.status();
                    let retryable = status.is_server_error() || status.as_u16() == 429;
                    if retryable && attempt < MAX_RETRIES {
                        tracing::warn!(
                            "horizon request to {url} failed with {status}, retrying in {backoff:?} (attempt {attempt}/{MAX_RETRIES})"
                        );
                        tokio::time::sleep(backoff).await;
                        backoff *= 2;
                        continue;
                    }
                    anyhow::bail!("horizon request to {url} failed with {status}");
                }
                Err(e) => {
                    let retryable = e.is_timeout() || e.is_connect() || e.is_request();
                    if retryable && attempt < MAX_RETRIES {
                        tracing::warn!(
                            "horizon request to {url} errored ({e}), retrying in {backoff:?} (attempt {attempt}/{MAX_RETRIES})"
                        );
                        tokio::time::sleep(backoff).await;
                        backoff *= 2;
                        continue;
                    }
                    return Err(e.into());
                }
            }
        }
    }

    fn build_url(&self, path: &str, query: &[(&str, &str)]) -> String {
        let mut url = format!("{}{}", self.base_url, path);
        if !query.is_empty() {
            let qs: Vec<String> = query.iter().map(|(k, v)| format!("{k}={v}")).collect();
            url.push('?');
            url.push_str(&qs.join("&"));
        }
        url
    }

    /// Follows `_links.next` up to `max_pages` pages (or until a page comes
    /// back empty), collecting every record along the way. Horizon's `next`
    /// link is a stable, self-contained URL, so no extra escaping is needed.
    async fn get_all_pages<T: for<'de> Deserialize<'de>>(
        &self,
        path: &str,
        query: &[(&str, &str)],
        max_pages: u32,
    ) -> anyhow::Result<Vec<T>> {
        let mut out = Vec::new();
        let mut url = self.build_url(path, query);
        for _ in 0..max_pages {
            let page = self.get_page::<T>(&url).await?;
            let got = page.embedded.records.len();
            out.extend(page.embedded.records);
            match page.links.next {
                Some(next) if got > 0 => url = next.href,
                _ => break,
            }
        }
        Ok(out)
    }

    /// Fetch up to `max_pages * 200` liquidity pools ordered by most recently
    /// active, paginating through Horizon's cursor-based `next` links.
    pub async fn liquidity_pools(&self, max_pages: u32) -> anyhow::Result<Vec<LiquidityPool>> {
        self.get_all_pages(
            "/liquidity_pools",
            &[("limit", "200"), ("order", "desc")],
            max_pages,
        )
        .await
    }

    /// Fetch up to `max_pages * 200` assets ordered by number of holding
    /// accounts, paginating through Horizon's cursor-based `next` links.
    pub async fn assets(&self, max_pages: u32) -> anyhow::Result<Vec<AssetRecord>> {
        self.get_all_pages("/assets", &[("limit", "200"), ("order", "desc")], max_pages)
            .await
    }

    /// Fetch payment operations strictly after `cursor` (Horizon's
    /// paging-token semantics), oldest first, paginating forward until
    /// caught up or `max_pages` is hit. `cursor: None` starts from genesis —
    /// callers doing a first run should seed a cursor instead of calling this
    /// unbounded.
    pub async fn payments_since(
        &self,
        cursor: Option<&str>,
        max_pages: u32,
    ) -> anyhow::Result<Vec<PaymentRecord>> {
        let mut query = vec![
            ("limit", "200"),
            ("order", "asc"),
            ("include_failed", "false"),
        ];
        if let Some(c) = cursor {
            query.push(("cursor", c));
        }
        self.get_all_pages("/payments", &query, max_pages).await
    }

    /// Fetch the most recent page of payments, newest first — used only to
    /// seed an initial cursor so a first run doesn't backfill full history.
    pub async fn latest_payments_page(&self, limit: u32) -> anyhow::Result<Vec<PaymentRecord>> {
        let url = self.build_url(
            "/payments",
            &[
                ("limit", &limit.to_string()),
                ("order", "desc"),
                ("include_failed", "false"),
            ],
        );
        Ok(self.get_page::<PaymentRecord>(&url).await?.embedded.records)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn page_deserializes_next_link() {
        let json = r#"{
            "_links": { "next": { "href": "https://horizon.stellar.org/payments?cursor=123" } },
            "_embedded": { "records": [] }
        }"#;
        let page: Page<PaymentRecord> = serde_json::from_str(json).unwrap();
        assert_eq!(
            page.links.next.unwrap().href,
            "https://horizon.stellar.org/payments?cursor=123"
        );
    }

    #[test]
    fn page_deserializes_missing_links() {
        let json = r#"{ "_embedded": { "records": [] } }"#;
        let page: Page<PaymentRecord> = serde_json::from_str(json).unwrap();
        assert!(page.links.next.is_none());
    }

    #[test]
    fn build_url_appends_query() {
        let client = HorizonClient::new("https://horizon.stellar.org");
        let url = client.build_url("/payments", &[("limit", "200"), ("order", "asc")]);
        assert_eq!(
            url,
            "https://horizon.stellar.org/payments?limit=200&order=asc"
        );
    }

    #[test]
    fn build_url_no_query() {
        let client = HorizonClient::new("https://horizon.stellar.org");
        let url = client.build_url("/payments", &[]);
        assert_eq!(url, "https://horizon.stellar.org/payments");
    }
}
