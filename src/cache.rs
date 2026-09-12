//! Thin Redis-backed cache for hot read endpoints. Caching is purely a
//! performance optimization here: if Redis is unset, unreachable, or errors
//! out mid-request, callers transparently fall back to computing the value
//! fresh from Postgres. A cache should never be a new way for the API to go
//! down.

use redis::aio::ConnectionManager;
use serde::{de::DeserializeOwned, Serialize};
use std::future::Future;

#[derive(Clone)]
pub struct Cache {
    conn: Option<ConnectionManager>,
    ttl_secs: u64,
}

impl Cache {
    /// Attempts to connect to Redis; on any failure, logs a warning and
    /// returns a cache that's permanently a no-op rather than failing
    /// startup over an optional dependency.
    pub async fn connect(redis_url: Option<&str>, ttl_secs: u64) -> Self {
        let Some(url) = redis_url else {
            tracing::info!("REDIS_URL not set, response caching disabled");
            return Self {
                conn: None,
                ttl_secs,
            };
        };

        match redis::Client::open(url) {
            Ok(client) => match client.get_connection_manager().await {
                Ok(conn) => {
                    tracing::info!("connected to redis for response caching");
                    Self {
                        conn: Some(conn),
                        ttl_secs,
                    }
                }
                Err(e) => {
                    tracing::warn!("redis unreachable, caching disabled: {e}");
                    Self {
                        conn: None,
                        ttl_secs,
                    }
                }
            },
            Err(e) => {
                tracing::warn!("invalid REDIS_URL, caching disabled: {e}");
                Self {
                    conn: None,
                    ttl_secs,
                }
            }
        }
    }

    /// Returns the cached value for `key` if present and still fresh,
    /// otherwise calls `compute`, caches the result, and returns it. Any
    /// Redis error (down, timeout, corrupt entry) is treated as a cache miss.
    pub async fn get_or_compute<T, F, Fut>(&self, key: &str, compute: F) -> anyhow::Result<T>
    where
        T: Serialize + DeserializeOwned,
        F: FnOnce() -> Fut,
        Fut: Future<Output = anyhow::Result<T>>,
    {
        if let Some(conn) = self.conn.clone() {
            if let Some(value) = Self::try_get::<T>(conn, key).await {
                return Ok(value);
            }
        }

        let value = compute().await?;

        if let Some(conn) = self.conn.clone() {
            Self::try_set(conn, key, &value, self.ttl_secs).await;
        }

        Ok(value)
    }

    async fn try_get<T: DeserializeOwned>(mut conn: ConnectionManager, key: &str) -> Option<T> {
        use redis::AsyncCommands;
        let raw: Option<String> = conn.get(key).await.ok()?;
        let raw = raw?;
        serde_json::from_str(&raw).ok()
    }

    async fn try_set<T: Serialize>(
        mut conn: ConnectionManager,
        key: &str,
        value: &T,
        ttl_secs: u64,
    ) {
        use redis::AsyncCommands;
        let Ok(raw) = serde_json::to_string(value) else {
            return;
        };
        if let Err(e) = conn.set_ex::<_, _, ()>(key, raw, ttl_secs).await {
            tracing::debug!("failed to write cache key {key}: {e}");
        }
    }
}
