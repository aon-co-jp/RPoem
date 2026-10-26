//! OpenAPI 3.0 spec for the REST admin surface.
//!
//! 2026-10-26: 手書きの静的JSONから、`open-runo-openapi`(poem-openapi相当の
//! 型駆動ビルダー)による生成へ移行した。各エンドポイントは
//! `Operation::get("/path").ok::<ResponseType>(..)` のように
//! `open-runo-api-types` の実際の構造体(`schemars::JsonSchema`)を指定して宣言する
//! ので、リクエスト/レスポンスの型・必須/任意・nullable・enumはサーバーの
//! 構造体から自動で出る(手でJSONを書くとずれる問題が構造的に無い)。
//! パスパラメータは `:name` から自動抽出、operationIdも自動生成される。
//!
//! 同じ定義から次も配信する(いずれも認証不要、仕様自体は形だけで
//! データを含まないため `/health` と同じ扱い):
//! - `GET /api/openapi.json` — OpenAPI 3.0.3
//! - `GET /api/docs` / `GET /api/redoc` — Swagger UI / ReDoc
//! - `GET /api/openapi.graphql` — Cosmo(WunderGraph)互換のGraphQL SDL
//! - `GET /api/openapi.tauri.json` — Tauri `invoke` マニフェスト
//! - `GET /api/client.ts` / `GET /api/client.tauri.ts` — TypeScriptクライアント
//!
//! ルーター定義(`lib.rs`)にあってここに無いルートは、テスト
//! `every_registered_route_is_documented` が検出して落ちる(ドキュメントの
//! 書き忘れ防止)。

// clippy: このモジュールの unwrap()/expect() は「固定ヘッダからのレスポンス構築」
// 等、構造上失敗しない箇所に限られる。
#![allow(clippy::expect_used, clippy::unwrap_used)]

use crate::hyper_compat::{json_response, Handler};
use hyper::StatusCode;
use open_runo_api_types as t;
use open_runo_openapi::{docs_html, DocsUi, Operation as Op, Spec, TsClient};
use serde_json::Value;
use std::sync::Arc;

const NOT_FOUND: &str = "Not found";

/// 全エンドポイントの型付き定義。`pub(crate)` なのは `mcp.rs` が同じ仕様を
/// MCPリソースとして出すため(仕様本体の唯一の正本)。
pub(crate) fn document() -> Spec {
    Spec::new("open-runo REST API", env!("CARGO_PKG_VERSION"))
        .description("REST admin surface for open-runo-router. GraphQL Federation is served separately at POST /graphql (GraphiQL at GET /graphql).")
        .api_key_header("X-Api-Key")
        .shared_error_typed::<t::RateLimitedResponse>(429, "RateLimited", "Rate limit exceeded; see Retry-After header and retry_after_secs in the body")
        // --- health / docs ---
        .op(Op::get("/health").id("health").summary("Service health check").tag("system").public().response_empty(200, "OK"))
        .op(Op::get("/healthz").id("healthz").summary("Kubernetes-style health alias").tag("system").public().response_empty(200, "OK"))
        .op(Op::get("/api/openapi.json").id("openapiJson").summary("This OpenAPI 3.0 document").tag("system").public().response_empty(200, "OpenAPI document"))
        .op(Op::get("/api/docs").id("docsSwagger").summary("Swagger UI").tag("system").public().response_empty(200, "text/html"))
        .op(Op::get("/api/redoc").id("docsRedoc").summary("ReDoc").tag("system").public().response_empty(200, "text/html"))
        .op(Op::get("/api/openapi.graphql").id("openapiGraphql").summary("GraphQL SDL generated from this API (Cosmo / WunderGraph compatible)").tag("system").public().response_empty(200, "text/plain"))
        .op(Op::get("/api/openapi.tauri.json").id("openapiTauri").summary("Tauri invoke manifest generated from this API").tag("system").public().response_empty(200, "OK"))
        .op(Op::get("/api/client.ts").id("clientTs").summary("TypeScript fetch client generated from this API").tag("system").public().response_empty(200, "text/plain"))
        .op(Op::get("/api/client.tauri.ts").id("clientTauriTs").summary("TypeScript Tauri-invoke client generated from this API").tag("system").public().response_empty(200, "text/plain"))
        .op(Op::get("/.well-known/acme-challenge/:token").id("acmeChallenge").summary("ACME HTTP-01 challenge response").tag("system").public().response_empty(200, "Key authorization").response_empty(404, NOT_FOUND))
        .op(Op::post("/api/keys/self-issue").id("selfIssueKey").summary("Self-issue an API key").tag("auth").public().response_empty(200, "Issued"))
        .op(Op::post("/api/session/login").id("sessionLogin").summary("Exchange an API key for an HttpOnly session cookie + CSRF token").tag("auth").public().response_empty(200, "Session started").response_empty(401, "Invalid API key"))
        .op(Op::post("/api/session/logout").id("sessionLogout").summary("End the current session").tag("auth").public().response_empty(200, "Session ended"))
        .op(Op::post("/mcp").id("mcp").summary("MCP server (JSON-RPC 2.0, Streamable HTTP)").tag("system").response_empty(200, "JSON-RPC response"))
        // --- schemas ---
        .op(Op::post("/api/schemas").summary("Register a schema version").tag("schemas").body::<t::RegisterSchemaRequest>().ok::<t::SchemaVersion>("Registered").response_empty(422, "Validation failed"))
        .op(Op::post("/api/schemas/upload").id("uploadSchemaMultipart").summary("Register a schema version from a multipart SDL upload").tag("schemas").body_raw("multipart/form-data").ok::<t::SchemaVersion>("Registered"))
        .op(Op::post("/api/schemas/upload-raw").id("uploadSchemaRaw").summary("Register a schema version from raw bytes (service_name, stage, namespace as query)").tag("schemas").query_param("service_name", true, "Service name").query_param("stage", false, "Stage").query_param("namespace", false, "Namespace").body_raw("application/octet-stream").ok::<t::SchemaVersion>("Registered"))
        .op(Op::get("/api/schemas/:service").summary("Latest schema for a service").tag("schemas").ok::<t::SchemaVersion>("OK").response_empty(404, NOT_FOUND))
        .op(Op::get("/api/schemas/:service/history").summary("Full schema version history").tag("schemas").ok::<t::SchemaHistoryResponse>("OK"))
        // --- federation / ai ---
        .op(Op::post("/api/federation/compose").summary("Compose service schemas into a federated graph").tag("federation").response_empty(200, "Composed"))
        .op(Op::get("/api/federation/status").summary("Current composed schema summary").tag("federation").ok::<t::FederationStatusResponse>("OK"))
        .op(Op::post("/api/ai/route").summary("Select the best AI provider for a request").tag("ai").response_empty(200, "OK"))
        // --- db ---
        .op(Op::get("/api/db/status").summary("DUAL DATABASE backend name & health").tag("db").ok::<t::DbStatusResponse>("OK"))
        .op(Op::get("/api/db/routing").summary("Per-table routing decisions").tag("db").ok::<t::DbRoutingInfo>("OK"))
        .op(Op::get("/api/db/:table").summary("List all records in a table").tag("db").ok::<t::DbRecordListResponse>("OK"))
        .op(Op::get("/api/db/:table/:key").summary("Get one record").tag("db").ok::<t::DbRecordResponse>("OK").response_empty(404, NOT_FOUND))
        .op(Op::get("/api/db/:table/:key/at/:commit_id").summary("Get one record as of a past commit").tag("db").ok::<t::DbRecordAtCommitResponse>("OK").response_empty(404, NOT_FOUND))
        .op(Op::put("/api/db/:table/:key").summary("Upsert a record").tag("db").body::<t::DbUpsertRequest>().ok::<t::DbRecordResponse>("Saved"))
        .op(Op::delete("/api/db/:table/:key").summary("Delete a record").tag("db").ok::<t::DbDeleteResponse>("Deleted"))
        .op(Op::post("/internal/db/mutate").id("internalDbMutate").summary("Internal replication endpoint (node-to-node)").tag("internal").response_empty(200, "Applied"))
        // --- cache / backup / migrate / integrity ---
        .op(Op::post("/api/cache/purge").summary("Purge one HTML page from the cache").tag("cache").response_empty(200, "Purged"))
        .op(Op::post("/api/cache/purge-all").summary("Purge the entire HTML page cache").tag("cache").response_empty(200, "Purged"))
        .op(Op::get("/api/cache/ai-stats").summary("Self-learning cache predictor stats").tag("cache").response_empty(200, "OK"))
        .op(Op::post("/api/backup/export").summary("Export a portable backup").tag("backup").response_empty(200, "Exported"))
        .op(Op::post("/api/backup/import").summary("Import a portable backup").tag("backup").response_empty(200, "Imported"))
        .op(Op::post("/api/backup/restore-latest").summary("Restore from the newest available backup").tag("backup").response_empty(200, "Restored").response_empty(404, "No backup found"))
        .op(Op::post("/api/migrate/export-sql").summary("SQL dump for engine migration (postgres/mysql/generic)").tag("migrate").response_empty(200, "OK"))
        .op(Op::post("/api/migrate/export-csv").summary("CSV export for BI/Snowflake ingestion").tag("migrate").response_empty(200, "OK"))
        .op(Op::post("/api/integrity/check").summary("Two-database reconciliation, self-healing").tag("db").response_empty(200, "OK"))
        .op(Op::get("/api/analytics/operations").id("analyticsOperations").summary("Per-operation request analytics").tag("analytics").response_empty(200, "OK"))
        .op(Op::get("/api/analytics/requests-per-month").id("analyticsRequestsPerMonth").summary("Monthly request counts").tag("analytics").response_empty(200, "OK"))
        // --- persisted queries ---
        .op(Op::post("/api/persisted-queries").summary("Register a GraphQL document (Trusted Documents)").tag("persisted-queries").response_empty(200, "Registered"))
        .op(Op::get("/api/persisted-queries/:hash").summary("Fetch a registered document by hash").tag("persisted-queries").response_empty(200, "OK").response_empty(404, NOT_FOUND))
        // --- feature flags ---
        .op(Op::post("/api/feature-flags").summary("Create or update a feature flag (upsert, keyed by name)").tag("feature-flags").body::<t::FeatureFlagRequest>().ok::<t::FeatureFlagResponse>("Upserted"))
        .op(Op::get("/api/feature-flags").summary("List all feature flags").tag("feature-flags").ok::<t::FeatureFlagListResponse>("OK"))
        .op(Op::get("/api/feature-flags/:name").summary("Fetch a feature flag").tag("feature-flags").ok::<t::FeatureFlagResponse>("OK").response_empty(404, NOT_FOUND))
        .op(Op::delete("/api/feature-flags/:name").summary("Delete a feature flag").tag("feature-flags").response_empty(200, "Deleted").response_empty(404, NOT_FOUND))
        .op(Op::get("/api/feature-flags/:name/evaluate").summary("Evaluate a feature flag for a given bucket key (deterministic 0-100 rollout bucketing)").tag("feature-flags").query_param("bucket_key", true, "Bucketing key").ok::<t::FeatureFlagEvaluationResponse>("OK").response_empty(404, NOT_FOUND))
        // --- SCIM ---
        .op(Op::get("/scim/v2/Users").id("scimListUsers").summary("List SCIM users (RFC 7644)").tag("scim").response_empty(200, "OK"))
        .op(Op::post("/scim/v2/Users").id("scimCreateUser").summary("Create a SCIM user (auto-issues an API key)").tag("scim").response_empty(201, "Created").response_empty(409, "userName already exists"))
        .op(Op::get("/scim/v2/Users/:id").id("scimGetUser").summary("Fetch a SCIM user").tag("scim").response_empty(200, "OK"))
        .op(Op::put("/scim/v2/Users/:id").id("scimReplaceUser").summary("Replace a SCIM user (deactivation auto-revokes keys)").tag("scim").response_empty(200, "OK"))
        .op(Op::delete("/scim/v2/Users/:id").id("scimDeleteUser").summary("Delete a SCIM user (auto-revokes keys)").tag("scim").response_empty(204, "Deleted"))
        .op(Op::get("/scim/v2/Groups").id("scimListGroups").summary("List SCIM groups").tag("scim").response_empty(200, "OK"))
        .op(Op::post("/scim/v2/Groups").id("scimCreateGroup").summary("Create a SCIM group").tag("scim").response_empty(201, "Created"))
        .op(Op::get("/scim/v2/Groups/:id").id("scimGetGroup").summary("Fetch a SCIM group").tag("scim").response_empty(200, "OK"))
        .op(Op::put("/scim/v2/Groups/:id").id("scimReplaceGroup").summary("Replace a SCIM group's membership").tag("scim").response_empty(200, "OK"))
        .op(Op::delete("/scim/v2/Groups/:id").id("scimDeleteGroup").summary("Delete a SCIM group").tag("scim").response_empty(204, "Deleted"))
        // --- streams ---
        .op(Op::get("/api/events").id("events").summary("Server-Sent Events stream of schema/federation changes").tag("streams").response_empty(200, "text/event-stream"))
        .op(Op::get("/api/ws-echo").id("wsEcho").summary("WebSocket echo (RFC 6455)").tag("streams").public().response_empty(101, "Switching Protocols"))
        .op(Op::get("/api/ws-events").id("wsEvents").summary("WebSocket stream of schema/federation changes").tag("streams").response_empty(101, "Switching Protocols"))
}

/// 後方互換: 既存の呼び出し元(MCPリソース等)が使うJSON文書。
pub(crate) fn spec() -> Value {
    document().build()
}

fn text_response(status: StatusCode, content_type: &'static str, body: String) -> crate::hyper_compat::Response {
    hyper::Response::builder()
        .status(status)
        .header("content-type", content_type)
        .body(crate::hyper_compat::fixed_body(hyper::body::Bytes::from(body)))
        .expect("fixed valid headers")
}

/// `GET /api/openapi.json` — serves the generated OpenAPI 3.0 document.
/// No auth required (matches `/health`'s exemption — the spec itself
/// contains no data, just endpoint shapes).
pub fn openapi_handler() -> Handler {
    Arc::new(move |_req, _params| Box::pin(async move { json_response(StatusCode::OK, &spec()) }))
}

/// `GET /api/docs`(Swagger UI)。
pub fn swagger_ui_handler() -> Handler {
    Arc::new(move |_req, _params| {
        Box::pin(async move {
            crate::hyper_compat::html_response(StatusCode::OK, docs_html(DocsUi::SwaggerUi, "open-runo REST API", "/api/openapi.json"))
        })
    })
}

/// `GET /api/redoc`(ReDoc)。
pub fn redoc_handler() -> Handler {
    Arc::new(move |_req, _params| {
        Box::pin(async move {
            crate::hyper_compat::html_response(StatusCode::OK, docs_html(DocsUi::Redoc, "open-runo REST API", "/api/openapi.json"))
        })
    })
}

/// `GET /api/openapi.graphql` — Cosmo/WunderGraph互換のGraphQL SDL。
pub fn graphql_sdl_handler() -> Handler {
    Arc::new(move |_req, _params| {
        Box::pin(async move { text_response(StatusCode::OK, "text/plain; charset=utf-8", document().to_graphql_sdl()) })
    })
}

/// `GET /api/openapi.tauri.json` — Tauri invokeマニフェスト。
pub fn tauri_manifest_handler() -> Handler {
    Arc::new(move |_req, _params| Box::pin(async move { json_response(StatusCode::OK, &document().to_tauri_manifest()) }))
}

/// `GET /api/client.ts` / `GET /api/client.tauri.ts` — TypeScriptクライアント。
pub fn typescript_handler(client: TsClient) -> Handler {
    Arc::new(move |_req, _params| {
        Box::pin(async move { text_response(StatusCode::OK, "text/plain; charset=utf-8", document().to_typescript(client)) })
    })
}

/// 任意の `Spec` から、ドキュメント一式のルートを `router` に足す。
/// `prefix`(例: `"/api"`)の下に次を追加する(全て認証不要で、仕様自体は
/// 形だけでデータを含まない):
/// `openapi.json` / `docs`(Swagger UI) / `redoc` / `openapi.graphql`(Cosmo互換SDL) /
/// `openapi.tauri.json` / `client.ts` / `client.tauri.ts`。
///
/// 他リポジトリのサーバー(`hyper_compat::Router` 利用)は、自分のルートを
/// `Spec` に宣言してこれを1回呼ぶだけで、型付きのAPIドキュメントを得られる。
pub fn mount_docs(router: crate::hyper_compat::Router, prefix: &str, spec: Spec) -> crate::hyper_compat::Router {
    use hyper::Method;
    let prefix = prefix.trim_end_matches('/');
    let title = spec.title().to_owned();
    let spec = Arc::new(spec);
    let json_url = format!("{prefix}/openapi.json");
    let s = Arc::clone(&spec);
    let h_json: Handler = Arc::new(move |_r, _p| {
        let s = Arc::clone(&s);
        Box::pin(async move { json_response(StatusCode::OK, &s.build()) })
    });
    let html = |ui: DocsUi| -> Handler {
        let page = docs_html(ui, &title, &json_url);
        Arc::new(move |_r, _p| {
            let page = page.clone();
            Box::pin(async move { crate::hyper_compat::html_response(StatusCode::OK, page) })
        })
    };
    let text = |f: fn(&Spec) -> String| -> Handler {
        let s = Arc::clone(&spec);
        Arc::new(move |_r, _p| {
            let s = Arc::clone(&s);
            Box::pin(async move { text_response(StatusCode::OK, "text/plain; charset=utf-8", f(&s)) })
        })
    };
    let s = Arc::clone(&spec);
    let h_tauri: Handler = Arc::new(move |_r, _p| {
        let s = Arc::clone(&s);
        Box::pin(async move { json_response(StatusCode::OK, &s.to_tauri_manifest()) })
    });
    router
        .route(Method::GET, &format!("{prefix}/openapi.json"), h_json)
        .route(Method::GET, &format!("{prefix}/docs"), html(DocsUi::SwaggerUi))
        .route(Method::GET, &format!("{prefix}/redoc"), html(DocsUi::Redoc))
        .route(Method::GET, &format!("{prefix}/openapi.graphql"), text(Spec::to_graphql_sdl))
        .route(Method::GET, &format!("{prefix}/openapi.tauri.json"), h_tauri)
        .route(Method::GET, &format!("{prefix}/client.ts"), text(|s| s.to_typescript(TsClient::Fetch)))
        .route(Method::GET, &format!("{prefix}/client.tauri.ts"), text(|s| s.to_typescript(TsClient::TauriInvoke)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hyper_compat::{serve, Router};
    use hyper::Method;

    #[tokio::test]
    async fn openapi_spec_is_valid_json_and_lists_known_paths() {
        let router = Router::new().route(Method::GET, "/api/openapi.json", openapi_handler());
        let (addr, _handle) = serve(router, "127.0.0.1:0".parse().unwrap())
            .await
            .expect("bind ephemeral port");

        let resp = reqwest::Client::new()
            .get(format!("http://{addr}/api/openapi.json"))
            .send()
            .await
            .expect("request should succeed");
        assert_eq!(resp.status(), reqwest::StatusCode::OK);
        let body: Value = resp.json().await.expect("valid json body");
        assert_eq!(body["openapi"], "3.0.3");
        assert!(body["paths"]["/health"].is_object());
        assert!(body["paths"]["/api/schemas"]["post"].is_object());
        assert!(body["paths"]["/scim/v2/Users/{id}"]["delete"].is_object());
        assert_eq!(
            body["components"]["securitySchemes"]["ApiKeyAuth"]["name"],
            "X-Api-Key"
        );

        // components.schemas is generated from open-runo-api-types, not
        // hand-written -- assert the shapes came through correctly,
        // including the nested-type $ref rewrite ($defs -> components/schemas).
        let schemas = &body["components"]["schemas"];
        assert_eq!(
            schemas["SchemaVersion"]["properties"]["sdl"]["type"], "string",
            "SchemaVersion should have every field of the real Rust struct, sdl included \
             (this is exactly the field that was missing from the register response before \
             open-runo-api-types existed)"
        );
        assert_eq!(
            schemas["SchemaHistoryResponse"]["properties"]["versions"]["items"]["$ref"],
            "#/components/schemas/SchemaVersion",
            "nested-type refs must point at components/schemas, not schemars' own $defs"
        );
        assert!(schemas["RegisterSchemaRequest"]["properties"]["service_name"].is_object());
        assert!(schemas["FederationStatusResponse"]["properties"]["type_count"].is_object());
        assert!(schemas["RateLimitedResponse"]["properties"]["retry_after_secs"].is_object());
        assert!(
            schemas["SchemaVersion"].get("$schema").is_none(),
            "the JSON-Schema-document-level $schema field shouldn't leak into an OpenAPI Schema Object"
        );

        // The 4 endpoints wired to typed schemas reference them by $ref,
        // and all four (plus every other protected endpoint) share the
        // same named 401/429 response components.
        assert_eq!(
            body["paths"]["/api/schemas"]["post"]["requestBody"]["content"]["application/json"]["schema"]["$ref"],
            "#/components/schemas/RegisterSchemaRequest"
        );
        assert_eq!(
            body["paths"]["/api/schemas"]["post"]["responses"]["200"]["content"]["application/json"]["schema"]["$ref"],
            "#/components/schemas/SchemaVersion"
        );
        assert_eq!(
            body["paths"]["/api/schemas"]["post"]["responses"]["429"]["$ref"],
            "#/components/responses/RateLimited"
        );
        assert_eq!(
            body["components"]["responses"]["RateLimited"]["content"]["application/json"]["schema"]["$ref"],
            "#/components/schemas/RateLimitedResponse"
        );
    }

    /// `docs/api-examples.md`'s "Coverage note" flagged DB CRUD and
    /// feature flags as still `description`-only (2026-07-11); this test
    /// pins the fix so it can't silently regress back to untyped.
    #[tokio::test]
    async fn db_and_feature_flag_endpoints_are_typed_and_feature_flags_are_documented() {
        let router = Router::new().route(Method::GET, "/api/openapi.json", openapi_handler());
        let (addr, _handle) = serve(router, "127.0.0.1:0".parse().unwrap())
            .await
            .expect("bind ephemeral port");
        let body: Value = reqwest::Client::new()
            .get(format!("http://{addr}/api/openapi.json"))
            .send()
            .await
            .expect("request should succeed")
            .json()
            .await
            .expect("valid json body");

        let schemas = &body["components"]["schemas"];
        for name in [
            "DbRecordItem",
            "DbRecordListResponse",
            "DbRecordResponse",
            "DbUpsertRequest",
            "DbDeleteResponse",
            "DbStatusResponse",
            "DbRoutingEntry",
            "DbRoutingInfo",
            "FeatureFlagRequest",
            "FeatureFlagResponse",
            "FeatureFlagListResponse",
            "FeatureFlagEvaluationResponse",
        ] {
            assert!(schemas[name].is_object(), "components.schemas.{name} should exist");
        }

        assert_eq!(
            body["paths"]["/api/db/{table}"]["get"]["responses"]["200"]["content"]["application/json"]["schema"]["$ref"],
            "#/components/schemas/DbRecordListResponse"
        );
        assert_eq!(
            body["paths"]["/api/db/{table}/{key}"]["put"]["requestBody"]["content"]["application/json"]["schema"]["$ref"],
            "#/components/schemas/DbUpsertRequest"
        );
        assert_eq!(
            body["paths"]["/api/db/{table}/{key}"]["delete"]["responses"]["200"]["content"]["application/json"]["schema"]["$ref"],
            "#/components/schemas/DbDeleteResponse"
        );

        // Feature flags were entirely absent from the OpenAPI paths before
        // this fix, despite being a real, wired REST feature
        // (`crates/open-runo-router/src/lib.rs` routes) with types already
        // sitting unused in `open-runo-api-types`.
        assert!(body["paths"]["/api/feature-flags"]["post"].is_object());
        assert!(body["paths"]["/api/feature-flags"]["get"].is_object());
        assert!(body["paths"]["/api/feature-flags/{name}"]["get"].is_object());
        assert!(body["paths"]["/api/feature-flags/{name}"]["delete"].is_object());
        assert!(body["paths"]["/api/feature-flags/{name}/evaluate"]["get"].is_object());
        assert_eq!(
            body["paths"]["/api/feature-flags"]["post"]["requestBody"]["content"]["application/json"]["schema"]["$ref"],
            "#/components/schemas/FeatureFlagRequest"
        );
        assert_eq!(
            body["paths"]["/api/feature-flags/{name}/evaluate"]["get"]["responses"]["200"]["content"]["application/json"]["schema"]["$ref"],
            "#/components/schemas/FeatureFlagEvaluationResponse"
        );
    }
}

#[cfg(test)]
mod generated_tests {
    use super::*;

    /// lib.rsの `.route(Method::X, "/path", ...)` を文字列として走査して、
    /// 登録済みルートが全てドキュメントにあることを確認する(書き忘れ防止)。
    #[test]
    fn every_registered_route_is_documented() {
        let src = include_str!("lib.rs");
        let mut routes: Vec<(String, String)> = Vec::new();
        let mut rest = src;
        while let Some(i) = rest.find("Method::") {
            rest = &rest[i + 8..];
            let method: String = rest.chars().take_while(|c| c.is_ascii_alphabetic()).collect();
            let after = rest[method.len()..].trim_start();
            let Some(after) = after.strip_prefix(',') else { continue };
            let after = after.trim_start();
            if let Some(p) = after.strip_prefix('"') {
                if let Some(end) = p.find('"') {
                    routes.push((method, p[..end].to_string()));
                }
            }
        }
        assert!(routes.len() > 40, "route scan broke: {}", routes.len());
        // 静的アセット(WASM管理UI)はAPIではないので対象外。
        let routes: Vec<(String, String)> = routes
            .into_iter()
            .filter(|(_, p)| !(p == "/" || p.starts_with("/pkg/") || p == "/icon.svg" || p == "/manifest.json"))
            .collect();
        let doc = document();
        let missing = doc.undocumented(routes.iter().map(|(m, p)| (m.as_str(), p.as_str())));
        assert!(missing.is_empty(), "ドキュメント未定義のルート: {missing:?}");
    }

    #[test]
    fn document_has_no_lint_problems() {
        let problems = document().lint();
        assert!(problems.is_empty(), "{problems:#?}");
    }

    #[test]
    fn graphql_sdl_and_ts_clients_are_generated_from_the_same_definitions() {
        let sdl = document().to_graphql_sdl();
        assert!(sdl.contains("type SchemaVersion {"), "{sdl}");
        assert!(sdl.contains("input RegisterSchemaRequestInput {"), "{sdl}");
        let ts = document().to_typescript(TsClient::TauriInvoke);
        assert!(ts.contains("export type SchemaVersion"));
        assert!(ts.contains("invoke<"));
        let m = document().to_tauri_manifest();
        assert!(m["commands"].as_array().unwrap().len() > 40);
    }

    /// 生成したSDLを、このリポジトリ自身のフェデレーション用SDLパーサーで
    /// 読み、QueryとMutationに操作が入っていることを確認する(Cosmoの
    /// サブグラフとして登録できる形になっている証拠)。
    #[test]
    fn generated_sdl_is_accepted_by_the_federation_sdl_parser() {
        let sdl = document().to_graphql_sdl();
        let svc = open_runo_federation::parse_service_sdl("rest-api", &sdl).expect("SDL parses");
        let q = svc.types.get("Query").expect("Query type");
        assert!(q.contains("getApiFederationStatus"), "{q:?}");
        let m = svc.types.get("Mutation").expect("Mutation type");
        assert!(m.contains("postApiSchemas"), "{m:?}");
        assert!(svc.types.contains_key("SchemaVersion"));
    }

    #[tokio::test]
    async fn docs_and_bridge_endpoints_are_served_over_real_http() {
        use crate::hyper_compat::{serve, Router};
        use hyper::Method;
        let router = Router::new()
            .route(Method::GET, "/api/docs", swagger_ui_handler())
            .route(Method::GET, "/api/redoc", redoc_handler())
            .route(Method::GET, "/api/openapi.graphql", graphql_sdl_handler())
            .route(Method::GET, "/api/openapi.tauri.json", tauri_manifest_handler())
            .route(Method::GET, "/api/client.ts", typescript_handler(TsClient::Fetch));
        let (addr, _h) = serve(router, "127.0.0.1:0".parse().unwrap()).await.expect("bind");
        let get = |p: &'static str| async move {
            let r = reqwest::get(format!("http://{addr}{p}")).await.expect("request");
            assert_eq!(r.status(), 200, "{p}");
            let ct = r.headers()["content-type"].to_str().unwrap().to_owned();
            (ct, r.text().await.unwrap())
        };
        let (ct, html) = get("/api/docs").await;
        assert!(ct.starts_with("text/html") && html.contains("/api/openapi.json"));
        assert!(get("/api/redoc").await.1.contains("Redoc.init"));
        let (ct, sdl) = get("/api/openapi.graphql").await;
        assert!(ct.starts_with("text/plain") && sdl.contains("type Query {"));
        let (_, tauri) = get("/api/openapi.tauri.json").await;
        assert!(serde_json::from_str::<Value>(&tauri).unwrap()["commands"].is_array());
        assert!(get("/api/client.ts").await.1.contains("createClient"));
    }

    /// 目視確認用: `cargo test -p open-runo-router --lib dump_generated -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn dump_generated() {
        println!("{}", document().to_graphql_sdl());
        println!("{}", document().to_typescript(TsClient::Fetch));
    }

    #[tokio::test]
    async fn mount_docs_serves_a_custom_spec_for_other_services() {
        use crate::hyper_compat::{serve, Router};
        #[derive(serde::Serialize, schemars::JsonSchema)]
        struct Pong {
            ok: bool,
        }
        let spec = Spec::new("custom", "9.9.9").op(Op::get("/ping").summary("ping").public().ok::<Pong>("pong"));
        let router = mount_docs(Router::new(), "/docs-root/", spec);
        let (addr, _h) = serve(router, "127.0.0.1:0".parse().unwrap()).await.expect("bind");
        let doc: Value = reqwest::get(format!("http://{addr}/docs-root/openapi.json")).await.unwrap().json().await.unwrap();
        assert_eq!(doc["info"]["title"], "custom");
        assert_eq!(doc["paths"]["/ping"]["get"]["responses"]["200"]["content"]["application/json"]["schema"]["$ref"], "#/components/schemas/Pong");
        let html = reqwest::get(format!("http://{addr}/docs-root/docs")).await.unwrap().text().await.unwrap();
        assert!(html.contains("/docs-root/openapi.json"));
        let sdl = reqwest::get(format!("http://{addr}/docs-root/openapi.graphql")).await.unwrap().text().await.unwrap();
        assert!(sdl.contains("getPing: Pong"), "{sdl}");
    }
}
