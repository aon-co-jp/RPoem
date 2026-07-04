//! open-runo Desktop — Tauri backend library.
//!
//! Tauri commands exposed to the TypeScript frontend.
//! Each command maps directly to a call against the open-runo-router REST API
//! running locally (default: http://localhost:8080).

use serde::{Deserialize, Serialize};
use tauri::State;

// ── Shared HTTP client state ──────────────────────────────────────────────

pub struct RouterClient {
    pub base_url: String,
    pub api_key: String,
    pub http: reqwest::Client,
}

impl RouterClient {
    pub fn new(base_url: impl Into<String>, api_key: impl Into<String>) -> Self {
        Self {
            base_url: base_url.into(),
            api_key: api_key.into(),
            http: reqwest::Client::new(),
        }
    }

    pub fn url(&self, path: &str) -> String {
        format!("{}{}", self.base_url, path)
    }
}

// ── Response types (mirror open-runo-router JSON) ──────────────────────────

#[derive(Debug, Serialize, Deserialize)]
pub struct HealthResponse {
    pub status: String,
    pub service: String,
    pub version: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SchemaResponse {
    pub id: String,
    pub service_name: String,
    pub sdl: String,
    pub stage: String,
    pub created_at: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RegisterSchemaRequest {
    pub service_name: String,
    pub sdl: String,
    pub stage: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RegisterSchemaResponse {
    pub id: String,
    pub service_name: String,
    pub stage: String,
    pub created_at: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct HistoryResponse {
    pub versions: Vec<SchemaResponse>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ServiceInput {
    pub service_name: String,
    /// type name → field names
    pub types: std::collections::HashMap<String, Vec<String>>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ComposeRequest {
    pub services: Vec<ServiceInput>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ComposeResponse {
    pub contributing_services: Vec<String>,
    pub types: std::collections::HashMap<String, Vec<String>>,
    pub breaking_changes: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct FederationStatusResponse {
    pub contributing_services: Vec<String>,
    pub type_count: usize,
    pub field_count: usize,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AiRouteRequest {
    pub policy: String,
    pub min_context_length: Option<u32>,
    pub candidates: Vec<AiCandidate>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AiCandidate {
    pub provider: String,
    pub estimated_cost_usd_per_1k_tokens: f64,
    pub estimated_latency_ms: u32,
    pub is_local: bool,
    pub context_length: u32,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DbRecordResponse {
    pub table: String,
    pub key: String,
    pub value: serde_json::Value,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DbRecordItem {
    pub key: String,
    pub value: serde_json::Value,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DbRecordListResponse {
    pub table: String,
    pub count: usize,
    pub records: Vec<DbRecordItem>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AiRouteResponse {
    pub selected_provider: String,
    pub is_local: bool,
    pub estimated_cost_usd_per_1k_tokens: f64,
    pub estimated_latency_ms: u32,
}

// ── Tauri commands ─────────────────────────────────────────────────────────

/// GET /health — ping the router.
#[tauri::command]
pub async fn health_check(client: State<'_, RouterClient>) -> Result<HealthResponse, String> {
    client
        .http
        .get(client.url("/health"))
        .send()
        .await
        .map_err(|e| e.to_string())?
        .json::<HealthResponse>()
        .await
        .map_err(|e| e.to_string())
}

/// POST /api/schemas — register a schema version.
#[tauri::command]
pub async fn register_schema(
    client: State<'_, RouterClient>,
    service_name: String,
    sdl: String,
    stage: String,
) -> Result<RegisterSchemaResponse, String> {
    client
        .http
        .post(client.url("/api/schemas"))
        .header("x-api-key", &client.api_key)
        .json(&RegisterSchemaRequest { service_name, sdl, stage })
        .send()
        .await
        .map_err(|e| e.to_string())?
        .json::<RegisterSchemaResponse>()
        .await
        .map_err(|e| e.to_string())
}

/// GET /api/schemas/:service — latest schema for a service.
#[tauri::command]
pub async fn get_schema(
    client: State<'_, RouterClient>,
    service: String,
    stage: Option<String>,
) -> Result<SchemaResponse, String> {
    let mut url = client.url(&format!("/api/schemas/{service}"));
    if let Some(s) = stage {
        url.push_str(&format!("?stage={s}"));
    }
    client
        .http
        .get(&url)
        .header("x-api-key", &client.api_key)
        .send()
        .await
        .map_err(|e| e.to_string())?
        .json::<SchemaResponse>()
        .await
        .map_err(|e| e.to_string())
}

/// GET /api/schemas/:service/history — full version history.
#[tauri::command]
pub async fn get_schema_history(
    client: State<'_, RouterClient>,
    service: String,
) -> Result<HistoryResponse, String> {
    client
        .http
        .get(client.url(&format!("/api/schemas/{service}/history")))
        .header("x-api-key", &client.api_key)
        .send()
        .await
        .map_err(|e| e.to_string())?
        .json::<HistoryResponse>()
        .await
        .map_err(|e| e.to_string())
}

/// POST /api/federation/compose — compose subgraph schemas.
#[tauri::command]
pub async fn compose_schemas(
    client: State<'_, RouterClient>,
    request: ComposeRequest,
) -> Result<ComposeResponse, String> {
    client
        .http
        .post(client.url("/api/federation/compose"))
        .header("x-api-key", &client.api_key)
        .json(&request)
        .send()
        .await
        .map_err(|e| e.to_string())?
        .json::<ComposeResponse>()
        .await
        .map_err(|e| e.to_string())
}

/// GET /api/federation/status — current federation summary.
#[tauri::command]
pub async fn federation_status(
    client: State<'_, RouterClient>,
) -> Result<FederationStatusResponse, String> {
    client
        .http
        .get(client.url("/api/federation/status"))
        .header("x-api-key", &client.api_key)
        .send()
        .await
        .map_err(|e| e.to_string())?
        .json::<FederationStatusResponse>()
        .await
        .map_err(|e| e.to_string())
}

/// POST /api/ai/route — select best AI provider.
#[tauri::command]
pub async fn ai_route(
    client: State<'_, RouterClient>,
    request: AiRouteRequest,
) -> Result<AiRouteResponse, String> {
    client
        .http
        .post(client.url("/api/ai/route"))
        .header("x-api-key", &client.api_key)
        .json(&request)
        .send()
        .await
        .map_err(|e| e.to_string())?
        .json::<AiRouteResponse>()
        .await
        .map_err(|e| e.to_string())
}

/// GET /api/db/:table — list all records in a logical table.
#[tauri::command]
pub async fn db_list(
    client: State<'_, RouterClient>,
    table: String,
) -> Result<DbRecordListResponse, String> {
    client
        .http
        .get(client.url(&format!("/api/db/{table}")))
        .header("x-api-key", &client.api_key)
        .send()
        .await
        .map_err(|e| e.to_string())?
        .json::<DbRecordListResponse>()
        .await
        .map_err(|e| e.to_string())
}

/// GET /api/db/:table/:key — retrieve one record.
#[tauri::command]
pub async fn db_get(
    client: State<'_, RouterClient>,
    table: String,
    key: String,
) -> Result<DbRecordResponse, String> {
    client
        .http
        .get(client.url(&format!("/api/db/{table}/{key}")))
        .header("x-api-key", &client.api_key)
        .send()
        .await
        .map_err(|e| e.to_string())?
        .json::<DbRecordResponse>()
        .await
        .map_err(|e| e.to_string())
}

/// PUT /api/db/:table/:key — upsert a record. Body: `{ "value": <any JSON> }`.
#[tauri::command]
pub async fn db_put(
    client: State<'_, RouterClient>,
    table: String,
    key: String,
    value: serde_json::Value,
) -> Result<DbRecordResponse, String> {
    client
        .http
        .put(client.url(&format!("/api/db/{table}/{key}")))
        .header("x-api-key", &client.api_key)
        .json(&serde_json::json!({ "value": value }))
        .send()
        .await
        .map_err(|e| e.to_string())?
        .json::<DbRecordResponse>()
        .await
        .map_err(|e| e.to_string())
}

// ── App entry point ────────────────────────────────────────────────────────

pub fn run() {
    let client = RouterClient::new(
        std::env::var("OPEN_RUNO_BASE_URL")
            .unwrap_or_else(|_| "http://localhost:8080".to_string()),
        std::env::var("OPEN_RUNO_API_KEY")
            .unwrap_or_else(|_| "dev-key".to_string()),
    );

    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_opener::init())
        .manage(client)
        .invoke_handler(tauri::generate_handler![
            health_check,
            register_schema,
            get_schema,
            get_schema_history,
            compose_schemas,
            federation_status,
            ai_route,
            db_list,
            db_get,
            db_put,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
