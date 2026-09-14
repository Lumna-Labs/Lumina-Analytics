//! Minimal, dependency-free Prometheus-format metrics for the API: per-route
//! request counters by status class, plus total request latency for a crude
//! average. This intentionally doesn't pull in a metrics crate — the surface
//! area here (a handful of counters) doesn't need one, and it keeps the
//! `/metrics` endpoint's behavior fully readable in one file.

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::Instant;

use axum::extract::{MatchedPath, Request, State};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};

use crate::api::AppState;

#[derive(Default)]
struct RouteStats {
    count_2xx: u64,
    count_4xx: u64,
    count_5xx: u64,
    count_other: u64,
    total_latency_ms: u64,
}

#[derive(Default)]
pub struct Metrics {
    routes: Mutex<HashMap<String, RouteStats>>,
}

impl Metrics {
    pub fn new() -> Self {
        Self::default()
    }

    fn record(&self, route: &str, status: u16, latency_ms: u64) {
        let mut routes = self.routes.lock().unwrap_or_else(|e| e.into_inner());
        let stats = routes.entry(route.to_string()).or_default();
        match status {
            200..=299 => stats.count_2xx += 1,
            400..=499 => stats.count_4xx += 1,
            500..=599 => stats.count_5xx += 1,
            _ => stats.count_other += 1,
        }
        stats.total_latency_ms += latency_ms;
    }

    /// Renders all counters in Prometheus text exposition format.
    pub fn render(&self) -> String {
        let routes = self.routes.lock().unwrap_or_else(|e| e.into_inner());
        let mut out = String::new();
        out.push_str(
            "# HELP lumina_http_requests_total Total HTTP requests by route and status class.\n",
        );
        out.push_str("# TYPE lumina_http_requests_total counter\n");
        for (route, stats) in routes.iter() {
            for (class, count) in [
                ("2xx", stats.count_2xx),
                ("4xx", stats.count_4xx),
                ("5xx", stats.count_5xx),
                ("other", stats.count_other),
            ] {
                if count > 0 {
                    out.push_str(&format!(
                        "lumina_http_requests_total{{route=\"{route}\",status=\"{class}\"}} {count}\n"
                    ));
                }
            }
        }
        out.push_str("# HELP lumina_http_request_duration_ms_total Cumulative request handling time in milliseconds, by route.\n");
        out.push_str("# TYPE lumina_http_request_duration_ms_total counter\n");
        for (route, stats) in routes.iter() {
            out.push_str(&format!(
                "lumina_http_request_duration_ms_total{{route=\"{route}\"}} {}\n",
                stats.total_latency_ms
            ));
        }
        out
    }
}

pub async fn track(State(state): State<AppState>, req: Request, next: Next) -> Response {
    let route = req
        .extensions()
        .get::<MatchedPath>()
        .map(|p| p.as_str().to_string())
        .unwrap_or_else(|| req.uri().path().to_string());
    let start = Instant::now();
    let response = next.run(req).await;
    let latency_ms = start.elapsed().as_millis() as u64;
    state
        .metrics
        .record(&route, response.status().as_u16(), latency_ms);
    response
}

pub async fn metrics_handler(State(state): State<AppState>) -> impl IntoResponse {
    (
        [("content-type", "text/plain; version=0.0.4")],
        state.metrics.render(),
    )
}
