use std::convert::Infallible;
use std::time::Duration as StdDuration;

use axum::extract::State;
use axum::response::sse::{Event, KeepAlive, Sse};
use futures_util::stream::Stream;
use futures_util::StreamExt;
use tokio_stream::wrappers::BroadcastStream;

use crate::api::AppState;

/// Streams live-event notifications (new alerts, which cover whale payments
/// and lending-risk escalations — see `alerts::record`) as they're published
/// via Postgres NOTIFY (see `events`). A best-effort nicety: every event
/// pushed here also landed in a table a client can poll, so a client that
/// connects late, misses a broadcast (`Lagged`), or never opens this stream
/// at all still sees everything on its next regular poll — nothing is only
/// available here.
pub async fn events_stream(
    State(state): State<AppState>,
) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    let receiver = state.events.subscribe();
    let stream = BroadcastStream::new(receiver).filter_map(|msg| async move {
        match msg {
            Ok(payload) => Some(Ok(Event::default().event("alert").data(payload))),
            // A slow/absent-for-a-while subscriber dropped some messages;
            // skip them rather than ending the stream, since a client that
            // just missed some history still catches up via polling.
            Err(_lagged) => None,
        }
    });
    Sse::new(stream).keep_alive(KeepAlive::new().interval(StdDuration::from_secs(15)))
}
