//! `/api/events` — Server-Sent Events (SSE) stream.
//!
//! Pushes real-time updates to connected clients without requiring polling.
//! Phase 1 emits a periodic heartbeat plus the current tail of the change
//! history (see [`open_runo_history::History`]) so dashboards (e.g. the Tauri
//! desktop app) can react to schema/federation/db changes as they happen.

use crate::state::AppState;
use futures::stream::{self, Stream};
use poem::{
    handler,
    web::{
        sse::{Event, SSE},
        Data,
    },
};
use std::{sync::Arc, time::Duration};

const HEARTBEAT_INTERVAL: Duration = Duration::from_secs(15);

/// GET /api/events
///
/// Opens a long-lived `text/event-stream` connection. Emits:
/// - `event: heartbeat` every [`HEARTBEAT_INTERVAL`] to keep the connection
///   alive through proxies/load balancers.
/// - `event: history` whenever polled and the change log has grown since
///   the last tick (best-effort; Phase 1 has no pub/sub bus yet).
#[handler]
pub fn stream_events(state: Data<&Arc<AppState>>) -> SSE {
    let state = Arc::clone(&state);
    let initial_len = state.history.lock().map(|h| h.log().len()).unwrap_or(0);

    let stream = stream::unfold(
        (state, initial_len),
        |(state, mut last_len)| async move {
            tokio::time::sleep(HEARTBEAT_INTERVAL).await;

            let current_len = state.history.lock().map(|h| h.log().len()).unwrap_or(last_len);

            let event = if current_len > last_len {
                last_len = current_len;
                Event::message(format!("{{\"history_len\":{current_len}}}"))
                    .event_type("history")
            } else {
                Event::message("ping").event_type("heartbeat")
            };

            Some((event, (state, last_len)))
        },
    );

    SSE::new(stream)
}

pub type EventStream = SSE;

#[allow(dead_code)]
fn _assert_stream_bound<S: Stream<Item = Event> + Send + 'static>(_s: S) {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::AppState;
    use poem::{get, test::TestClient, EndpointExt, Route};
    use std::sync::Arc;

    #[tokio::test]
    async fn events_endpoint_returns_event_stream_content_type() {
        let state = Arc::new(AppState::new());
        let app = Route::new().at("/api/events", get(stream_events)).data(state);
        let client = TestClient::new(app);

        let resp = client.get("/api/events").send().await;
        resp.assert_status_is_ok();
        resp.assert_header("content-type", "text/event-stream");
    }
}
