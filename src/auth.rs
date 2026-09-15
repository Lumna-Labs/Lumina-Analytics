//! Optional shared-secret gate for the API's mutating endpoints (alert
//! rules/channels, watchlist). Off by default (`ADMIN_API_KEY` unset) so
//! local/dev and single-operator deployments keep working with no extra
//! setup — this exists for operators who expose the API publicly and want to
//! stop anonymous visitors from editing shared state, not as a correctness
//! dependency, matching `ratelimit`'s fail-open posture.

use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Json, Response};

use crate::api::AppState;

const HEADER_NAME: &str = "x-admin-key";

/// Returns `Some(response)` (401) when an admin key is configured and the
/// request's `X-Admin-Key` header doesn't match it; `None` means proceed.
/// Called explicitly at the top of each mutating handler rather than as
/// router middleware, since these resources also expose unauthenticated GET
/// handlers on the very same paths.
pub fn guard(state: &AppState, headers: &HeaderMap) -> Option<Response> {
    let expected = state.admin_api_key.as_deref()?;
    let provided = headers.get(HEADER_NAME).and_then(|v| v.to_str().ok());
    if provided.is_some_and(|p| constant_time_eq(p, expected)) {
        None
    } else {
        Some(
            (
                StatusCode::UNAUTHORIZED,
                Json(serde_json::json!({ "error": "missing or invalid X-Admin-Key header" })),
            )
                .into_response(),
        )
    }
}

/// Compares two strings without short-circuiting on the first mismatched
/// byte, so response timing doesn't leak how many leading characters of a
/// guessed key were correct.
fn constant_time_eq(a: &str, b: &str) -> bool {
    let (a, b) = (a.as_bytes(), b.as_bytes());
    if a.len() != b.len() {
        return false;
    }
    a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn equal_strings_match() {
        assert!(constant_time_eq("secret123", "secret123"));
    }

    #[test]
    fn different_strings_do_not_match() {
        assert!(!constant_time_eq("secret123", "secret124"));
    }

    #[test]
    fn different_lengths_do_not_match() {
        assert!(!constant_time_eq("short", "muchlonger"));
    }

    #[test]
    fn empty_strings_match() {
        assert!(constant_time_eq("", ""));
    }
}
