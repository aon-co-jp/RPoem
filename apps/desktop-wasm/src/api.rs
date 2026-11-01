//! gRPC-web client for open-runo (`open_runo.admin.v1.Admin`), callable from WASM.
//!
//! REST全廃・gRPC移行(CLAUDE.md「2026-11-01追記」): 管理画面はすべて同一オリジンの
//! `POST /open_runo.admin.v1.Admin/<method>`(`application/grpc-web+proto`)を叩く。ブラウザは
//! 素のgRPC(HTTP/2トレーラー)を話せないため、サーバーがgRPC-webを同じポートで受ける
//! (Connect / gRPC-web-text も受けられる)。公開関数の名前・引数・戻り値は従来と同じに保ち、
//! 応答は`open-runo-api-types`の型へデコードする(ページ側は無変更)。型は`open-runo-proto`
//! (`admin.proto`から生成)で、サーバー・CLIと共有する。生存確認の `/health` だけは管理APIではない
//! ため従来どおりGETで叩く。

use open_runo_api_types::{
    DbRecordListResponse, DbRecordResponse, FeatureFlagEvaluationResponse, FeatureFlagListResponse,
    FeatureFlagResponse, FederationStatusResponse, SchemaHistoryResponse, SchemaVersion,
};
use open_runo_proto::prost::Message;
use open_runo_proto::{code, encode_frame, parse_web_trailers, split_frame, struct_to_json, v1};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::cell::RefCell;
use wasm_bindgen::JsCast;
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

/// gRPCの失敗(コード+メッセージ)。HTTPレベルの失敗はコード`UNKNOWN`で文字列に入れる。
struct RpcError {
    code: i32,
    message: String,
}

impl RpcError {
    fn text(message: impl Into<String>) -> Self {
        Self {
            code: code::UNKNOWN,
            message: message.into(),
        }
    }
    fn into_string(self) -> String {
        if self.code == code::UNKNOWN || self.code == code::OK {
            self.message
        } else {
            format!("{}: {}", code::name(self.code), self.message)
        }
    }
}

/// `fetch`を1回行い、`(HTTP状態, ok, リクエストID, 本文のバイト列)`を返す。
async fn fetch_bytes(
    method: &str,
    path: &str,
    body: Option<&[u8]>,
    api_key: Option<&str>,
    grpc_web: bool,
) -> Result<(u16, bool, Option<String>, Vec<u8>), RpcError> {
    let js = |e: wasm_bindgen::JsValue| RpcError::text(format!("{e:?}"));
    let opts = RequestInit::new();
    opts.set_method(method);
    opts.set_mode(RequestMode::SameOrigin);
    if let Some(body) = body {
        opts.set_body(&js_sys::Uint8Array::from(body));
    }
    let request = Request::new_with_str_and_init(path, &opts).map_err(js)?;
    let headers = request.headers();
    if let Some(key) = api_key {
        headers.set("x-api-key", key).map_err(js)?;
    }
    if grpc_web {
        headers.set("content-type", "application/grpc-web+proto").map_err(js)?;
        headers.set("x-grpc-web", "1").map_err(js)?;
        headers.set("accept", "application/grpc-web+proto").map_err(js)?;
    }
    let window = web_sys::window().ok_or_else(|| RpcError::text("no window"))?;
    let resp: Response = JsFuture::from(window.fetch_with_request(&request))
        .await
        .map_err(|e| RpcError::text(format!("fetch error: {e:?}")))?
        .dyn_into()
        .map_err(js)?;
    let buf = JsFuture::from(resp.array_buffer().map_err(js)?)
        .await
        .map_err(|e| RpcError::text(format!("body read error: {e:?}")))?;
    let bytes = js_sys::Uint8Array::new(&buf).to_vec();
    let request_id = resp.headers().get("x-request-id").ok().flatten();
    Ok((resp.status(), resp.ok(), request_id, bytes))
}

/// gRPC-webの単項呼び出し1回。成功なら応答メッセージのバイト列を返す。
async fn call_once<Req: Message>(method: &str, req: &Req, api_key: Option<&str>) -> Result<Vec<u8>, RpcError> {
    let body = encode_frame(&req.encode_to_vec());
    let (status, ok, request_id, bytes) =
        fetch_bytes("POST", &open_runo_proto::method_path(method), Some(&body), api_key, true).await?;
    let suffix = request_id.map(|id| format!(" (request-id: {id})")).unwrap_or_default();
    if !ok {
        // 流量制限(429)などHTTPレベルの失敗。
        return Err(RpcError::text(if status == 429 {
            format!("HTTP 429 (rate limited){suffix}")
        } else {
            format!("HTTP {status}{suffix}")
        }));
    }
    // データフレーム(0個以上)の後ろに、フラグ0x80のトレーラーフレームが来る。
    let mut message = Vec::new();
    let mut status_pair = (code::UNKNOWN, "missing grpc-web trailers".to_string());
    let mut rest: &[u8] = &bytes;
    while let Some((flag, body, tail)) = split_frame(rest) {
        if flag & 0x80 != 0 {
            status_pair = parse_web_trailers(body);
        } else if message.is_empty() {
            message = body.to_vec();
        }
        rest = tail;
    }
    if status_pair.0 != code::OK {
        return Err(RpcError {
            code: status_pair.0,
            message: format!("{}{suffix}", status_pair.1),
        });
    }
    Ok(message)
}

/// Get a working API key, obtaining one automatically if needed — the user
/// of the app never sees, enters, or configures a key. Order: in-memory
/// cache → `localStorage` (survives reloads) → `SelfIssueKey` RPC (認証不要)。
async fn get_or_issue_api_key() -> Result<String, RpcError> {
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
    let bytes = call_once("SelfIssueKey", &v1::Empty {}, None).await.map_err(|e| RpcError {
        code: e.code,
        message: format!("self-issue failed: {}", e.message),
    })?;
    let issued = v1::SelfIssued::decode(&bytes[..])
        .map_err(|e| RpcError::text(format!("self-issue decode error: {e}")))?;
    CACHED_KEY.with(|c| *c.borrow_mut() = Some(issued.api_key.clone()));
    if let Some(storage) = local_storage() {
        let _ = storage.set_item(STORAGE_KEY, &issued.api_key);
    }
    Ok(issued.api_key)
}

/// 認証付き単項呼び出し。キーが失効していた(UNAUTHENTICATED)ら自己発行し直して1回だけ再試行する。
async fn rpc<Req: Message, Resp: Message + Default>(method: &str, req: &Req) -> Result<Resp, String> {
    let key = get_or_issue_api_key().await.map_err(RpcError::into_string)?;
    let bytes = match call_once(method, req, Some(&key)).await {
        Ok(b) => b,
        Err(e) if e.code == code::UNAUTHENTICATED => {
            clear_cached_api_key();
            let fresh = get_or_issue_api_key().await.map_err(RpcError::into_string)?;
            call_once(method, req, Some(&fresh)).await.map_err(RpcError::into_string)?
        },
        Err(e) => return Err(e.into_string()),
    };
    Resp::decode(&bytes[..]).map_err(|e| format!("decode error: {e}"))
}

/// `google.protobuf.Struct`で返るRPCを、REST時代と同じJSON形の型へデコードする。
async fn rpc_json<Req: Message, T: DeserializeOwned>(method: &str, req: &Req) -> Result<T, String> {
    let s: open_runo_proto::prost_types::Struct = rpc(method, req).await?;
    serde_json::from_value(struct_to_json(&s)).map_err(|e| format!("decode error: {e}"))
}

#[derive(Debug, Deserialize)]
pub struct Health {
    pub status: String,
    pub service: String,
    pub version: String,
}

pub async fn health_check() -> Result<Health, String> {
    let (status, ok, _, bytes) = fetch_bytes("GET", "/health", None, None, false)
        .await
        .map_err(RpcError::into_string)?;
    if !ok {
        return Err(format!("HTTP {status}"));
    }
    serde_json::from_slice(&bytes).map_err(|e| format!("decode error: {e}"))
}

fn schema_version(v: v1::SchemaVersion) -> SchemaVersion {
    SchemaVersion {
        id: v.id,
        namespace: v.namespace,
        service_name: v.service_name,
        sdl: v.sdl,
        stage: v.stage,
        created_at: v.created_at,
    }
}

pub async fn register_schema(service_name: &str, sdl: &str, stage: &str) -> Result<SchemaVersion, String> {
    let v: v1::SchemaVersion = rpc(
        "RegisterSchema",
        &v1::RegisterSchemaRequest {
            service_name: service_name.to_string(),
            sdl: sdl.to_string(),
            stage: Some(stage.to_string()),
            namespace: None,
        },
    )
    .await?;
    Ok(schema_version(v))
}

/// SDLファイルを読み、本文を`RegisterSchema`に渡す(旧multipartアップロードの置き換え)。
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
    register_schema(service_name, &sdl, stage).await
}

pub async fn get_schema_history(service: &str) -> Result<SchemaHistoryResponse, String> {
    let r: v1::SchemaHistoryResponse = rpc(
        "SchemaHistory",
        &v1::SchemaHistoryRequest {
            service_name: service.to_string(),
            namespace: None,
        },
    )
    .await?;
    Ok(SchemaHistoryResponse {
        versions: r.versions.into_iter().map(schema_version).collect(),
    })
}

pub async fn federation_status() -> Result<FederationStatusResponse, String> {
    let r: v1::FederationStatusResponse = rpc("FederationStatus", &v1::Empty {}).await?;
    Ok(FederationStatusResponse {
        contributing_services: r.contributing_services,
        type_count: r.type_count as usize,
        field_count: r.field_count as usize,
    })
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
    rpc_json(
        "AiRoute",
        &v1::AiRouteRequest {
            policy: policy.to_string(),
            min_context_length: None,
            candidates: candidates
                .iter()
                .map(|c| v1::RouteCandidate {
                    provider: c.provider.to_string(),
                    estimated_cost_usd_per_1k_tokens: c.estimated_cost_usd_per_1k_tokens,
                    estimated_latency_ms: c.estimated_latency_ms,
                    is_local: c.is_local,
                    context_length: c.context_length,
                })
                .collect(),
        },
    )
    .await
}

pub async fn db_list(table: &str) -> Result<DbRecordListResponse, String> {
    rpc_json("DbList", &v1::DbTable { table: table.to_string() }).await
}

pub async fn db_get(table: &str, key: &str) -> Result<DbRecordResponse, String> {
    rpc_json("DbGet", &v1::DbKey { table: table.to_string(), key: key.to_string() }).await
}

pub async fn db_put(table: &str, key: &str, value_json: &str) -> Result<DbRecordResponse, String> {
    let value: Value =
        serde_json::from_str(value_json).map_err(|e| format!("invalid JSON value: {e}"))?;
    rpc_json(
        "DbPut",
        &v1::DbPutRequest {
            table: table.to_string(),
            key: key.to_string(),
            value: Some(open_runo_proto::json_to_value(&value)),
        },
    )
    .await
}

pub async fn db_delete(table: &str, key: &str) -> Result<(), String> {
    let _: Value = rpc_json("DbDelete", &v1::DbKey { table: table.to_string(), key: key.to_string() }).await?;
    Ok(())
}

fn flag(f: v1::FeatureFlag) -> FeatureFlagResponse {
    FeatureFlagResponse {
        name: f.name,
        enabled: f.enabled,
        rollout_percent: u8::try_from(f.rollout_percent).unwrap_or(u8::MAX),
        description: f.description,
    }
}

pub async fn feature_flag_list() -> Result<FeatureFlagListResponse, String> {
    let r: v1::ListFeatureFlagsResponse = rpc("ListFeatureFlags", &v1::Empty {}).await?;
    Ok(FeatureFlagListResponse {
        flags: r.flags.into_iter().map(flag).collect(),
    })
}

pub async fn feature_flag_upsert(
    name: &str,
    enabled: bool,
    rollout_percent: u8,
    description: &str,
) -> Result<FeatureFlagResponse, String> {
    let f: v1::FeatureFlag = rpc(
        "UpsertFeatureFlag",
        &v1::UpsertFeatureFlagRequest {
            name: name.to_string(),
            enabled: Some(enabled),
            rollout_percent: Some(u32::from(rollout_percent)),
            description: Some(description.to_string()),
        },
    )
    .await?;
    Ok(flag(f))
}

pub async fn feature_flag_evaluate(name: &str, bucket_key: &str) -> Result<FeatureFlagEvaluationResponse, String> {
    let e: v1::FlagEvaluation = rpc(
        "EvaluateFeatureFlag",
        &v1::EvaluateFlagRequest {
            name: name.to_string(),
            bucket_key: bucket_key.to_string(),
        },
    )
    .await?;
    Ok(FeatureFlagEvaluationResponse {
        name: e.name,
        bucket_key: e.bucket_key,
        enabled: e.enabled,
    })
}

pub async fn feature_flag_delete(name: &str) -> Result<(), String> {
    let _: v1::Empty = rpc("DeleteFeatureFlag", &v1::FlagName { name: name.to_string() }).await?;
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
    rpc_json("ScimListUsers", &v1::ScimListRequest { filter: None }).await
}

pub async fn scim_create_user(user_name: &str, roles: Vec<&str>) -> Result<Value, String> {
    rpc_json(
        "ScimCreateUser",
        &v1::ScimCreateUserRequest {
            user_name: user_name.to_string(),
            roles: roles.into_iter().map(str::to_string).collect(),
        },
    )
    .await
}

pub async fn scim_delete_user(id: &str) -> Result<(), String> {
    let _: v1::Empty = rpc("ScimDeleteUser", &v1::ScimUserId { id: id.to_string() }).await?;
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
    let r: v1::PersistedQuery = rpc(
        "RegisterPersistedQuery",
        &v1::RegisterPersistedQueryRequest { query: query.to_string() },
    )
    .await?;
    Ok(PqRegisterResponse {
        hash: r.hash,
        registered_at: r.registered_at,
    })
}

pub async fn get_persisted_query(hash: &str) -> Result<PqQueryResponse, String> {
    let r: v1::PersistedQuery = rpc("GetPersistedQuery", &v1::PersistedQueryHash { hash: hash.to_string() }).await?;
    Ok(PqQueryResponse {
        hash: r.hash,
        query: r.query,
        registered_at: r.registered_at,
    })
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
    let r: v1::PurgeResponse = rpc("PurgeCache", &v1::PurgeRequest { path: path.to_string() }).await?;
    Ok(PurgeResponse { purged: r.purged })
}

pub async fn cache_purge_all() -> Result<PurgeResponse, String> {
    let r: v1::PurgeResponse = rpc("PurgeAllCache", &v1::Empty {}).await?;
    Ok(PurgeResponse { purged: r.purged })
}

pub async fn cache_ai_stats() -> Result<AiStatsResponse, String> {
    rpc_json("CacheAiStats", &v1::Empty {}).await
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
    rpc_json("BackupExport", &v1::Empty {}).await
}

pub async fn integrity_check() -> Result<IntegrityResponse, String> {
    rpc_json("IntegrityCheck", &v1::Empty {}).await
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
    rpc_json("AnalyticsRequestsPerMonth", &v1::Empty {}).await
}

pub async fn operations_summary() -> Result<OperationsResponse, String> {
    rpc_json("AnalyticsOperations", &v1::Empty {}).await
}
