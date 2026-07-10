//! Poem-free handler implementations, migrated one at a time from
//! `handlers/*.rs` (which stay on `poem` until every handler here has an
//! equivalent and `lib.rs::build_app` switches over). Each function here
//! returns a `hyper_compat::Handler` closing over whatever state it needs,
//! matching the JSON shape/status codes of its poem counterpart exactly.

use crate::hyper_compat::{json_response, Handler};
use crate::state::AppState;
use hyper::StatusCode;
use serde::Serialize;
use std::sync::Arc;

#[derive(Serialize)]
struct FederationStatusResponse {
    contributing_services: Vec<String>,
    type_count: usize,
    field_count: usize,
}

/// GET /api/federation/status — poem-free port of
/// `handlers::federation::federation_status`. Auth is not yet wired at this
/// layer (see CLAUDE.md HANDOFF); it currently mirrors the *unauthenticated*
/// body of the handler only.
pub fn federation_status_handler(state: Arc<AppState>) -> Handler {
    Arc::new(move |_req, _params| {
        let state = Arc::clone(&state);
        Box::pin(async move {
            let schema = state
                .federation_schema
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .clone();
            let field_count: usize = schema.types.values().map(|f| f.len()).sum();
            json_response(
                StatusCode::OK,
                &FederationStatusResponse {
                    contributing_services: schema.contributing_services,
                    type_count: schema.types.len(),
                    field_count,
                },
            )
        })
    })
}

#[derive(Serialize)]
struct DbStatus {
    backend: &'static str,
    status: &'static str,
}

/// GET /api/db/status — poem-free port of `handlers::db::db_status`.
/// Test-mode (`AppState::new()`) always runs the in-memory backend, so the
/// response is a fixed shape identical to the poem handler's test-mode path.
pub fn db_status_handler(_state: Arc<AppState>) -> Handler {
    Arc::new(move |_req, _params| {
        Box::pin(async move {
            json_response(
                StatusCode::OK,
                &DbStatus {
                    backend: "in-memory",
                    status: "ok",
                },
            )
        })
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hyper_compat::{serve, Router};
    use hyper::Method;

    #[tokio::test]
    async fn federation_status_reflects_composed_schema() {
        let state = Arc::new(AppState::new());
        let router = Router::new().route(
            Method::GET,
            "/api/federation/status",
            federation_status_handler(Arc::clone(&state)),
        );
        let (addr, _handle) = serve(router, "127.0.0.1:0".parse().unwrap())
            .await
            .expect("bind ephemeral port");

        let resp = reqwest::Client::new()
            .get(format!("http://{addr}/api/federation/status"))
            .send()
            .await
            .expect("request should succeed");
        assert_eq!(resp.status(), reqwest::StatusCode::OK);
        let body: serde_json::Value = resp.json().await.expect("valid json body");
        assert_eq!(body["contributing_services"], serde_json::json!([]));
        assert_eq!(body["type_count"], 0);
        assert_eq!(body["field_count"], 0);
    }

    #[tokio::test]
    async fn db_status_reports_in_memory_backend() {
        let state = Arc::new(AppState::new());
        let router = Router::new().route(
            Method::GET,
            "/api/db/status",
            db_status_handler(Arc::clone(&state)),
        );
        let (addr, _handle) = serve(router, "127.0.0.1:0".parse().unwrap())
            .await
            .expect("bind ephemeral port");

        let resp = reqwest::Client::new()
            .get(format!("http://{addr}/api/db/status"))
            .send()
            .await
            .expect("request should succeed");
        assert_eq!(resp.status(), reqwest::StatusCode::OK);
        let body: serde_json::Value = resp.json().await.expect("valid json body");
        assert_eq!(body["backend"], "in-memory");
        assert_eq!(body["status"], "ok");
    }
}
