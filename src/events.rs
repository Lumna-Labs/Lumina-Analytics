//! Cross-process live-event fan-out for the API's `/events` Server-Sent
//! Events stream.
//!
//! `lumina-ingest` and `lumina-api` are separate processes (see the README's
//! architecture diagram), so an in-process channel alone can't get a newly
//! recorded alert from the ingester to a browser subscribed to the API.
//! Postgres `LISTEN`/`NOTIFY` bridges that gap without adding another moving
//! part (no message broker): the ingester calls `db::notify_event` in the
//! same transaction-adjacent path that already writes the row, and the API
//! runs one long-lived `PgListener` that rebroadcasts whatever it hears to
//! every connected SSE client over an in-process `tokio::sync::broadcast`
//! channel.
//!
//! This is a real-time nicety, not a dependency: every value pushed here is
//! also sitting in Postgres, so a client that misses a broadcast (not
//! connected yet, a dropped `Lagged` message, a listener reconnect) just
//! sees it on its next regular poll instead. Nothing here can make data
//! disappear, only arrive late.

use std::time::Duration;

use sqlx::postgres::PgListener;
use tokio::sync::broadcast;

pub const CHANNEL: &str = "lumina_events";

pub type EventBus = broadcast::Sender<String>;

/// A generous capacity: slow/absent subscribers just miss old messages
/// (`Lagged`), which is fine since SSE here is a freshness nicety, not a
/// guaranteed-delivery log.
pub fn new_bus() -> EventBus {
    broadcast::channel(256).0
}

/// Runs forever: connects a `PgListener`, forwards every notification on
/// `CHANNEL` to `bus`, and reconnects with a fixed backoff if the connection
/// drops. Intended to be spawned as a background task; a failure here never
/// affects request handling, since routes read `bus` (or Postgres directly)
/// independent of whether this task is currently connected.
pub async fn listen_and_forward(database_url: String, bus: EventBus) {
    loop {
        match PgListener::connect(&database_url).await {
            Ok(mut listener) => {
                if let Err(e) = listener.listen(CHANNEL).await {
                    tracing::warn!("failed to LISTEN on {CHANNEL}: {e}");
                } else {
                    tracing::info!("listening for live events on Postgres channel {CHANNEL}");
                    loop {
                        match listener.recv().await {
                            Ok(notification) => {
                                // No receivers is a normal, expected state
                                // (no SSE clients connected) — not an error.
                                let _ = bus.send(notification.payload().to_string());
                            }
                            Err(e) => {
                                tracing::warn!("live-event listener connection lost: {e}");
                                break;
                            }
                        }
                    }
                }
            }
            Err(e) => {
                tracing::warn!("failed to connect live-event listener: {e}");
            }
        }
        tokio::time::sleep(Duration::from_secs(5)).await;
    }
}
