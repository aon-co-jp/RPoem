//! REST handlers for the Schema Registry API.
//!
//! Endpoints:
//!   POST /api/schemas                    – register a new schema version
//!   GET  /api/schemas/:service           – latest schema for a service
//!   GET  /api/schemas/:service/history   – full version history

use crate::state::AppState;
use crate::validation::{self, REGISTER_SCHEMA_REQUEST};
use open_runo_schema_registry::{Stage, DEFAULT_NAMESPACE};
use poem::{
    handler,
    http::StatusCode,
    web::{Data, Json, Path, Query},
    Result,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(Debug, Deserialize)]
pub struct RegisterRequest {
    pub service_name: String,
    pub sdl: String,
    #[serde(default = "default_stage")]
    pub stage: String,
    /// Multi-graph namespace (optional; defaults to `default`).
    #[serde(default)]
    pub namespace: Option<String>,
}

fn default_stage() -> String {
    "local".to_string()
}

fn parse_stage(s: &str) -> Stage {
    match s.to_lowercase().as_str() {
        "development" | "dev" => Stage::Development,
        "staging" | "stg" => Stage::Staging,
        "production" | "prod" => Stage::Production,
        _ => Stage::Local,
    }
}

fn stage_name(stage: Stage) -> &'static str {
    match stage {
        Stage::Local => "local",
        Stage::Development => "development",
        Stage::Staging => "staging",
        Stage::Production => "production",
    }
}

#[derive(Debug, Serialize)]
struct RegisterResponse {
    id: String,
    namespace: String,
    service_name: String,
    stage: String,
    created_at: String,
}

#[derive(Debug, Serialize)]
struct SchemaResponse {
    id: String,
    namespace: String,
    service_name: String,
    sdl: String,
    stage: String,
    created_at: String,
}

#[derive(Debug, Serialize)]
struct HistoryResponse {
    versions: Vec<SchemaResponse>,
}

/// POST /api/schemas — register a schema version.
#[handler]
pub async fn register_schema(
    req: &poem::Request,
    Data(state): Data<&Arc<AppState>>,
    Json(raw): Json<serde_json::Value>,
) -> Result<Json<RegisterResponse>> {
    validation::validate(&REGISTER_SCHEMA_REQUEST, &raw)?;
    let body: RegisterRequest = serde_json::from_value(raw)
        .map_err(|e| poem::Error::from_string(e.to_string(), StatusCode::UNPROCESSABLE_ENTITY))?;

    let stage = parse_stage(&body.stage);
    let namespace = body.namespace.clone().unwrap_or_else(|| DEFAULT_NAMESPACE.to_string());
    let version = state
        .schema_registry
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .register_in(&namespace, &body.service_name, &body.sdl, stage);

    crate::audit::record(
        state,
        &crate::audit::actor_from(req),
        "schema.register",
        format!("{}@{}", body.service_name, body.stage),
    )
    .await;

    // Publish to the realtime broker (GraphQL Subscriptions). A send error
    // just means no subscribers are connected right now.
    let _ = state.events.send(crate::state::SchemaEvent {
        service_name: body.service_name.clone(),
        stage: body.stage.clone(),
        at: chrono::Utc::now().to_rfc3339(),
    });

    Ok(Json(RegisterResponse {
        id: version.id.to_string(),
        namespace: version.namespace.clone(),
        service_name: version.service_name.clone(),
        stage: stage_name(version.stage).to_string(),
        created_at: version.created_at.to_rfc3339(),
    }))
}

/// GET /api/schemas/:service — latest schema (defaults to local stage).
#[derive(Debug, Deserialize)]
pub struct StageQuery {
    pub stage: Option<String>,
    /// Multi-graph namespace (optional; defaults to `default`).
    pub namespace: Option<String>,
}

#[handler]
pub async fn get_schema(
    Data(state): Data<&Arc<AppState>>,
    Path(service): Path<String>,
    Query(q): Query<StageQuery>,
) -> Result<Json<SchemaResponse>> {
    let stage_str = q.stage.as_deref().unwrap_or("local");
    let stage = parse_stage(stage_str);
    let namespace = q.namespace.as_deref().unwrap_or(DEFAULT_NAMESPACE);
    let registry = state
        .schema_registry
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);

    match registry.latest_in(namespace, &service, stage) {
        Some(v) => Ok(Json(SchemaResponse {
            id: v.id.to_string(),
            namespace: v.namespace.clone(),
            service_name: v.service_name.clone(),
            sdl: v.sdl.clone(),
            stage: stage_name(v.stage).to_string(),
            created_at: v.created_at.to_rfc3339(),
        })),
        None => Err(poem::Error::from_string(
            format!("no schema found for '{service}' at stage '{stage_str}'"),
            StatusCode::NOT_FOUND,
        )),
    }
}

/// GET /api/schemas/:service/history — full version history.
#[handler]
pub async fn get_schema_history(
    Data(state): Data<&Arc<AppState>>,
    Path(service): Path<String>,
    Query(q): Query<StageQuery>,
) -> Result<Json<HistoryResponse>> {
    let namespace = q.namespace.as_deref().unwrap_or(DEFAULT_NAMESPACE);
    let registry = state
        .schema_registry
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);

    let versions = registry
        .history_in(namespace, &service)
        .iter()
        .map(|v| SchemaResponse {
            id: v.id.to_string(),
            namespace: v.namespace.clone(),
            service_name: v.service_name.clone(),
            sdl: v.sdl.clone(),
            stage: stage_name(v.stage).to_string(),
            created_at: v.created_at.to_rfc3339(),
        })
        .collect();

    Ok(Json(HistoryResponse { versions }))
}
