//! `/api/db/*` — DUAL DATABASE REST endpoints.
//!
//! Exposes the [`DbBackend`] stored in [`AppState`] as a simple key-value REST API.
//! The active backend is transparent to the caller: in development it is
//! `InMemoryBackend`; in production it is `DualBackend` (PostgreSQL + aruaru-db).
//!
//! ## Endpoints
//!
//! | Method | Path                       | Description                              |
//! |--------|----------------------------|------------------------------------------|
//! | GET    | `/api/db/status`           | Backend name + routing table             |
//! | GET    | `/api/db/routing`          | Per-table routing decisions              |
//! | GET    | `/api/db/:table`           | List all records in a logical table      |
//! | GET    | `/api/db/:table/:key`      | Retrieve one record                      |
//! | PUT    | `/api/db/:table/:key`      | Upsert a record (JSON body)              |
//! | DELETE | `/api/db/:table/:key`      | Delete a record                          |

use crate::state::AppState;
use crate::validation::{self, DB_UPSERT_REQUEST};
use poem::{
    handler,
    http::StatusCode,
    web::{Data, Json, Path},
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

// ── Response types ────────────────────────────────────────────────────────────

#[derive(Debug, Serialize)]
pub struct DbStatus {
    pub backend: &'static str,
    pub status: &'static str,
}

#[derive(Debug, Serialize)]
pub struct RoutingEntry {
    pub table: String,
    pub target: String,
}

#[derive(Debug, Serialize)]
pub struct RoutingInfo {
    pub default_target: String,
    pub entries: Vec<RoutingEntry>,
}

#[derive(Debug, Serialize)]
pub struct RecordResponse {
    pub table: String,
    pub key: String,
    pub value: serde_json::Value,
}

#[derive(Debug, Serialize)]
pub struct RecordListResponse {
    pub table: String,
    pub count: usize,
    pub records: Vec<RecordItem>,
}

#[derive(Debug, Serialize)]
pub struct RecordItem {
    pub key: String,
    pub value: serde_json::Value,
}

#[derive(Debug, Serialize)]
pub struct DeleteResponse {
    pub table: String,
    pub key: String,
    pub deleted: bool,
}

#[derive(Debug, Deserialize)]
pub struct UpsertBody {
    /// Arbitrary JSON payload to store.
    pub value: serde_json::Value,
}

// ── Helpers ────────────────────────────────────────────────────────────────────

fn internal(msg: impl std::fmt::Display) -> poem::Error {
    poem::Error::from_string(msg.to_string(), StatusCode::INTERNAL_SERVER_ERROR)
}

fn not_found(table: &str, key: &str) -> poem::Error {
    poem::Error::from_string(
        format!("record not found: {table}/{key}"),
        StatusCode::NOT_FOUND,
    )
}

fn parse_value(raw: &str) -> serde_json::Value {
    serde_json::from_str(raw).unwrap_or(serde_json::Value::String(raw.to_string()))
}

// ── Handlers ───────────────────────────────────────────────────────────────────

/// GET /api/db/status
///
/// Returns the backend name and a simple "ok" status.
/// Useful for monitoring and dashboards.
#[handler]
pub async fn db_status(state: Data<&Arc<AppState>>) -> Json<DbStatus> {
    Json(DbStatus {
        backend: state.db.backend_name(),
        status: "ok",
    })
}

/// GET /api/db/routing
///
/// Returns the default routing table for the current backend.
/// For `InMemoryBackend` and single backends this is a static description.
/// For `DualBackend` the actual routing table is reflected here.
#[handler]
pub async fn db_routing(_state: Data<&Arc<AppState>>) -> Json<RoutingInfo> {
    // Reflect the default_routing() table defined in open-runo-db so callers
    // can understand where each table is stored without reading source code.
    let entries = vec![
        RoutingEntry { table: "sessions".into(),       target: "postgresql".into() },
        RoutingEntry { table: "api_keys".into(),       target: "postgresql".into() },
        RoutingEntry { table: "rate_limits".into(),    target: "postgresql".into() },
        RoutingEntry { table: "schemas".into(),        target: "both".into() },
        RoutingEntry { table: "backup_jobs".into(),    target: "both".into() },
        RoutingEntry { table: "persisted_queries".into(), target: "both".into() },
        RoutingEntry { table: "schema_history".into(), target: "aruaru-db".into() },
        RoutingEntry { table: "change_records".into(), target: "aruaru-db".into() },
        RoutingEntry { table: "audit_log".into(),      target: "aruaru-db".into() },
    ];

    Json(RoutingInfo {
        default_target: "postgresql".into(),
        entries,
    })
}

/// GET /api/db/:table
///
/// List all records stored in the given logical table.
/// Returns an array sorted by key (order depends on backend).
#[handler]
pub async fn db_list(
    Path(table): Path<String>,
    state: Data<&Arc<AppState>>,
) -> poem::Result<Json<RecordListResponse>> {
    let records = state
        .db
        .list(&table)
        .await
        .map_err(|e| internal(e))?;

    let items: Vec<RecordItem> = records
        .into_iter()
        .map(|r| RecordItem {
            key: r.key,
            value: parse_value(&r.value),
        })
        .collect();

    Ok(Json(RecordListResponse {
        count: items.len(),
        table,
        records: items,
    }))
}

/// GET /api/db/:table/:key
///
/// Retrieve a single record by table + key.
/// Returns 404 if the record does not exist.
#[handler]
pub async fn db_get(
    Path((table, key)): Path<(String, String)>,
    state: Data<&Arc<AppState>>,
) -> poem::Result<Json<RecordResponse>> {
    let raw = state
        .db
        .get(&table, &key)
        .await
        .map_err(|e| internal(e))?
        .ok_or_else(|| not_found(&table, &key))?;

    Ok(Json(RecordResponse {
        table,
        key,
        value: parse_value(&raw),
    }))
}

/// PUT /api/db/:table/:key
///
/// Upsert a record. Body: `{ "value": <any JSON> }`.
/// Creates the record if absent, overwrites if present.
#[handler]
pub async fn db_put(
    req: &poem::Request,
    Path((table, key)): Path<(String, String)>,
    state: Data<&Arc<AppState>>,
    Json(raw): Json<serde_json::Value>,
) -> poem::Result<Json<RecordResponse>> {
    validation::validate(&DB_UPSERT_REQUEST, &raw)?;
    let body: UpsertBody = serde_json::from_value(raw)
        .map_err(|e| internal(format!("deserialize body: {e}")))?;

    let serialized = serde_json::to_string(&body.value)
        .map_err(|e| internal(format!("serialize value: {e}")))?;

    state
        .db
        .put(&table, &key, &serialized)
        .await
        .map_err(|e| internal(e))?;

    crate::audit::record(
        &state,
        &crate::audit::actor_from(req),
        "db.put",
        format!("{table}/{key}"),
    )
    .await;

    Ok(Json(RecordResponse {
        table,
        key,
        value: body.value,
    }))
}

/// DELETE /api/db/:table/:key
///
/// Delete a record. Returns `{ "deleted": true }` whether or not the record
/// existed (the operation is idempotent).
#[handler]
pub async fn db_delete(
    req: &poem::Request,
    Path((table, key)): Path<(String, String)>,
    state: Data<&Arc<AppState>>,
) -> poem::Result<Json<DeleteResponse>> {
    state
        .db
        .delete(&table, &key)
        .await
        .map_err(|e| internal(e))?;

    crate::audit::record(
        &state,
        &crate::audit::actor_from(req),
        "db.delete",
        format!("{table}/{key}"),
    )
    .await;

    Ok(Json(DeleteResponse {
        table,
        key,
        deleted: true,
    }))
}
