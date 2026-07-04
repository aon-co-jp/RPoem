//! REST handlers for the Federation Engine.
//!
//! Endpoints:
//!   POST /api/federation/compose  – compose service schemas into one graph
//!   GET  /api/federation/status   – current composed schema summary

use crate::state::AppState;
use open_runo_federation::{compose, detect_breaking_changes, ServiceSchema};
use poem::{
    handler,
    http::StatusCode,
    web::{Data, Json},
    Result,
};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

#[derive(Debug, Deserialize)]
pub struct ComposeRequest {
    pub services: Vec<ServiceInput>,
}

#[derive(Debug, Deserialize)]
pub struct ServiceInput {
    pub service_name: String,
    /// Map of type name → list of field names.
    pub types: BTreeMap<String, Vec<String>>,
}

#[derive(Debug, Serialize)]
pub struct ComposeResponse {
    pub contributing_services: Vec<String>,
    pub types: BTreeMap<String, Vec<String>>,
    pub breaking_changes: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct FederationStatusResponse {
    pub contributing_services: Vec<String>,
    pub type_count: usize,
    pub field_count: usize,
}

/// POST /api/federation/compose — build a composed schema from service inputs.
#[handler]
pub async fn compose_schemas(
    Data(state): Data<&Arc<AppState>>,
    Json(body): Json<ComposeRequest>,
) -> Result<Json<ComposeResponse>> {
    let service_schemas: Vec<ServiceSchema> = body
        .services
        .into_iter()
        .map(|s| ServiceSchema {
            service_name: s.service_name,
            types: s
                .types
                .into_iter()
                .map(|(k, v)| (k, BTreeSet::from_iter(v)))
                .collect(),
        })
        .collect();

    let new_composed = compose(&service_schemas).map_err(|e| {
        poem::Error::from_string(e.to_string(), StatusCode::UNPROCESSABLE_ENTITY)
    })?;

    // Detect breaking changes against the previously stored composition.
    let breaking = {
        let previous = state
            .federation_schema
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone();
        if previous.contributing_services.is_empty() {
            vec![]
        } else {
            detect_breaking_changes(&previous, &new_composed)
        }
    };

    // Extract response fields before updating shared state (avoids partial move).
    let contributing_services = new_composed.contributing_services.clone();
    let types_out: BTreeMap<String, Vec<String>> = new_composed
        .types
        .iter()
        .map(|(k, v)| (k.clone(), v.iter().cloned().collect()))
        .collect();

    // Persist the new composed schema.
    *state
        .federation_schema
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner) = new_composed;

    Ok(Json(ComposeResponse {
        contributing_services,
        types: types_out,
        breaking_changes: breaking,
    }))
}

/// GET /api/federation/status — summary of the current composed schema.
#[handler]
pub async fn federation_status(
    Data(state): Data<&Arc<AppState>>,
) -> Result<Json<FederationStatusResponse>> {
    let schema = state
        .federation_schema
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .clone();

    let field_count: usize = schema.types.values().map(|f| f.len()).sum();

    Ok(Json(FederationStatusResponse {
        contributing_services: schema.contributing_services,
        type_count: schema.types.len(),
        field_count,
    }))
}
