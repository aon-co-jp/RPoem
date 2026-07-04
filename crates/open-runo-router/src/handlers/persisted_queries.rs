//! `/api/persisted-queries` — Persisted Queries / Trusted Documents REST API.
//!
//! | Method | Path                           | Description                     |
//! |--------|--------------------------------|---------------------------------|
//! | POST   | `/api/persisted-queries`       | Register a document → its hash  |
//! | GET    | `/api/persisted-queries/:hash` | Fetch a registered document     |
//!
//! Enforcement inside the GraphQL execution path lands with the gateway
//! integration (Phase B); this surface manages the registry itself.

use crate::state::AppState;
use open_runo_persisted_queries::{EnforcementMode, PersistedQueryStore};
use poem::{
    handler,
    http::StatusCode,
    web::{Data, Json, Path},
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(Debug, Deserialize)]
pub struct RegisterRequest {
    pub query: String,
}

#[derive(Debug, Serialize)]
pub struct RegisterResponse {
    pub hash: String,
    pub registered_at: String,
}

#[derive(Debug, Serialize)]
pub struct QueryResponse {
    pub hash: String,
    pub query: String,
    pub registered_at: String,
}

fn store(state: &AppState) -> PersistedQueryStore {
    PersistedQueryStore::new(Arc::clone(&state.db), EnforcementMode::Allow)
}

fn bad_request(msg: impl std::fmt::Display) -> poem::Error {
    poem::Error::from_string(msg.to_string(), StatusCode::BAD_REQUEST)
}

fn internal(msg: impl std::fmt::Display) -> poem::Error {
    poem::Error::from_string(msg.to_string(), StatusCode::INTERNAL_SERVER_ERROR)
}

/// POST /api/persisted-queries — register a GraphQL document.
#[handler]
pub async fn register_persisted_query(
    req: &poem::Request,
    state: Data<&Arc<AppState>>,
    Json(body): Json<RegisterRequest>,
) -> poem::Result<Json<RegisterResponse>> {
    let record = store(&state)
        .register(&body.query)
        .await
        .map_err(bad_request)?;

    crate::audit::record(
        &state,
        &crate::audit::actor_from(req),
        "persisted_query.register",
        record.hash.clone(),
    )
    .await;

    Ok(Json(RegisterResponse {
        hash: record.hash,
        registered_at: record.registered_at.to_rfc3339(),
    }))
}

/// GET /api/persisted-queries/:hash — fetch a registered document.
#[handler]
pub async fn get_persisted_query(
    Path(hash): Path<String>,
    state: Data<&Arc<AppState>>,
) -> poem::Result<Json<QueryResponse>> {
    let record = store(&state)
        .get(&hash)
        .await
        .map_err(internal)?
        .ok_or_else(|| {
            poem::Error::from_string(
                format!("persisted query not found: {hash}"),
                StatusCode::NOT_FOUND,
            )
        })?;

    Ok(Json(QueryResponse {
        hash: record.hash,
        query: record.document,
        registered_at: record.registered_at.to_rfc3339(),
    }))
}
