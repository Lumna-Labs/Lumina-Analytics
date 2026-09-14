//! Simple, dependency-free per-IP token-bucket rate limiting for the API.
//! Disabled by default (`RATE_LIMIT_RPS=0`) — this is a defensive measure
//! for a publicly exposed deployment, not a correctness dependency, so it
//! fails open rather than risk throttling legitimate local/dev traffic.

use std::collections::HashMap;
use std::net::{IpAddr, SocketAddr};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use axum::extract::{ConnectInfo, Request, State};
use axum::http::StatusCode;
use axum::middleware::Next;
use axum::response::{IntoResponse, Json, Response};

use crate::api::AppState;

struct Bucket {
    tokens: f64,
    last_refill: Instant,
}

pub struct RateLimiter {
    rps: f64,
    burst: f64,
    buckets: Mutex<HashMap<IpAddr, Bucket>>,
}

impl RateLimiter {
    pub fn new(rps: u32, burst: u32) -> Self {
        Self {
            rps: rps as f64,
            burst: burst.max(1) as f64,
            buckets: Mutex::new(HashMap::new()),
        }
    }

    pub fn enabled(&self) -> bool {
        self.rps > 0.0
    }

    /// Returns `true` if the request should be allowed. Refills the bucket
    /// based on elapsed time since it was last touched, then consumes one
    /// token if available.
    fn allow(&self, ip: IpAddr) -> bool {
        let mut buckets = self.buckets.lock().unwrap_or_else(|e| e.into_inner());
        let now = Instant::now();
        let bucket = buckets.entry(ip).or_insert_with(|| Bucket {
            tokens: self.burst,
            last_refill: now,
        });
        let elapsed = now.duration_since(bucket.last_refill).as_secs_f64();
        bucket.tokens = (bucket.tokens + elapsed * self.rps).min(self.burst);
        bucket.last_refill = now;

        if bucket.tokens >= 1.0 {
            bucket.tokens -= 1.0;
            true
        } else {
            false
        }
    }

    /// Drops buckets untouched for a while so memory doesn't grow unbounded
    /// under a churn of distinct client IPs. Called opportunistically, not
    /// on a fixed schedule.
    fn sweep_if_large(&self) {
        let mut buckets = self.buckets.lock().unwrap_or_else(|e| e.into_inner());
        if buckets.len() < 10_000 {
            return;
        }
        let now = Instant::now();
        buckets.retain(|_, b| now.duration_since(b.last_refill) < Duration::from_secs(3600));
    }
}

pub async fn enforce(
    State(state): State<AppState>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    req: Request,
    next: Next,
) -> Response {
    if !state.rate_limiter.enabled() {
        return next.run(req).await;
    }
    state.rate_limiter.sweep_if_large();
    if state.rate_limiter.allow(addr.ip()) {
        next.run(req).await
    } else {
        (
            StatusCode::TOO_MANY_REQUESTS,
            Json(serde_json::json!({ "error": "rate limit exceeded" })),
        )
            .into_response()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ip() -> IpAddr {
        "127.0.0.1".parse().unwrap()
    }

    #[test]
    fn allows_up_to_burst_then_blocks() {
        let limiter = RateLimiter::new(1, 3);
        assert!(limiter.allow(ip()));
        assert!(limiter.allow(ip()));
        assert!(limiter.allow(ip()));
        assert!(!limiter.allow(ip()));
    }

    #[test]
    fn disabled_when_rps_is_zero() {
        let limiter = RateLimiter::new(0, 10);
        assert!(!limiter.enabled());
    }

    #[test]
    fn distinct_ips_have_independent_buckets() {
        let limiter = RateLimiter::new(1, 1);
        assert!(limiter.allow("10.0.0.1".parse().unwrap()));
        assert!(!limiter.allow("10.0.0.1".parse().unwrap()));
        assert!(limiter.allow("10.0.0.2".parse().unwrap()));
    }
}
