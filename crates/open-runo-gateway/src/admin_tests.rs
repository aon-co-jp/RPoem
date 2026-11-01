//! REST版とGraphQL版が**同じ結果**になることの確認。各テストはRESTを叩く
//! 環境AとGraphQLを叩く環境Bを別々に用意し、同じ操作列を流して応答を比べる
//! (id/時刻など実行ごとに変わる値は比較しない)。
use open_runo_router::hyper_compat::serve;
use open_runo_router::keyring::{GuardianConfig, KeyGuardian};
use open_runo_router::state::AppState;
use serde_json::{json, Value};
use std::sync::Arc;

struct Env {
    state: Arc<AppState>,
    base: String,
    client: reqwest::Client,
    _handle: tokio::task::JoinHandle<()>,
}

async fn env() -> Env {
    let state = Arc::new(AppState::new());
    let (_get, post) = crate::graphql_hyper::graphql_handlers(Arc::clone(&state));
    let app = open_runo_router::build_hyper_app(Arc::clone(&state), 10_000, 60)
        .await
        .route(hyper::Method::POST, "/graphql", post);
    let (addr, handle) = serve(app, "127.0.0.1:0".parse().unwrap())
        .await
        .expect("bind");
    Env {
        state,
        base: format!("http://{addr}"),
        client: reqwest::Client::new(),
        _handle: tokio::spawn(async move {
            let _ = handle.await;
        }),
    }
}

impl Env {
    async fn rest(&self, method: reqwest::Method, path: &str, body: Option<Value>) -> (u16, Value) {
        let mut req = self
            .client
            .request(method, format!("{}{path}", self.base))
            .header("x-api-key", "devkey");
        if let Some(b) = body {
            req = req.json(&b);
        }
        let resp = req.send().await.expect("rest");
        let status = resp.status().as_u16();
        let text = resp.text().await.unwrap_or_default();
        (status, serde_json::from_str(&text).unwrap_or(Value::Null))
    }

    /// GraphQL。成功時は`data`、失敗時は先頭エラーの(`extensions.status`, message)。
    async fn gql(&self, query: &str, key: Option<&str>) -> Result<Value, (i64, String)> {
        let mut req = self
            .client
            .post(format!("{}/graphql", self.base))
            .json(&json!({ "query": query }));
        // None = 開発用の既定鍵(レジストリが空なら通る)。Some("")は鍵なし。
        match key.unwrap_or("devkey") {
            "" => {},
            k => req = req.header("x-api-key", k),
        }
        let v: Value = req.send().await.expect("gql").json().await.expect("json");
        if let Some(err) = v["errors"].get(0) {
            return Err((
                err["extensions"]["status"].as_i64().unwrap_or(0),
                err["message"].as_str().unwrap_or_default().to_string(),
            ));
        }
        Ok(v["data"].clone())
    }
}

const SDL: &str = "type Query { users: [User] } type User { id: ID! name: String }";

#[tokio::test]
async fn schema_register_get_history_match_rest() {
    let (r, g) = (env().await, env().await);

    let (st, rv) = r
        .rest(
            reqwest::Method::POST,
            "/api/schemas",
            Some(json!({ "service_name": "users", "sdl": SDL, "stage": "staging" })),
        )
        .await;
    assert_eq!(st, 200);
    let gv = g
        .gql(
            &format!(
                "mutation {{ admin {{ registerSchema(serviceName: \"users\", sdl: {}, stage: \"staging\") {{ namespace serviceName sdl stage }} }} }}",
                serde_json::to_string(SDL).unwrap()
            ),
            None,
        )
        .await
        .unwrap();
    let rs = &gv["admin"]["registerSchema"];
    assert_eq!(rs["namespace"], rv["namespace"]);
    assert_eq!(rs["serviceName"], rv["service_name"]);
    assert_eq!(rs["sdl"], rv["sdl"]);
    assert_eq!(rs["stage"], rv["stage"]);

    // upload(ファイル内容)も同じ登録経路。履歴が2件になる。
    g.gql(
        &format!(
            "mutation {{ admin {{ uploadSchema(serviceName: \"users\", sdlFile: {}) {{ stage }} }} }}",
            serde_json::to_string(SDL).unwrap()
        ),
        None,
    )
    .await
    .unwrap();

    let (_, rget) = r
        .rest(
            reqwest::Method::GET,
            "/api/schemas/users?stage=staging",
            None,
        )
        .await;
    let gget = g
        .gql(
            "{ admin { schema(serviceName: \"users\", stage: \"staging\") { sdl stage } } }",
            None,
        )
        .await
        .unwrap();
    assert_eq!(gget["admin"]["schema"]["sdl"], rget["sdl"]);
    assert_eq!(gget["admin"]["schema"]["stage"], rget["stage"]);

    let (_, rh) = r
        .rest(reqwest::Method::GET, "/api/schemas/users/history", None)
        .await;
    let gh = g
        .gql(
            "{ admin { schemaHistory(serviceName: \"users\") { serviceName stage } } }",
            None,
        )
        .await
        .unwrap();
    assert_eq!(rh["versions"].as_array().unwrap().len(), 1);
    assert_eq!(gh["admin"]["schemaHistory"].as_array().unwrap().len(), 2);

    // 未登録は404(REST)/ extensions.status=404(GraphQL)。
    let (st, _) = r
        .rest(reqwest::Method::GET, "/api/schemas/nope", None)
        .await;
    assert_eq!(st, 404);
    let err = g
        .gql("{ admin { schema(serviceName: \"nope\") { sdl } } }", None)
        .await
        .unwrap_err();
    assert_eq!(err.0, 404);
    assert!(g
        .state
        .schema_registry
        .lock()
        .unwrap()
        .latest("users", open_runo_schema_registry::Stage::Staging)
        .is_some());
}

#[tokio::test]
async fn federation_compose_and_status_match_rest() {
    let (r, g) = (env().await, env().await);
    let (st, rc) = r
        .rest(
            reqwest::Method::POST,
            "/api/federation/compose",
            Some(json!({ "services": [
                { "service_name": "a", "types": { "User": ["id", "name"] } },
                { "service_name": "b", "types": { "User": ["id", "email"], "Post": ["id"] } },
            ]})),
        )
        .await;
    assert_eq!(st, 200);
    let gc = g
        .gql(
            r#"mutation { admin { composeFederation(services: [
                { serviceName: "a", types: [{ typeName: "User", fields: ["id", "name"] }] },
                { serviceName: "b", types: [{ typeName: "User", fields: ["id", "email"] }, { typeName: "Post", fields: ["id"] }] }
            ]) { contributingServices types { typeName fields } breakingChanges } } }"#,
            None,
        )
        .await
        .unwrap();
    let c = &gc["admin"]["composeFederation"];
    assert_eq!(c["contributingServices"], rc["contributing_services"]);
    assert_eq!(c["breakingChanges"], rc["breaking_changes"]);
    let gql_user = c["types"]
        .as_array()
        .unwrap()
        .iter()
        .find(|t| t["typeName"] == "User")
        .unwrap()["fields"]
        .clone();
    assert_eq!(gql_user, rc["types"]["User"]);

    let (_, rs) = r
        .rest(reqwest::Method::GET, "/api/federation/status", None)
        .await;
    let gs = g
        .gql(
            "{ admin { federationStatus { contributingServices typeCount fieldCount } } }",
            None,
        )
        .await
        .unwrap();
    let s = &gs["admin"]["federationStatus"];
    assert_eq!(s["contributingServices"], rs["contributing_services"]);
    assert_eq!(s["typeCount"], rs["type_count"]);
    assert_eq!(s["fieldCount"], rs["field_count"]);

    // 不正SDLはRESTと同じく422。
    let (st, _) = r
        .rest(
            reqwest::Method::POST,
            "/api/federation/compose",
            Some(json!({ "services": [{ "service_name": "x", "sdl": "type {{{" }] })),
        )
        .await;
    assert_eq!(st, 422);
    let err = g
        .gql(
            r#"mutation { admin { composeFederation(services: [{ serviceName: "x", sdl: "type {{{" }]) { contributingServices } } }"#,
            None,
        )
        .await
        .unwrap_err();
    assert_eq!(err.0, 422);
}

#[tokio::test]
async fn feature_flags_match_rest() {
    let (r, g) = (env().await, env().await);
    let (st, rf) = r
        .rest(
            reqwest::Method::POST,
            "/api/feature-flags",
            Some(json!({ "name": "canary", "enabled": true, "rollout_percent": 40, "description": "d" })),
        )
        .await;
    assert_eq!(st, 200);
    let gf = g
        .gql(
            r#"mutation { admin { upsertFeatureFlag(name: "canary", enabled: true, rolloutPercent: 40, description: "d") { name enabled rolloutPercent description } } }"#,
            None,
        )
        .await
        .unwrap();
    let f = &gf["admin"]["upsertFeatureFlag"];
    assert_eq!(f["name"], rf["name"]);
    assert_eq!(f["rolloutPercent"], rf["rollout_percent"]);
    assert_eq!(f["description"], rf["description"]);

    let (_, rl) = r
        .rest(reqwest::Method::GET, "/api/feature-flags", None)
        .await;
    let gl = g
        .gql("{ admin { featureFlags { name } } }", None)
        .await
        .unwrap();
    assert_eq!(
        gl["admin"]["featureFlags"].as_array().unwrap().len(),
        rl["flags"].as_array().unwrap().len()
    );

    // 評価(同じバケットキーなら同じ結果)。
    for key in ["u1", "u2", "u3", "u4", "u5", "u6"] {
        let (_, re) = r
            .rest(
                reqwest::Method::GET,
                &format!("/api/feature-flags/canary/evaluate?bucket_key={key}"),
                None,
            )
            .await;
        let ge = g
            .gql(
                &format!("{{ admin {{ evaluateFeatureFlag(name: \"canary\", bucketKey: \"{key}\") {{ enabled }} }} }}"),
                None,
            )
            .await
            .unwrap();
        assert_eq!(ge["admin"]["evaluateFeatureFlag"]["enabled"], re["enabled"]);
    }

    // 不正値(rollout 101)はどちらも422、未知フラグは404。
    let (st, _) = r
        .rest(
            reqwest::Method::POST,
            "/api/feature-flags",
            Some(json!({ "name": "bad", "rollout_percent": 101 })),
        )
        .await;
    assert_eq!(st, 422);
    let err = g
        .gql(
            r#"mutation { admin { upsertFeatureFlag(name: "bad", rolloutPercent: 101) { name } } }"#,
            None,
        )
        .await
        .unwrap_err();
    assert_eq!(err.0, 422);
    assert_eq!(
        r.rest(reqwest::Method::GET, "/api/feature-flags/none", None)
            .await
            .0,
        404
    );
    assert_eq!(
        g.gql(r#"{ admin { featureFlag(name: "none") { name } } }"#, None)
            .await
            .unwrap_err()
            .0,
        404
    );

    // 削除(2回目は404)。
    assert_eq!(
        r.rest(reqwest::Method::DELETE, "/api/feature-flags/canary", None)
            .await
            .0,
        200
    );
    let gd = g
        .gql(
            r#"mutation { admin { deleteFeatureFlag(name: "canary") } }"#,
            None,
        )
        .await
        .unwrap();
    assert_eq!(gd["admin"]["deleteFeatureFlag"], true);
    assert_eq!(
        g.gql(
            r#"mutation { admin { deleteFeatureFlag(name: "canary") } }"#,
            None
        )
        .await
        .unwrap_err()
        .0,
        404
    );
}

#[tokio::test]
async fn persisted_queries_match_rest() {
    let (r, g) = (env().await, env().await);
    let (st, rp) = r
        .rest(
            reqwest::Method::POST,
            "/api/persisted-queries",
            Some(json!({ "query": "{ health }" })),
        )
        .await;
    assert_eq!(st, 200);
    let gp = g
        .gql(
            r#"mutation { admin { registerPersistedQuery(query: "{ health }") { hash query } } }"#,
            None,
        )
        .await
        .unwrap();
    assert_eq!(gp["admin"]["registerPersistedQuery"]["hash"], rp["hash"]);

    let hash = rp["hash"].as_str().unwrap();
    let (_, rg) = r
        .rest(
            reqwest::Method::GET,
            &format!("/api/persisted-queries/{hash}"),
            None,
        )
        .await;
    let gg = g
        .gql(
            &format!("{{ admin {{ persistedQuery(hash: \"{hash}\") {{ hash query }} }} }}"),
            None,
        )
        .await
        .unwrap();
    assert_eq!(gg["admin"]["persistedQuery"]["query"], rg["query"]);
    assert_eq!(
        g.gql(
            r#"{ admin { persistedQuery(hash: "deadbeef") { hash } } }"#,
            None
        )
        .await
        .unwrap_err()
        .0,
        404
    );
}

#[tokio::test]
async fn cache_purge_matches_rest() {
    let (r, g) = (env().await, env().await);
    let (st, rp) = r
        .rest(
            reqwest::Method::POST,
            "/api/cache/purge",
            Some(json!({ "path": "/a" })),
        )
        .await;
    assert_eq!(st, 200);
    let gp = g
        .gql(
            r#"mutation { admin { purgeCache(path: "/a") { purged } } }"#,
            None,
        )
        .await
        .unwrap();
    assert_eq!(gp["admin"]["purgeCache"]["purged"], rp["purged"]);

    let (_, ra) = r
        .rest(reqwest::Method::POST, "/api/cache/purge-all", None)
        .await;
    let ga = g
        .gql("mutation { admin { purgeAllCache { purged } } }", None)
        .await
        .unwrap();
    assert_eq!(ga["admin"]["purgeAllCache"]["purged"], ra["purged"]);
}

#[tokio::test]
async fn admin_requires_valid_key_like_rest() {
    let e = env().await;
    // 鍵を1つ発行するとレジストリが空でなくなり、無効な鍵は拒否される。
    let guardian = KeyGuardian::new(Arc::clone(&e.state.db), GuardianConfig::from_env());
    let key = guardian.issue("tester", vec![], None).await.unwrap();

    assert_eq!(
        e.gql("{ admin { featureFlags { name } } }", Some(""))
            .await
            .unwrap_err()
            .0,
        401
    );
    assert_eq!(
        e.gql("{ admin { featureFlags { name } } }", Some("bogus"))
            .await
            .unwrap_err()
            .0,
        401
    );
    assert_eq!(
        e.gql("mutation { admin { purgeAllCache { purged } } }", Some(""))
            .await
            .unwrap_err()
            .0,
        401
    );
    assert_eq!(
        e.rest(reqwest::Method::GET, "/api/feature-flags", None)
            .await
            .0,
        401
    );

    // 正しい鍵なら通る。公開フィールド(health)は従来どおり鍵なしで動く。
    let ok = e
        .gql("{ admin { featureFlags { name } } }", Some(&key))
        .await
        .unwrap();
    assert!(ok["admin"]["featureFlags"].is_array());
    assert_eq!(e.gql("{ health }", None).await.unwrap()["health"], "ok");
}

// ── 第2段階 ─────────────────────────────────────────────────────────────

#[tokio::test]
async fn db_operations_match_rest() {
    let (r, g) = (env().await, env().await);

    let (_, rs) = r.rest(reqwest::Method::GET, "/api/db/status", None).await;
    let gs = g.gql("{ admin { dbStatus } }", None).await.unwrap();
    assert_eq!(gs["admin"]["dbStatus"], rs);

    let (_, rr) = r.rest(reqwest::Method::GET, "/api/db/routing", None).await;
    let gr = g.gql("{ admin { dbRouting } }", None).await.unwrap();
    assert_eq!(gr["admin"]["dbRouting"], rr);

    let value = json!({ "n": 1, "tags": ["a", "b"] });
    let (st, rp) = r
        .rest(
            reqwest::Method::PUT,
            "/api/db/notes/k1",
            Some(json!({ "value": value })),
        )
        .await;
    assert_eq!(st, 200);
    let gp = g
        .gql(
            r#"mutation { admin { dbPut(table: "notes", key: "k1", value: {n: 1, tags: ["a","b"]}) } }"#,
            None,
        )
        .await
        .unwrap();
    assert_eq!(gp["admin"]["dbPut"], rp);

    let (_, rg) = r.rest(reqwest::Method::GET, "/api/db/notes/k1", None).await;
    let gg = g
        .gql(r#"{ admin { dbGet(table: "notes", key: "k1") } }"#, None)
        .await
        .unwrap();
    assert_eq!(gg["admin"]["dbGet"], rg);

    let (_, rl) = r.rest(reqwest::Method::GET, "/api/db/notes", None).await;
    let gl = g
        .gql(r#"{ admin { dbList(table: "notes") } }"#, None)
        .await
        .unwrap();
    assert_eq!(gl["admin"]["dbList"], rl);

    // 履歴読み出しはインメモリ実装では未対応(501)。RESTと同じ。
    let (st, _) = r
        .rest(reqwest::Method::GET, "/api/db/notes/k1/at/abc", None)
        .await;
    let err = g
        .gql(
            r#"{ admin { dbGetAtCommit(table: "notes", key: "k1", commitId: "abc") } }"#,
            None,
        )
        .await
        .unwrap_err();
    assert_eq!(i64::from(st), err.0);

    let (_, rd) = r
        .rest(reqwest::Method::DELETE, "/api/db/notes/k1", None)
        .await;
    let gd = g
        .gql(
            r#"mutation { admin { dbDelete(table: "notes", key: "k1") } }"#,
            None,
        )
        .await
        .unwrap();
    assert_eq!(gd["admin"]["dbDelete"], rd);
    assert_eq!(
        r.rest(reqwest::Method::GET, "/api/db/notes/k1", None)
            .await
            .0,
        404
    );
    assert_eq!(
        g.gql(r#"{ admin { dbGet(table: "notes", key: "k1") } }"#, None)
            .await
            .unwrap_err()
            .0,
        404
    );
}

#[tokio::test]
async fn analytics_and_ai_route_match_rest() {
    let (r, g) = (env().await, env().await);

    let (_, ra) = r
        .rest(
            reqwest::Method::GET,
            "/api/analytics/requests-per-month",
            None,
        )
        .await;
    let ga = g
        .gql("{ admin { analyticsRequestsPerMonth } }", None)
        .await
        .unwrap();
    // 件数は直前のリクエスト数で変わるため、形だけ比べる。
    assert!(ra["months"].is_array());
    assert!(ga["admin"]["analyticsRequestsPerMonth"]["months"].is_array());
    let (_, ro) = r
        .rest(reqwest::Method::GET, "/api/analytics/operations", None)
        .await;
    let go = g
        .gql("{ admin { analyticsOperations } }", None)
        .await
        .unwrap();
    assert!(ro["operations"].is_array());
    assert!(go["admin"]["analyticsOperations"]["operations"].is_array());

    let (_, rs) = r
        .rest(reqwest::Method::GET, "/api/cache/ai-stats", None)
        .await;
    let gs = g.gql("{ admin { cacheAiStats } }", None).await.unwrap();
    assert_eq!(gs["admin"]["cacheAiStats"], rs);

    let (st, rr) = r
        .rest(
            reqwest::Method::POST,
            "/api/ai/route",
            Some(json!({ "policy": "cost", "min_context_length": 8000, "candidates": [
                { "provider": "openai", "estimated_cost_usd_per_1k_tokens": 0.5, "estimated_latency_ms": 300, "is_local": false, "context_length": 128000 },
                { "provider": "local", "estimated_cost_usd_per_1k_tokens": 0.0, "estimated_latency_ms": 900, "is_local": true, "context_length": 32000 },
            ]})),
        )
        .await;
    assert_eq!(st, 200);
    let gr = g
        .gql(
            r#"{ admin { aiRoute(policy: "cost", minContextLength: 8000, candidates: [
                { provider: "openai", estimatedCostUsdPer1KTokens: 0.5, estimatedLatencyMs: 300, isLocal: false, contextLength: 128000 },
                { provider: "local", estimatedCostUsdPer1KTokens: 0.0, estimatedLatencyMs: 900, isLocal: true, contextLength: 32000 }
            ]) } }"#,
            None,
        )
        .await
        .unwrap();
    assert_eq!(gr["admin"]["aiRoute"], rr);

    // 候補が条件を満たさない場合は同じ422。
    let (st, _) = r
        .rest(
            reqwest::Method::POST,
            "/api/ai/route",
            Some(json!({ "policy": "cost", "min_context_length": 999999, "candidates": [
                { "provider": "openai", "estimated_cost_usd_per_1k_tokens": 0.5, "estimated_latency_ms": 300, "is_local": false, "context_length": 1000 },
            ]})),
        )
        .await;
    let err = g
        .gql(
            r#"{ admin { aiRoute(policy: "cost", minContextLength: 999999, candidates: [
                { provider: "openai", estimatedCostUsdPer1KTokens: 0.5, estimatedLatencyMs: 300, isLocal: false, contextLength: 1000 }
            ]) } }"#,
            None,
        )
        .await
        .unwrap_err();
    assert_eq!(i64::from(st), err.0);
}

#[tokio::test]
async fn backup_migrate_integrity_match_rest() {
    let (r, g) = (env().await, env().await);
    for e in [&r, &g] {
        e.state.db.put("notes", "k", "{\"v\":1}").await.unwrap();
    }

    let (st, ri) = r
        .rest(reqwest::Method::POST, "/api/integrity/check", None)
        .await;
    assert_eq!(st, 200);
    let gi = g
        .gql("mutation { admin { integrityCheck } }", None)
        .await
        .unwrap();
    assert_eq!(gi["admin"]["integrityCheck"], ri);

    // バックアップは書き出し先のパスが毎回違うので、件数と形を比べる。
    let (st, rb) = r
        .rest(reqwest::Method::POST, "/api/backup/export", None)
        .await;
    assert_eq!(st, 200);
    let gb = g
        .gql("mutation { admin { backupExport } }", None)
        .await
        .unwrap();
    let gb = &gb["admin"]["backupExport"];
    assert_eq!(gb["records"], rb["records"]);
    assert_eq!(
        gb["written"].as_array().unwrap().len(),
        rb["written"].as_array().unwrap().len()
    );

    // 書き出したファイルを両方の経路で読み戻せる(同じ件数)。
    let path = rb["written"][0].as_str().unwrap().to_string();
    let (st, rimp) = r
        .rest(
            reqwest::Method::POST,
            "/api/backup/import",
            Some(json!({ "path": path })),
        )
        .await;
    assert_eq!(st, 200);
    let gimp = g
        .gql(
            &format!(
                r#"mutation {{ admin {{ backupImport(path: {}) }} }}"#,
                serde_json::to_string(&path).unwrap()
            ),
            None,
        )
        .await
        .unwrap();
    assert_eq!(gimp["admin"]["backupImport"], rimp);
    let (st, _) = r
        .rest(
            reqwest::Method::POST,
            "/api/backup/import",
            Some(json!({ "path": "/nonexistent/backup.json" })),
        )
        .await;
    let err = g
        .gql(
            r#"mutation { admin { backupImport(path: "/nonexistent/backup.json") } }"#,
            None,
        )
        .await
        .unwrap_err();
    assert_eq!(i64::from(st), err.0);

    let (st, _) = r
        .rest(reqwest::Method::POST, "/api/backup/restore-latest", None)
        .await;
    let gr = g
        .gql("mutation { admin { backupRestoreLatest } }", None)
        .await;
    assert_eq!(st == 200, gr.is_ok());

    let (st, rsql) = r
        .rest(
            reqwest::Method::POST,
            "/api/migrate/export-sql",
            Some(json!({ "dialect": "postgres" })),
        )
        .await;
    assert_eq!(st, 200);
    let gsql = g
        .gql(
            r#"mutation { admin { migrateExportSql(dialect: "postgres") } }"#,
            None,
        )
        .await
        .unwrap();
    assert_eq!(
        gsql["admin"]["migrateExportSql"]["written"]
            .as_array()
            .unwrap()
            .len(),
        rsql["written"].as_array().unwrap().len()
    );
    assert_eq!(
        g.gql(
            r#"mutation { admin { migrateExportSql(dialect: "oracle") } }"#,
            None
        )
        .await
        .unwrap_err()
        .0,
        422
    );

    let (st, rcsv) = r
        .rest(reqwest::Method::POST, "/api/migrate/export-csv", None)
        .await;
    assert_eq!(st, 200);
    let gcsv = g
        .gql("mutation { admin { migrateExportCsv } }", None)
        .await
        .unwrap();
    assert_eq!(
        gcsv["admin"]["migrateExportCsv"]["written"]
            .as_array()
            .unwrap()
            .len(),
        rcsv["written"].as_array().unwrap().len()
    );
}

#[tokio::test]
async fn self_issue_and_session_flow_match_rest() {
    let e = env().await;

    // 鍵の自己発行: RESTもGraphQLも認証不要で、同じ形の応答。
    let (st, rk) = e
        .rest(reqwest::Method::POST, "/api/keys/self-issue", None)
        .await;
    assert_eq!(st, 200);
    let gk = e.gql("mutation { selfIssueKey }", Some("")).await.unwrap();
    let gk = &gk["selfIssueKey"];
    assert!(gk["api_key"].as_str().is_some_and(|k| !k.is_empty()));
    assert!(gk["expires_at"].is_string());
    assert!(rk["api_key"].is_string() && rk["expires_at"].is_string());
    let key = gk["api_key"].as_str().unwrap().to_string();

    // セッション: GraphQLのsessionLoginがSet-Cookieを返し、そのCookieで管理操作が通る。
    let resp = e
        .client
        .post(format!("{}/graphql", e.base))
        .header("x-api-key", &key)
        .json(&json!({ "query": "mutation { sessionLogin }" }))
        .send()
        .await
        .unwrap();
    let cookie = resp
        .headers()
        .get("set-cookie")
        .expect("Set-Cookie")
        .to_str()
        .unwrap()
        .split(';')
        .next()
        .unwrap()
        .to_string();
    let body: Value = resp.json().await.unwrap();
    let login = &body["data"]["sessionLogin"];
    assert!(login["owner"].is_string());
    let csrf = login["csrf_token"].as_str().unwrap().to_string();

    // Cookie+CSRFのみで管理Queryが通る(RESTのセッション認証と同じ検査)。
    let resp = e
        .client
        .post(format!("{}/graphql", e.base))
        .header("cookie", &cookie)
        .header("x-csrf-token", &csrf)
        .json(&json!({ "query": "{ admin { featureFlags { name } } }" }))
        .send()
        .await
        .unwrap();
    let body: Value = resp.json().await.unwrap();
    assert!(body["errors"].is_null(), "{body}");
    // CSRFヘッダ無しの更新系は拒否(RESTの403と同じ扱い=admin失敗)。
    let resp = e
        .client
        .post(format!("{}/graphql", e.base))
        .header("cookie", &cookie)
        .json(&json!({ "query": "{ admin { featureFlags { name } } }" }))
        .send()
        .await
        .unwrap();
    let body: Value = resp.json().await.unwrap();
    assert!(!body["errors"].is_null());

    // ログアウトでCookieが消え、そのセッションは使えなくなる。
    let resp = e
        .client
        .post(format!("{}/graphql", e.base))
        .header("cookie", &cookie)
        .header("x-csrf-token", &csrf)
        .json(&json!({ "query": "mutation { sessionLogout }" }))
        .send()
        .await
        .unwrap();
    assert!(resp.headers().get("set-cookie").is_some());
    let resp = e
        .client
        .post(format!("{}/graphql", e.base))
        .header("cookie", &cookie)
        .header("x-csrf-token", &csrf)
        .json(&json!({ "query": "{ admin { featureFlags { name } } }" }))
        .send()
        .await
        .unwrap();
    let body: Value = resp.json().await.unwrap();
    assert!(!body["errors"].is_null());

    // 無効な鍵でのログインは401(RESTも401)。
    let guardian = KeyGuardian::new(Arc::clone(&e.state.db), GuardianConfig::from_env());
    let _ = guardian.issue("t", vec![], None).await.unwrap();
    assert_eq!(
        e.gql("mutation { sessionLogin }", Some("bogus"))
            .await
            .unwrap_err()
            .0,
        401
    );
}
