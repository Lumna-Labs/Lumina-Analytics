use std::str::FromStr;
use std::time::Duration as StdDuration;

use chrono::Utc;
use rust_decimal::Decimal;
use serde_json::json;
use sqlx::PgPool;

use lumina::alerts::{self, Severity};
use lumina::config::Config;
use lumina::db;
use lumina::horizon::{HorizonClient, LiquidityPool, PaymentRecord};
use lumina::logic;
use lumina::pricing;
use lumina::soroban;

/// An alert-worthy event detected mid-transaction, deferred until after
/// `tx.commit()` so recording it (a DB write plus an optional outbound
/// webhook call) never happens while a batch-ingest transaction is open.
struct PendingAlert {
    kind: &'static str,
    severity: Severity,
    message: String,
    details: serde_json::Value,
}

const PAYMENTS_CURSOR_KEY: &str = "horizon_payments_cursor";
/// Safety cap on pages fetched per cycle so a huge backlog (e.g. after
/// downtime) can't turn one poll tick into an unbounded crawl; the next tick
/// picks up where this one left off since the cursor is persisted per page.
const MAX_PAGES_PER_CYCLE: u32 = 25;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let config = Config::from_env();
    let pool = db::connect(&config.database_url).await?;
    db::migrate(&pool).await?;

    let horizon = HorizonClient::new(config.horizon_url.clone());
    let http = reqwest::Client::builder()
        .user_agent("lumina-analytics/0.1")
        .timeout(StdDuration::from_secs(15))
        .build()?;
    let mut interval = tokio::time::interval(StdDuration::from_secs(config.poll_interval_secs));

    tracing::info!(
        "lumina-ingest starting, polling {} every {}s",
        config.horizon_url,
        config.poll_interval_secs
    );

    let blend_pools = config.blend_pool_ids();
    if !blend_pools.is_empty() {
        tracing::info!(
            "lending ingestion enabled for {} Blend pool contract(s)",
            blend_pools.len()
        );
    }

    loop {
        interval.tick().await;
        if let Err(e) = run_once(&pool, &horizon, &http, &config).await {
            tracing::error!("ingestion cycle failed: {e:?}");
        }
        if !blend_pools.is_empty() {
            if let Err(e) = soroban::ingest_lending_cycle(&pool, &http, &config, &blend_pools).await
            {
                tracing::error!("lending ingestion cycle failed: {e:?}");
            }
        }
    }
}

async fn run_once(
    pool: &PgPool,
    horizon: &HorizonClient,
    http: &reqwest::Client,
    config: &Config,
) -> anyhow::Result<()> {
    let now = Utc::now();

    let pools = horizon.liquidity_pools(10).await?;
    tracing::info!("fetched {} liquidity pools", pools.len());
    let assets = horizon.assets(10).await?;
    tracing::info!("fetched {} assets", assets.len());

    // One transaction for this cycle's pool/token/price writes. Committing
    // each row separately (the original approach) means one fsync per row —
    // fine at 200 rows, but pagination can now bring in thousands, and a
    // partial write on error would leave `pools`/`tokens` referencing a
    // snapshot that only half-landed. Batching into a single transaction
    // fixes both: one fsync for the whole cycle, and all-or-nothing writes.
    let mut tx = pool.begin().await?;

    for lp in &pools {
        if let Err(e) = ingest_pool(&mut tx, now, lp).await {
            tracing::warn!("skipping pool {}: {e:?}", lp.id);
        }
    }

    if config.price_feed_enabled {
        if let Err(e) = ingest_prices(&mut tx, http, config, now, &pools).await {
            tracing::warn!("price ingestion skipped this cycle: {e:?}");
        }
    }

    for a in &assets {
        let amount = Decimal::from_str(&a.balances.authorized).unwrap_or_default();
        db::upsert_token(&mut *tx, &a.asset_code, &a.asset_issuer).await?;
        db::insert_token_snapshot(
            &mut *tx,
            now,
            &a.asset_code,
            &a.asset_issuer,
            amount,
            a.accounts.authorized,
            a.num_claimable_balances,
        )
        .await?;
    }

    tx.commit().await?;

    ingest_payments(pool, horizon, http, config).await?;

    Ok(())
}

/// Walks Horizon payments forward from the last-persisted cursor so no
/// whale-sized payment is missed, regardless of how many payments occurred
/// during the poll interval. On first run (no cursor yet) it seeds the
/// cursor at the current tip instead of backfilling the network's entire
/// payment history.
async fn ingest_payments(
    pool: &PgPool,
    horizon: &HorizonClient,
    http: &reqwest::Client,
    config: &Config,
) -> anyhow::Result<()> {
    let cursor = db::get_ingest_cursor(pool, PAYMENTS_CURSOR_KEY).await?;

    let Some(cursor) = cursor else {
        let latest = horizon.latest_payments_page(1).await?;
        if let Some(newest) = latest.first() {
            db::set_ingest_cursor(pool, PAYMENTS_CURSOR_KEY, &newest.paging_token).await?;
            tracing::info!(
                "seeded payments cursor at {} (no backfill on first run)",
                newest.paging_token
            );
        }
        return Ok(());
    };

    let payments = horizon
        .payments_since(Some(&cursor), MAX_PAGES_PER_CYCLE)
        .await?;
    if payments.is_empty() {
        return Ok(());
    }
    tracing::info!("fetched {} new payments since cursor", payments.len());

    // Single transaction: the cursor advance must commit atomically with the
    // whale rows it depends on, or a crash mid-cycle could either replay
    // duplicate inserts (harmless, they're deduped by op_id) or, worse,
    // advance the cursor past payments that never got written.
    let mut tx = pool.begin().await?;
    let mut pending_alerts = Vec::new();
    let mut last_cursor = cursor;
    for p in &payments {
        if let Some(alert) = ingest_whale_payment(&mut tx, p, config.whale_threshold).await? {
            pending_alerts.push(alert);
        }
        last_cursor = p.paging_token.clone();
    }
    db::set_ingest_cursor(&mut *tx, PAYMENTS_CURSOR_KEY, &last_cursor).await?;
    tx.commit().await?;

    tracing::info!(
        "recorded {} whale-sized payment(s) this cycle",
        pending_alerts.len()
    );
    for alert in pending_alerts {
        if let Err(e) = alerts::record(
            pool,
            http,
            config,
            alert.kind,
            alert.severity,
            alert.message,
            alert.details,
        )
        .await
        {
            tracing::warn!("failed to record whale alert: {e:?}");
        }
    }
    Ok(())
}

/// Bound on how many pool-ratio hops price discovery will chase outward from
/// XLM (XLM -> A -> B -> ...). Each hop only ever derives a price from real
/// reserves of an asset already priced in an earlier hop, so this is a depth
/// cap on legitimate routing, not a guess — it just stops the search once
/// it's implausible a stable, liquid route still exists.
const MAX_PRICE_HOPS: u32 = 4;

/// Fetches XLM/USD, records it, then derives and records a USD price for
/// every asset reachable from native XLM through a chain of tracked classic-
/// AMM pools (preferring, at each hop, the deepest pool that reaches a given
/// asset). An asset one hop from XLM (paired directly with it) is priced the
/// same as before; an asset only reachable via one or more intermediate
/// already-priced assets is now priced too, still purely from real reserve
/// ratios — never guessed. Assets with no path back to XLM through any
/// tracked pool stay unpriced.
async fn ingest_prices(
    tx: &mut sqlx::PgConnection,
    http: &reqwest::Client,
    config: &Config,
    now: chrono::DateTime<Utc>,
    pools: &[LiquidityPool],
) -> anyhow::Result<()> {
    let xlm_usd = pricing::fetch_xlm_usd(http, &config.coingecko_api_url).await?;
    db::insert_asset_price(&mut *tx, now, "XLM", "", xlm_usd, "coingecko").await?;

    let native: pricing::AssetId = ("XLM".to_string(), String::new());
    let mut priced: std::collections::HashMap<pricing::AssetId, Decimal> =
        std::collections::HashMap::new();
    priced.insert(native.clone(), xlm_usd);

    let reserves: Vec<(String, Decimal, String, Decimal)> = pools
        .iter()
        .filter_map(|lp| {
            let (a, b) = (lp.reserves.first()?, lp.reserves.get(1)?);
            let reserve_a = Decimal::from_str(&a.amount).ok()?;
            let reserve_b = Decimal::from_str(&b.amount).ok()?;
            Some((a.asset.clone(), reserve_a, b.asset.clone(), reserve_b))
        })
        .collect();

    for hop in 0..MAX_PRICE_HOPS {
        let mut newly: std::collections::HashMap<pricing::AssetId, (Decimal, Decimal)> =
            std::collections::HashMap::new();
        for (asset_a, reserve_a, asset_b, reserve_b) in &reserves {
            for (known_asset, known_price) in &priced {
                let Some((asset, price, weight)) = pricing::price_from_known_leg(
                    asset_a,
                    *reserve_a,
                    asset_b,
                    *reserve_b,
                    known_asset,
                    *known_price,
                ) else {
                    continue;
                };
                if priced.contains_key(&asset) {
                    continue; // already priced in an earlier (shorter) hop.
                }
                newly
                    .entry(asset)
                    .and_modify(|(best_weight, best_price)| {
                        if weight > *best_weight {
                            *best_weight = weight;
                            *best_price = price;
                        }
                    })
                    .or_insert((weight, price));
            }
        }
        if newly.is_empty() {
            break;
        }
        let source = if hop == 0 {
            "xlm_pool_ratio"
        } else {
            "multi_hop_pool_ratio"
        };
        for ((code, issuer), (_, price)) in newly {
            db::insert_asset_price(&mut *tx, now, &code, &issuer, price, source).await?;
            priced.insert((code, issuer), price);
        }
    }

    Ok(())
}

async fn ingest_pool(
    tx: &mut sqlx::PgConnection,
    now: chrono::DateTime<Utc>,
    lp: &LiquidityPool,
) -> anyhow::Result<()> {
    let (asset_a, asset_b) = match (lp.reserves.first(), lp.reserves.get(1)) {
        (Some(a), Some(b)) => (a, b),
        _ => anyhow::bail!("pool {} has fewer than 2 reserves", lp.id),
    };
    let reserve_a = Decimal::from_str(&asset_a.amount)?;
    let reserve_b = Decimal::from_str(&asset_b.amount)?;
    let total_shares = Decimal::from_str(&lp.total_shares)?;
    let trustline_count: i32 = lp.total_trustlines.parse().unwrap_or(0);

    db::upsert_pool(&mut *tx, &lp.id, &asset_a.asset, &asset_b.asset, lp.fee_bp).await?;
    db::insert_pool_snapshot(
        &mut *tx,
        now,
        &lp.id,
        reserve_a,
        reserve_b,
        total_shares,
        trustline_count,
    )
    .await?;
    Ok(())
}

/// Returns a pending alert if the payment met the whale threshold and was
/// newly recorded (not a cursor-replay duplicate of one already stored).
async fn ingest_whale_payment(
    tx: &mut sqlx::PgConnection,
    p: &PaymentRecord,
    threshold: f64,
) -> anyhow::Result<Option<PendingAlert>> {
    let Some(raw_amount) = &p.amount else {
        return Ok(None);
    };
    let amount = Decimal::from_str(raw_amount)?;
    let threshold = Decimal::try_from(threshold).unwrap_or_default();
    let Some(amount) = logic::whale_amount(&p.op_type, Some(amount), threshold) else {
        return Ok(None);
    };

    let (asset_code, asset_issuer) = logic::resolve_payment_asset(
        p.asset_type.as_deref(),
        p.asset_code.as_deref(),
        p.asset_issuer.as_deref(),
    );
    let source = p
        .from
        .clone()
        .or_else(|| p.source_account.clone())
        .unwrap_or_default();

    let inserted = db::insert_whale_transaction(
        &mut *tx,
        Utc::now(),
        &p.id,
        &p.transaction_hash,
        &source,
        p.to.as_deref(),
        &asset_code,
        asset_issuer.as_deref(),
        amount,
    )
    .await?;
    if !inserted {
        return Ok(None);
    }

    let severity = alerts::severity_for_whale_multiple(amount, threshold);
    let message = match &p.to {
        Some(dest) => format!("Whale payment: {amount} {asset_code} from {source} to {dest}"),
        None => format!("Whale payment: {amount} {asset_code} from {source}"),
    };
    let details = json!({
        "tx_hash": p.transaction_hash,
        "asset_code": asset_code,
        "asset_issuer": asset_issuer,
        "amount": amount.to_string(),
        "source_account": source,
        "dest_account": p.to,
    });
    Ok(Some(PendingAlert {
        kind: "whale_payment",
        severity,
        message,
        details,
    }))
}
