//! GraphQL client for open-runo-router / open-runo-gateway, callable from WASM.
//!
//! REST全廃(CLAUDE.md「2026-11-01追記」)の第3段階: 管理画面はすべて同一オリジンの
//! `POST /graphql` を叩く。公開関数の名前・引数・戻り値は従来のREST版と同じに保ち、
//! 応答は`open-runo-api-types`の型へそのままデコードする(クエリ側で
//! `service_name: serviceName` のようにフィールドへ別名を付け、REST時代のJSON形と揃える)。
//! 生存確認の `/health` だけは管理APIではないため従来どおりGETで叩く。

use open_runo_api_types::{
    DbRecordListResponse, DbRecordResponse, FeatureFlagEvaluationResponse, FeatureFlagListResponse,
    FeatureFlagResponse, FederationStatusResponse, RateLimitedResponse, SchemaHistoryResponse,
    SchemaVersion,
};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::cell::RefCell;
use wasm_bindgen::{JsCast, JsValue};
use wasm_bindgen_futures::JsFuture;
use web_sys::{Request, RequestInit, RequestMode, Response};

/// `localStorage` key the self-issued API key is cached under, so a page
/// reload doesn't re-issue a fresh one every time.
const STORAGE_KEY: &str = "open-runo-api-key";

thread_local! {
    /// In-memory cache for this page load. WASM is single-threaded so a
    /// `RefCell` is enough — no need for a `Mutex`.
    static CACHED_KEY: RefCell<Option<String>> = const { RefCell::new(None) };
}

fn local_storage() -> Option<web_sys::Storage> {
    web_sys::window()?.local_storage().ok()?
}

/// Drop the cached key (memory + `localStorage`) so the next call
/// transparently self-issues a fresh one.
fn clear_cached_api_key() {
    CACHED_KEY.with(|c| *c.borrow_mut() = None);
    if let Some(storage) = local_storage() {
        let _ = storage.remove_item(STORAGE_KEY);
    }
}

/// HTTPレベルの応答(本文テキスト)。`ok=false`はHTTPエラー(429等)。
struct Raw {
    status: u16,
    ok: bool,
    request_id: Option<String>,
    text: String,
}

async fn fetch_raw(method: &str, path: &str, body: Option<&str>, api_key: Option<&str>) -> Result<Raw, String> {
    let opts = RequestInit::new();
    opts.set_method(method);
    opts.set_mode(RequestMode::SameOrigin);
    if let Some(body) = body {
        opts.set_body(&JsValue::from_str(body));
    }
    let request = Request::new_with_str_and_init(path, &opts).map_err(|e| format!("{e:?}"))?;
    if let Some(key) = api_key {
        request.headers().set("x-api-key", key).map_err(|e| format!("{e:?}"))?;
    }
    if body.is_some() {
        request
            .headers()
            .set("content-type", "application/json")
            .map_err(|e| format!("{e:?}"))?;
    }
    let window = web_sys::window().ok_or("no window")?;
    let resp: Response = JsFuture::from(window.fetch_with_request(&request))
        .await
        .map_err(|e| format!("fetch error: {e:?}"))?
        .dyn_into()
        .map_err(|e| format!("{e:?}"))?;
    let text = JsFuture::from(resp.text().map_err(|e| format!("{e:?}"))?)
        .await
        .map_err(|e| format!("body read error: {e:?}"))?
        .as_string()
        .unwrap_or_default();
    Ok(Raw {
        status: resp.status(),
        ok: resp.ok(),
        request_id: resp.headers().get("x-request-id").ok().flatten(),
        text,
    })
}

/// `POST /graphql` を1回叩く。HTTPエラーは文字列に、GraphQLエラーは
/// `Err(GqlFailure)` にする(401の判定に`extensions.status`が要るため)。
enum Failure {
    Text(String),
    Unauthorized,
}

async fn gql_once(query: &str, vars: &Value, api_key: Option<&str>) -> Result<Value, Failure> {
    let body = json!({ "query": query, "variables": vars }).to_string();
    let raw = fetch_raw("POST", "/graphql", Some(&body), api_key)
        .await
        .map_err(Failure::Text)?;
    let suffix = raw
        .request_id
        .as_ref()
        .map(|id| format!(" (request-id: {id})"))
        .unwrap_or_default();
    if !raw.ok {
        if raw.status == 429 {
            if let Ok(b) = serde_json::from_str::<RateLimitedResponse>(&raw.text) {
                return Err(Failure::Text(format!("rate limited, retry in {}s{suffix}", b.retry_after_secs)));
            }
            return Err(Failure::Text(format!("HTTP 429 (rate limited){suffix}")));
        }
        return Err(Failure::Text(format!("HTTP {}{suffix}", raw.status)));
    }
    let v: Value = serde_json::from_str(&raw.text)
        .map_err(|e| Failure::Text(format!("decode error: {e}{suffix}")))?;
    if let Some(err) = v["errors"].get(0) {
        if err["extensions"]["status"].as_i64() == Some(401) {
            return Err(Failure::Unauthorized);
        }
        let msg = err["message"].as_str().unwrap_or("GraphQL error");
        let status = err["extensions"]["status"]
            .as_i64()
            .map(|s| format!("HTTP {s}: "))
            .unwrap_or_default();
        return Err(Failure::Text(format!("{status}{msg}{suffix}")));
    }
    Ok(v["data"].clone())
}

/// Get a working API key, obtaining one automatically if needed — the user
/// of the app never sees, enters, or configures a key. Order: in-memory
/// cache → `localStorage` (survives reloads) → `mutation { selfIssueKey }`
/// (認証不要、`admin_sys::self_issue_key`)。
async fn get_or_issue_api_key() -> Result<String, String> {
    if let Some(key) = CACHED_KEY.with(|c| c.borrow().clone()) {
        return Ok(key);
    }
    if let Some(storage) = local_storage() {
        if let Ok(Some(key)) = storage.get_item(STORAGE_KEY) {
            if !key.is_empty() {
                CACHED_KEY.with(|c| *c.borrow_mut() = Some(key.clone()));
                return Ok(key);
            }
        }
    }
    let data = gql_once("mutation { selfIssueKey }", &json!({}), None)
        .await
        .map_err(|f| match f {
            Failure::Text(t) => format!("self-issue failed: {t}"),
            Failure::Unauthorized => "self-issue failed: unauthorized".to_string(),
        })?;
    let key = data["selfIssueKey"]["api_key"]
        .as_str()
        .ok_or("self-issue decode error: no api_key")?
        .to_string();
    CACHED_KEY.with(|c| *c.borrow_mut() = Some(key.clone()));
    if let Some(storage) = local_storage() {
        let _ = storage.set_item(STORAGE_KEY, &key);
    }
    Ok(key)
}

/// 認証付きでGraphQLを実行する。キーが失効していた(401)ら自己発行し直して1回だけ再試行。
async fn gql(query: &str, vars: Value) -> Result<Value, String> {
    let key = get_or_issue_api_key().await?;
    match gql_once(query, &vars, Some(&key)).await {
        Ok(v) => Ok(v),
        Err(Failure::Text(t)) => Err(t),
        Err(Failure::Unauthorized) => {
            clear_cached_api_key();
            let fresh = get_or_issue_api_key().await?;
            match gql_once(query, &vars, Some(&fresh)).await {
                Ok(v) => Ok(v),
                Err(Failure::Text(t)) => Err(t),
                Err(Failure::Unauthorized) => Err("HTTP 401: authentication required".to_string()),
            }
        },
    }
}

/// `{ admin { <field>... } }` / `mutation { admin { ... } }` を実行して
/// `data.admin.<field>` を型へデコードする。`selection`は`field(args){sub}`形式。
async fn admin<T: DeserializeOwned>(
    kind: &str,
    decl: &str,
    field: &str,
    selection: &str,
    vars: Value,
) -> Result<T, String> {
    let doc = format!("{kind} {decl} {{ admin {{ {selection} }} }}");
    let data = gql(&doc, vars).await?;
    serde_json::from_value(data["admin"][field].clone()).map_err(|e| format!("decode error: {e}"))
}

#[derive(Debug, Deserialize)]
pub struct Health {
    pub status: String,
    pub service: String,
    pub version: String,
}

pub async fn health_check() -> Result<Health, String> {
    let raw = fetch_raw("GET", "/health", None, None).await?;
    if !raw.ok {
        return Err(format!("HTTP {}", raw.status));
    }
    serde_json::from_str(&raw.text).map_err(|e| format!("decode error: {e}"))
}

const SCHEMA_FIELDS: &str = "id namespace service_name: serviceName sdl stage created_at: createdAt";

pub async fn register_schema(service_name: &str, sdl: &str, stage: &str) -> Result<SchemaVersion, String> {
    admin(
        "mutation",
        "($n: String!, $s: String!, $st: String)",
        "registerSchema",
        &format!("registerSchema(serviceName: $n, sdl: $s, stage: $st) {{ {SCHEMA_FIELDS} }}"),
        json!({ "n": service_name, "s": sdl, "st": stage }),
    )
    .await
}

/// SDLファイルを読み、本文を`uploadSchema`に渡す(RESTのmultipart版の置き換え)。
pub async fn register_schema_upload(
    service_name: &str,
    stage: &str,
    file: &web_sys::File,
) -> Result<SchemaVersion, String> {
    let sdl = JsFuture::from(file.text())
        .await
        .map_err(|e| format!("file read error: {e:?}"))?
        .as_string()
        .ok_or("file is not text")?;
    admin(
        "mutation",
        "($n: String!, $s: String!, $st: String)",
        "uploadSchema",
        &format!("uploadSchema(serviceName: $n, sdlFile: $s, stage: $st) {{ {SCHEMA_FIELDS} }}"),
        json!({ "n": service_name, "s": sdl, "st": stage }),
    )
    .await
}

pub async fn get_schema_history(service: &str) -> Result<SchemaHistoryResponse, String> {
    let versions: Vec<SchemaVersion> = admin(
        "query",
        "($s: String!)",
        "schemaHistory",
        &format!("schemaHistory(serviceName: $s) {{ {SCHEMA_FIELDS} }}"),
        json!({ "s": service }),
    )
    .await?;
    Ok(SchemaHistoryResponse { versions })
}

pub async fn federation_status() -> Result<FederationStatusResponse, String> {
    admin(
        "query",
        "",
        "federationStatus",
        "federationStatus { contributing_services: contributingServices type_count: typeCount field_count: fieldCount }",
        json!({}),
    )
    .await
}

#[derive(Debug, Serialize)]
pub struct AiRouteCandidate<'a> {
    pub provider: &'a str,
    pub estimated_cost_usd_per_1k_tokens: f64,
    pub estimated_latency_ms: u32,
    pub is_local: bool,
    pub context_length: u32,
}

#[derive(Debug, Deserialize)]
pub struct AiRouteResponse {
    pub selected_provider: String,
    pub is_local: bool,
    pub estimated_cost_usd_per_1k_tokens: f64,
    pub estimated_latency_ms: u32,
}

pub async fn ai_route(
    policy: &str,
    candidates: Vec<AiRouteCandidate<'_>>,
) -> Result<AiRouteResponse, String> {
    let candidates: Vec<Value> = candidates
        .iter()
        .map(|c| {
            json!({
                "provider": c.provider,
                "estimatedCostUsdPer1KTokens": c.estimated_cost_usd_per_1k_tokens,
                "estimatedLatencyMs": c.estimated_latency_ms,
                "isLocal": c.is_local,
                "contextLength": c.context_length,
            })
        })
        .collect();
    admin(
        "query",
        "($p: String!, $c: [RouteCandidateInput!]!)",
        "aiRoute",
        "aiRoute(policy: $p, candidates: $c)",
        json!({ "p": policy, "c": candidates }),
    )
    .await
}

pub async fn db_list(table: &str) -> Result<DbRecordListResponse, String> {
    admin("query", "($t: String!)", "dbList", "dbList(table: $t)", json!({ "t": table })).await
}

pub async fn db_get(table: &str, key: &str) -> Result<DbRecordResponse, String> {
    admin(
        "query",
        "($t: String!, $k: String!)",
        "dbGet",
        "dbGet(table: $t, key: $k)",
        json!({ "t": table, "k": key }),
    )
    .await
}

pub async fn db_put(table: &str, key: &str, value_json: &str) -> Result<DbRecordResponse, String> {
    let value: Value =
        serde_json::from_str(value_json).map_err(|e| format!("invalid JSON value: {e}"))?;
    admin(
        "mutation",
        "($t: String!, $k: String!, $v: JSON!)",
        "dbPut",
        "dbPut(table: $t, key: $k, value: $v)",
        json!({ "t": table, "k": key, "v": value }),
    )
    .await
}

pub async fn db_delete(table: &str, key: &str) -> Result<(), String> {
    let _: Value = admin(
        "mutation",
        "($t: String!, $k: String!)",
        "dbDelete",
        "dbDelete(table: $t, key: $k)",
        json!({ "t": table, "k": key }),
    )
    .await?;
    Ok(())
}

const FLAG_FIELDS: &str = "name enabled rollout_percent: rolloutPercent description";

pub async fn feature_flag_list() -> Result<FeatureFlagListResponse, String> {
    let flags: Vec<FeatureFlagResponse> = admin(
        "query",
        "",
        "featureFlags",
        &format!("featureFlags {{ {FLAG_FIELDS} }}"),
        json!({}),
    )
    .await?;
    Ok(FeatureFlagListResponse { flags })
}

pub async fn feature_flag_upsert(
    name: &str,
    enabled: bool,
    rollout_percent: u8,
    description: &str,
) -> Result<FeatureFlagResponse, String> {
    admin(
        "mutation",
        "($n: String!, $e: Boolean, $r: Int, $d: String)",
        "upsertFeatureFlag",
        &format!(
            "upsertFeatureFlag(name: $n, enabled: $e, rolloutPercent: $r, description: $d) {{ {FLAG_FIELDS} }}"
        ),
        json!({ "n": name, "e": enabled, "r": rollout_percent, "d": description }),
    )
    .await
}

pub async fn feature_flag_evaluate(name: &str, bucket_key: &str) -> Result<FeatureFlagEvaluationResponse, String> {
    admin(
        "query",
        "($n: String!, $b: String)",
        "evaluateFeatureFlag",
        "evaluateFeatureFlag(name: $n, bucketKey: $b) { name bucket_key: bucketKey enabled }",
        json!({ "n": name, "b": bucket_key }),
    )
    .await
}

pub async fn feature_flag_delete(name: &str) -> Result<(), String> {
    let _: Value = admin(
        "mutation",
        "($n: String!)",
        "deleteFeatureFlag",
        "deleteFeatureFlag(name: $n)",
        json!({ "n": name }),
    )
    .await?;
    Ok(())
}

#[derive(Debug, Deserialize)]
pub struct ScimUser {
    pub id: String,
    #[serde(rename = "userName")]
    pub user_name: String,
    pub active: bool,
    pub roles: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub struct ScimUserList {
    #[serde(rename = "totalResults")]
    pub total_results: usize,
    #[serde(rename = "Resources")]
    pub resources: Vec<ScimUser>,
}

pub async fn scim_list_users() -> Result<ScimUserList, String> {
    admin("query", "", "scimUsers", "scimUsers", json!({})).await
}

pub async fn scim_create_user(user_name: &str, roles: Vec<&str>) -> Result<Value, String> {
    admin(
        "mutation",
        "($u: String!, $r: [String!])",
        "scimCreateUser",
        "scimCreateUser(userName: $u, roles: $r)",
        json!({ "u": user_name, "r": roles }),
    )
    .await
}

pub async fn scim_delete_user(id: &str) -> Result<(), String> {
    let _: Value = admin(
        "mutation",
        "($i: String!)",
        "scimDeleteUser",
        "scimDeleteUser(id: $i)",
        json!({ "i": id }),
    )
    .await?;
    Ok(())
}

#[derive(Debug, Deserialize)]
pub struct PqRegisterResponse {
    pub hash: String,
    pub registered_at: String,
}

#[derive(Debug, Deserialize)]
pub struct PqQueryResponse {
    pub hash: String,
    pub query: String,
    pub registered_at: String,
}

pub async fn register_persisted_query(query: &str) -> Result<PqRegisterResponse, String> {
    admin(
        "mutation",
        "($q: String!)",
        "registerPersistedQuery",
        "registerPersistedQuery(query: $q) { hash registered_at: registeredAt }",
        json!({ "q": query }),
    )
    .await
}

pub async fn get_persisted_query(hash: &str) -> Result<PqQueryResponse, String> {
    admin(
        "query",
        "($h: String!)",
        "persistedQuery",
        "persistedQuery(hash: $h) { hash query registered_at: registeredAt }",
        json!({ "h": hash }),
    )
    .await
}

#[derive(Debug, Deserialize)]
pub struct PurgeResponse {
    pub purged: String,
}

#[derive(Debug, Deserialize)]
pub struct AiStatsResponse {
    pub ai_enabled: bool,
    pub cache_hits: u64,
    pub cache_misses: u64,
    pub hit_ratio: f64,
    pub tracked_keys: usize,
}

pub async fn cache_purge(path: &str) -> Result<PurgeResponse, String> {
    admin(
        "mutation",
        "($p: String!)",
        "purgeCache",
        "purgeCache(path: $p) { purged }",
        json!({ "p": path }),
    )
    .await
}

pub async fn cache_purge_all() -> Result<PurgeResponse, String> {
    admin("mutation", "", "purgeAllCache", "purgeAllCache { purged }", json!({})).await
}

pub async fn cache_ai_stats() -> Result<AiStatsResponse, String> {
    admin("query", "", "cacheAiStats", "cacheAiStats", json!({})).await
}

#[derive(Debug, Deserialize)]
pub struct ExportResponse {
    pub written: Vec<String>,
    pub records: usize,
}

#[derive(Debug, Deserialize)]
pub struct IntegrityResponse {
    pub backend: String,
    pub healed: usize,
}

pub async fn backup_export() -> Result<ExportResponse, String> {
    admin("mutation", "", "backupExport", "backupExport", json!({})).await
}

pub async fn integrity_check() -> Result<IntegrityResponse, String> {
    admin("mutation", "", "integrityCheck", "integrityCheck", json!({})).await
}

// ── Analytics (docs/cosmo-parity.md 4a) ─────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct MonthlyCount {
    pub month: String,
    pub count: u64,
}

#[derive(Debug, Deserialize)]
pub struct RequestsPerMonthResponse {
    pub months: Vec<MonthlyCount>,
}

#[derive(Debug, Deserialize)]
pub struct OperationSummary {
    pub method: String,
    pub path: String,
    pub count: u64,
    pub error_count: u64,
    pub avg_duration_ms: f64,
    pub error_rate: f64,
}

#[derive(Debug, Deserialize)]
pub struct OperationsResponse {
    pub operations: Vec<OperationSummary>,
}

pub async fn requests_per_month() -> Result<RequestsPerMonthResponse, String> {
    admin(
        "query",
        "",
        "analyticsRequestsPerMonth",
        "analyticsRequestsPerMonth",
        json!({}),
    )
    .await
}

pub async fn operations_summary() -> Result<OperationsResponse, String> {
    admin("query", "", "analyticsOperations", "analyticsOperations", json!({})).await
}
