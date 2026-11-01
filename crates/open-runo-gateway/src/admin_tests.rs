//! `admin`(GraphQL管理操作)の通し試験。実TCP+実HTTPで`POST /graphql`を叩く。
//!
//! 旧RESTは全廃済み(CLAUDE.md「2026-11-01追記」)。移行時に行ったREST/GraphQL同値性テスト
//! (同じ操作を両方へ流して結果とステータスを比較)の期待値を、リテラルとして残してある。
use open_runo_router::hyper_compat::{serve, Router};
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
    let app = Router::new().route(hyper::Method::POST, "/graphql", post);
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
    /// GraphQL。成功時は`data`、失敗時は先頭エラーの(`extensions.status`, message)。
    /// `key`: None=開発用の既定鍵(レジストリが空なら通る)、Some("")=鍵なし。
    async fn gql(&self, query: &str, key: Option<&str>) -> Result<Value, (i64, String)> {
        let mut req = self
            .client
            .post(format!("{}/graphql", self.base))
            .json(&json!({ "query": query }));
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

    async fn ok(&self, query: &str) -> Value {
        self.gql(query, None)
            .await
            .unwrap_or_else(|e| panic!("{query}: {e:?}"))
    }

    async fn status(&self, query: &str) -> i64 {
        self.gql(query, None).await.expect_err(query).0
    }
}

const SDL: &str = "type Query { users: [User] } type User { id: ID! name: String }";

#[tokio::test]
async fn schema_register_get_history() {
    let g = env().await;
    let sdl = serde_json::to_string(SDL).unwrap();

    let v = g
        .ok(&format!(
            "mutation {{ admin {{ registerSchema(serviceName: \"users\", sdl: {sdl}, stage: \"staging\") {{ namespace serviceName sdl stage id createdAt }} }} }}"
        ))
        .await;
    let rs = &v["admin"]["registerSchema"];
    assert_eq!(rs["namespace"], "default");
    assert_eq!(rs["serviceName"], "users");
    assert_eq!(rs["sdl"], SDL);
    assert_eq!(rs["stage"], "staging");
    assert!(rs["id"].as_str().is_some_and(|s| s.len() == 36));

    // upload(ファイル内容)も同じ登録経路。履歴が2件になる。
    g.ok(&format!(
        "mutation {{ admin {{ uploadSchema(serviceName: \"users\", sdlFile: {sdl}) {{ stage }} }} }}"
    ))
    .await;

    let got = g
        .ok("{ admin { schema(serviceName: \"users\", stage: \"staging\") { sdl stage } } }")
        .await;
    assert_eq!(got["admin"]["schema"]["sdl"], SDL);
    assert_eq!(got["admin"]["schema"]["stage"], "staging");

    let h = g
        .ok("{ admin { schemaHistory(serviceName: \"users\") { serviceName stage } } }")
        .await;
    assert_eq!(h["admin"]["schemaHistory"].as_array().unwrap().len(), 2);

    // 未登録は404。
    assert_eq!(
        g.status("{ admin { schema(serviceName: \"nope\") { sdl } } }")
            .await,
        404
    );
    // レジストリ(共通関数の副作用)へ反映されている。
    assert!(g
        .state
        .schema_registry
        .lock()
        .unwrap()
        .latest("users", open_runo_schema_registry::Stage::Staging)
        .is_some());
}

#[tokio::test]
async fn federation_compose_and_status() {
    let g = env().await;
    let c = g
        .ok(r#"mutation { admin { composeFederation(services: [
            { serviceName: "a", types: [{ typeName: "User", fields: ["id", "name"] }] },
            { serviceName: "b", types: [{ typeName: "User", fields: ["id", "email"] }, { typeName: "Post", fields: ["id"] }] }
        ]) { contributingServices types { typeName fields } breakingChanges } } }"#)
        .await;
    let c = &c["admin"]["composeFederation"];
    assert_eq!(c["contributingServices"], json!(["a", "b"]));
    assert_eq!(c["breakingChanges"], json!([]));
    let user = c["types"]
        .as_array()
        .unwrap()
        .iter()
        .find(|t| t["typeName"] == "User")
        .unwrap();
    assert_eq!(user["fields"], json!(["email", "id", "name"]));

    let s = g
        .ok("{ admin { federationStatus { contributingServices typeCount fieldCount } } }")
        .await;
    let s = &s["admin"]["federationStatus"];
    assert_eq!(s["contributingServices"], json!(["a", "b"]));
    assert_eq!(s["typeCount"], 2);
    assert_eq!(s["fieldCount"], 4);

    // 前回の合成から型が消えれば破壊的変更として報告される。
    let c2 = g
        .ok(r#"mutation { admin { composeFederation(services: [
            { serviceName: "a", types: [{ typeName: "User", fields: ["id"] }] }
        ]) { breakingChanges } } }"#)
        .await;
    assert!(!c2["admin"]["composeFederation"]["breakingChanges"]
        .as_array()
        .unwrap()
        .is_empty());

    // 不正SDLは422。
    assert_eq!(
        g.status(r#"mutation { admin { composeFederation(services: [{ serviceName: "x", sdl: "type {{{" }]) { contributingServices } } }"#)
            .await,
        422
    );
}

#[tokio::test]
async fn feature_flags_lifecycle() {
    let g = env().await;
    let f = g
        .ok(r#"mutation { admin { upsertFeatureFlag(name: "canary", enabled: true, rolloutPercent: 100, description: "d") { name enabled rolloutPercent description } } }"#)
        .await;
    let f = &f["admin"]["upsertFeatureFlag"];
    assert_eq!(f["name"], "canary");
    assert_eq!(f["rolloutPercent"], 100);
    assert_eq!(f["description"], "d");

    let l = g.ok("{ admin { featureFlags { name } } }").await;
    assert_eq!(l["admin"]["featureFlags"].as_array().unwrap().len(), 1);

    // 全量ロールアウトは常にtrue、0%は常にfalse(決定的)。
    for key in ["u1", "u2", "u3"] {
        let e = g
            .ok(&format!(
                "{{ admin {{ evaluateFeatureFlag(name: \"canary\", bucketKey: \"{key}\") {{ enabled bucketKey }} }} }}"
            ))
            .await;
        assert_eq!(e["admin"]["evaluateFeatureFlag"]["enabled"], true);
        assert_eq!(e["admin"]["evaluateFeatureFlag"]["bucketKey"], key);
    }
    g.ok(r#"mutation { admin { upsertFeatureFlag(name: "off", rolloutPercent: 0) { name } } }"#)
        .await;
    let e = g
        .ok(r#"{ admin { evaluateFeatureFlag(name: "off", bucketKey: "u1") { enabled } } }"#)
        .await;
    assert_eq!(e["admin"]["evaluateFeatureFlag"]["enabled"], false);

    // 不正値は422、未知フラグは404。
    assert_eq!(
        g.status(
            r#"mutation { admin { upsertFeatureFlag(name: "bad", rolloutPercent: 101) { name } } }"#
        )
        .await,
        422
    );
    assert_eq!(
        g.status(r#"{ admin { featureFlag(name: "none") { name } } }"#)
            .await,
        404
    );

    // 削除(2回目は404)。
    let d = g
        .ok(r#"mutation { admin { deleteFeatureFlag(name: "canary") } }"#)
        .await;
    assert_eq!(d["admin"]["deleteFeatureFlag"], true);
    assert_eq!(
        g.status(r#"mutation { admin { deleteFeatureFlag(name: "canary") } }"#)
            .await,
        404
    );
}

#[tokio::test]
async fn persisted_queries_register_and_fetch() {
    let g = env().await;
    let p = g
        .ok(r#"mutation { admin { registerPersistedQuery(query: "{ health }") { hash query registeredAt } } }"#)
        .await;
    let p = &p["admin"]["registerPersistedQuery"];
    let hash = p["hash"].as_str().unwrap();
    assert_eq!(hash.len(), 64);
    assert!(hash.chars().all(|c| c.is_ascii_hexdigit()));

    let got = g
        .ok(&format!(
            "{{ admin {{ persistedQuery(hash: \"{hash}\") {{ hash query }} }} }}"
        ))
        .await;
    assert_eq!(got["admin"]["persistedQuery"]["query"], "{ health }");
    assert_eq!(
        g.status(r#"{ admin { persistedQuery(hash: "deadbeef") { hash } } }"#)
            .await,
        404
    );
}

#[tokio::test]
async fn cache_purge() {
    let g = env().await;
    let p = g
        .ok(r#"mutation { admin { purgeCache(path: "/a") { purged } } }"#)
        .await;
    assert_eq!(p["admin"]["purgeCache"]["purged"], "/a");
    let a = g
        .ok("mutation { admin { purgeAllCache { purged } } }")
        .await;
    assert_eq!(a["admin"]["purgeAllCache"]["purged"], "*");
}

#[tokio::test]
async fn admin_requires_valid_key() {
    let e = env().await;
    // 鍵を1つ発行するとレジストリが空でなくなり、無効な鍵は拒否される。
    let guardian = KeyGuardian::new(Arc::clone(&e.state.db), GuardianConfig::from_env());
    let key = guardian.issue("tester", vec![], None).await.unwrap();

    for q in [
        "{ admin { featureFlags { name } } }",
        "mutation { admin { purgeAllCache { purged } } }",
    ] {
        // 鍵なし/不正な鍵はどちらも401(extensions.status)。
        assert_eq!(e.gql(q, Some("")).await.unwrap_err().0, 401);
        assert_eq!(e.gql(q, Some("bogus")).await.unwrap_err().0, 401);
    }

    // 正しい鍵なら通る。公開フィールド(health)は従来どおり鍵なしで動く。
    let ok = e
        .gql("{ admin { featureFlags { name } } }", Some(&key))
        .await
        .unwrap();
    assert!(ok["admin"]["featureFlags"].is_array());
    assert_eq!(e.gql("{ health }", Some("")).await.unwrap()["health"], "ok");
}

#[tokio::test]
async fn db_operations() {
    let g = env().await;

    let s = g.ok("{ admin { dbStatus } }").await;
    assert_eq!(
        s["admin"]["dbStatus"],
        json!({ "backend": "in-memory", "status": "ok" })
    );

    let r = g.ok("{ admin { dbRouting } }").await;
    assert_eq!(r["admin"]["dbRouting"]["default_target"], "postgresql");
    assert_eq!(
        r["admin"]["dbRouting"]["entries"].as_array().unwrap().len(),
        9
    );

    let p = g
        .ok(r#"mutation { admin { dbPut(table: "notes", key: "k1", value: {n: 1, tags: ["a","b"]}) } }"#)
        .await;
    assert_eq!(
        p["admin"]["dbPut"],
        json!({ "table": "notes", "key": "k1", "value": { "n": 1, "tags": ["a", "b"] } })
    );

    let got = g
        .ok(r#"{ admin { dbGet(table: "notes", key: "k1") } }"#)
        .await;
    assert_eq!(
        got["admin"]["dbGet"]["value"],
        json!({ "n": 1, "tags": ["a", "b"] })
    );

    let l = g.ok(r#"{ admin { dbList(table: "notes") } }"#).await;
    assert_eq!(l["admin"]["dbList"]["count"], 1);
    assert_eq!(l["admin"]["dbList"]["table"], "notes");

    // 履歴読み出しはインメモリ実装では未対応(501)。
    assert_eq!(
        g.status(r#"{ admin { dbGetAtCommit(table: "notes", key: "k1", commitId: "abc") } }"#)
            .await,
        501
    );

    let d = g
        .ok(r#"mutation { admin { dbDelete(table: "notes", key: "k1") } }"#)
        .await;
    assert_eq!(
        d["admin"]["dbDelete"],
        json!({ "table": "notes", "key": "k1", "deleted": true })
    );
    assert_eq!(
        g.status(r#"{ admin { dbGet(table: "notes", key: "k1") } }"#)
            .await,
        404
    );
}

#[tokio::test]
async fn analytics_and_ai_route() {
    let g = env().await;

    let a = g.ok("{ admin { analyticsRequestsPerMonth } }").await;
    assert!(a["admin"]["analyticsRequestsPerMonth"]["months"].is_array());
    let o = g.ok("{ admin { analyticsOperations } }").await;
    assert!(o["admin"]["analyticsOperations"]["operations"].is_array());

    let s = g.ok("{ admin { cacheAiStats } }").await;
    assert!(s["admin"]["cacheAiStats"]["ai_enabled"].is_boolean());

    // コスト優先: 無料のローカルが選ばれる。
    let r = g
        .ok(r#"{ admin { aiRoute(policy: "cost", minContextLength: 8000, candidates: [
            { provider: "openai", estimatedCostUsdPer1KTokens: 0.5, estimatedLatencyMs: 300, isLocal: false, contextLength: 128000 },
            { provider: "local", estimatedCostUsdPer1KTokens: 0.0, estimatedLatencyMs: 900, isLocal: true, contextLength: 32000 }
        ]) } }"#)
        .await;
    assert_eq!(r["admin"]["aiRoute"]["selected_provider"], "local_llm");
    assert_eq!(r["admin"]["aiRoute"]["is_local"], true);

    // 条件を満たす候補が無ければ422。
    assert_eq!(
        g.status(
            r#"{ admin { aiRoute(policy: "cost", minContextLength: 999999, candidates: [
            { provider: "openai", estimatedCostUsdPer1KTokens: 0.5, estimatedLatencyMs: 300, isLocal: false, contextLength: 1000 }
        ]) } }"#
        )
        .await,
        422
    );
}

#[tokio::test]
async fn backup_migrate_integrity() {
    let g = env().await;
    g.state.db.put("notes", "k", "{\"v\":1}").await.unwrap();

    let i = g.ok("mutation { admin { integrityCheck } }").await;
    assert_eq!(i["admin"]["integrityCheck"]["healed"], 0);
    assert_eq!(i["admin"]["integrityCheck"]["backend"], "in-memory");

    let b = g.ok("mutation { admin { backupExport } }").await;
    let b = &b["admin"]["backupExport"];
    assert!(b["records"].as_u64().unwrap() >= 1);
    let path = b["written"][0].as_str().unwrap().to_string();

    // 書き出したファイルを読み戻せる。
    let imp = g
        .ok(&format!(
            "mutation {{ admin {{ backupImport(path: {}) }} }}",
            serde_json::to_string(&path).unwrap()
        ))
        .await;
    assert!(imp["admin"]["backupImport"]["restored"].as_u64().unwrap() >= 1);
    // 存在しないパスは400。
    assert_eq!(
        g.status(r#"mutation { admin { backupImport(path: "/nonexistent/backup.json") } }"#)
            .await,
        400
    );

    // 直近のバックアップから復元できる(上で書き出したものがある)。
    let r = g.ok("mutation { admin { backupRestoreLatest } }").await;
    assert!(r["admin"]["backupRestoreLatest"]["restored_from"].is_string());

    let sql = g
        .ok(r#"mutation { admin { migrateExportSql(dialect: "postgres") } }"#)
        .await;
    assert!(!sql["admin"]["migrateExportSql"]["written"]
        .as_array()
        .unwrap()
        .is_empty());
    assert_eq!(
        g.status(r#"mutation { admin { migrateExportSql(dialect: "oracle") } }"#)
            .await,
        422
    );
    let csv = g.ok("mutation { admin { migrateExportCsv } }").await;
    assert!(!csv["admin"]["migrateExportCsv"]["written"]
        .as_array()
        .unwrap()
        .is_empty());
}

#[tokio::test]
async fn scim_users_via_graphql() {
    let g = env().await;
    let c = g
        .ok(r#"mutation { admin { scimCreateUser(userName: "alice@example.com", roles: ["developer"]) } }"#)
        .await;
    let c = &c["admin"]["scimCreateUser"];
    assert_eq!(c["userName"], "alice@example.com");
    // 平文の鍵は作成時の応答にだけ載る。以後は鍵レジストリが空でなくなるため、この鍵で呼ぶ。
    let key = c["urn:open-runo:params:scim:api-key"]
        .as_str()
        .filter(|k| k.starts_with("orn_"))
        .expect("auto-issued key")
        .to_string();
    let id = c["id"].as_str().unwrap().to_string();
    let k = Some(key.as_str());

    let l = g.gql("{ admin { scimUsers } }", k).await.unwrap();
    assert_eq!(l["admin"]["scimUsers"]["totalResults"], 1);

    // 重複は409。
    assert_eq!(
        g.gql(
            r#"mutation { admin { scimCreateUser(userName: "alice@example.com") } }"#,
            k
        )
        .await
        .unwrap_err()
        .0,
        409
    );

    let d = g
        .gql(
            &format!("mutation {{ admin {{ scimDeleteUser(id: \"{id}\") }} }}"),
            k,
        )
        .await
        .unwrap();
    assert_eq!(d["admin"]["scimDeleteUser"], true);
    // 削除したユーザーの鍵は自動失効する(REST時代の`key_guardian_full_auto_lifecycle`と同じ)。
    assert_eq!(
        g.gql("{ admin { scimUsers } }", k).await.unwrap_err().0,
        401
    );
}

#[tokio::test]
async fn self_issue_and_session_flow() {
    let e = env().await;

    // 鍵の自己発行は認証不要。
    let gk = e.gql("mutation { selfIssueKey }", Some("")).await.unwrap();
    let gk = &gk["selfIssueKey"];
    assert!(gk["api_key"].as_str().is_some_and(|k| !k.is_empty()));
    assert!(gk["expires_at"].is_string());
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

    let call = |cookie: &str, csrf: Option<&str>, query: &str| {
        let mut req = e
            .client
            .post(format!("{}/graphql", e.base))
            .header("cookie", cookie.to_string())
            .json(&json!({ "query": query }));
        if let Some(c) = csrf {
            req = req.header("x-csrf-token", c.to_string());
        }
        async move { req.send().await.unwrap() }
    };

    // Cookie+CSRFのみで管理Queryが通る。
    let body: Value = call(&cookie, Some(&csrf), "{ admin { featureFlags { name } } }")
        .await
        .json()
        .await
        .unwrap();
    assert!(body["errors"].is_null(), "{body}");
    // CSRFヘッダ無しは拒否(POSTはCSRFトークン必須)。
    let body: Value = call(&cookie, None, "{ admin { featureFlags { name } } }")
        .await
        .json()
        .await
        .unwrap();
    assert!(!body["errors"].is_null());

    // ログアウトでCookieが消え、そのセッションは使えなくなる。
    let resp = call(&cookie, Some(&csrf), "mutation { sessionLogout }").await;
    assert!(resp.headers().get("set-cookie").is_some());
    let body: Value = call(&cookie, Some(&csrf), "{ admin { featureFlags { name } } }")
        .await
        .json()
        .await
        .unwrap();
    assert!(!body["errors"].is_null());

    // 無効な鍵でのログインは401(鍵発行でレジストリが空でなくなっている)。
    assert_eq!(
        e.gql("mutation { sessionLogin }", Some("bogus"))
            .await
            .unwrap_err()
            .0,
        401
    );
}
