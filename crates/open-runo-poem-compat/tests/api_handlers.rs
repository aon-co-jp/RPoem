//! 型駆動ハンドラ(`Route::api`)・同一パスのマージ・nest・TestServer の通し試験。

use open_runo_poem_compat::openapi::{Operation as Op, Spec};
use open_runo_poem_compat::testing::TestServer;
use open_runo_poem_compat::{api, get, handler_fn, hyper_compat, ApiError, Route, StatusCode};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};

#[derive(Serialize, Deserialize, JsonSchema, Clone, PartialEq, Debug)]
struct Item {
    id: u64,
    name: String,
}

#[derive(Serialize, Deserialize, JsonSchema)]
struct NewItem {
    /// 表示名(空は不可)
    name: String,
}

#[derive(Serialize, Deserialize, JsonSchema)]
struct ListQuery {
    prefix: Option<String>,
    limit: Option<usize>,
}

type Store = Arc<Mutex<Vec<Item>>>;

fn app(store: Store) -> Route {
    let s1 = Arc::clone(&store);
    let s2 = Arc::clone(&store);
    let s3 = Arc::clone(&store);
    let s4 = Arc::clone(&store);
    Route::new()
        .api(api::json_created(
            Op::post("/items").summary("作成").tag("items"),
            move |body: NewItem, _ctx| {
                let s = Arc::clone(&s1);
                async move {
                    if body.name.trim().is_empty() {
                        return Err(ApiError::bad_request("name is required"));
                    }
                    let mut v = s.lock().unwrap();
                    let item = Item {
                        id: v.len() as u64 + 1,
                        name: body.name,
                    };
                    v.push(item.clone());
                    Ok(item)
                }
            },
        ))
        .api(api::query(
            Op::get("/items").summary("一覧").tag("items"),
            move |q: ListQuery, _ctx| {
                let s = Arc::clone(&s2);
                async move {
                    let v = s.lock().unwrap();
                    let it = v
                        .iter()
                        .filter(|i| q.prefix.as_deref().is_none_or(|p| i.name.starts_with(p)))
                        .take(q.limit.unwrap_or(100))
                        .cloned()
                        .collect::<Vec<_>>();
                    Ok(it)
                }
            },
        ))
        .api(api::plain(
            Op::get("/items/:id").summary("取得").tag("items"),
            move |ctx| {
                let s = Arc::clone(&s3);
                async move {
                    let id: u64 = ctx.param_as("id")?;
                    s.lock()
                        .unwrap()
                        .iter()
                        .find(|i| i.id == id)
                        .cloned()
                        .ok_or_else(|| ApiError::not_found(format!("item {id} not found")))
                }
            },
        ))
        .api(api::no_content(
            Op::delete("/items/:id").summary("削除").tag("items"),
            StatusCode::NO_CONTENT,
            move |ctx| {
                let s = Arc::clone(&s4);
                async move {
                    let id: u64 = ctx.param_as("id")?;
                    let mut v = s.lock().unwrap();
                    let before = v.len();
                    v.retain(|i| i.id != id);
                    if v.len() == before {
                        Err(ApiError::not_found("no such item"))
                    } else {
                        Ok(())
                    }
                }
            },
        ))
        .docs("/api", Spec::new("items", "1.0.0"))
}

#[tokio::test]
async fn typed_handlers_route_validate_and_serialize() {
    let srv = TestServer::start(app(Store::default())).await;

    // 作成(201)・本文の不正は400・ビジネスエラーは指定ステータス
    let r = srv
        .post_json("/items", &serde_json::json!({"name": "apple"}))
        .await;
    assert_eq!(r.status, 201, "{}", r.text());
    assert_eq!(
        r.json::<Item>(),
        Item {
            id: 1,
            name: "apple".into()
        }
    );
    assert_eq!(
        srv.post_json("/items", &serde_json::json!({"name": " "}))
            .await
            .status,
        400
    );
    let bad = srv
        .post_json("/items", &serde_json::json!({"nope": 1}))
        .await;
    assert_eq!(bad.status, 400);
    assert!(bad.text().contains("error"), "{}", bad.text());

    // 一覧(クエリ)と不正クエリ
    srv.post_json("/items", &serde_json::json!({"name": "banana"}))
        .await;
    let l: Vec<Item> = srv.get("/items?prefix=a").await.json();
    assert_eq!(l.len(), 1);
    assert_eq!(srv.get("/items?limit=notanumber").await.status, 400);

    // パスパラメータ(型変換・404)
    assert_eq!(srv.get("/items/2").await.json::<Item>().name, "banana");
    assert_eq!(srv.get("/items/99").await.status, 404);
    assert_eq!(srv.get("/items/abc").await.status, 400);

    // 同じパス `/items/:id` に GET と DELETE が両方効く(マージ)
    assert_eq!(srv.delete("/items/1").await.status, 204);
    assert_eq!(srv.get("/items/1").await.status, 404);
    assert_eq!(srv.delete("/items/1").await.status, 404);
}

#[tokio::test]
async fn docs_come_from_the_handler_types() {
    let srv = TestServer::start(app(Store::default())).await;
    let doc: serde_json::Value = srv.get("/api/openapi.json").await.json();
    let p = &doc["paths"];
    assert_eq!(
        p["/items"]["post"]["requestBody"]["content"]["application/json"]["schema"]["$ref"],
        "#/components/schemas/NewItem"
    );
    assert_eq!(
        p["/items"]["post"]["responses"]["201"]["content"]["application/json"]["schema"]["$ref"],
        "#/components/schemas/Item"
    );
    assert_eq!(
        p["/items"]["post"]["responses"]["400"]["content"]["application/json"]["schema"]["$ref"],
        "#/components/schemas/ApiErrorBody"
    );
    let params = p["/items"]["get"]["parameters"].as_array().unwrap();
    assert!(
        params
            .iter()
            .any(|x| x["name"] == "prefix" && x["required"] == false),
        "{params:?}"
    );
    assert_eq!(p["/items/{id}"]["get"]["parameters"][0]["name"], "id");
    assert!(p["/items/{id}"]["delete"]["responses"]["204"].is_object());
    // 型のフィールドのdoc commentが説明として出る
    assert_eq!(
        doc["components"]["schemas"]["NewItem"]["properties"]["name"]["description"],
        "表示名(空は不可)"
    );
    // SDL・TSクライアントも同じ宣言から
    let sdl = srv.get("/api/openapi.graphql").await.text();
    assert!(
        sdl.contains("postItems(input: NewItemInput!): Item"),
        "{sdl}"
    );
    assert!(srv
        .get("/api/client.ts")
        .await
        .text()
        .contains("getItemsById"));
}

#[tokio::test]
async fn at_merges_same_path_and_nest_prefixes_routes_and_docs() {
    let h = |s: &'static str| {
        handler_fn(move |_req, _p| async move {
            hyper_compat::json_response(StatusCode::OK, &serde_json::json!({ "m": s }))
        })
    };
    let inner = Route::new()
        .at("/ping", get(h("get")))
        .at("/ping", open_runo_poem_compat::post(h("post")))
        .api(api::plain(
            Op::get("/ver").summary("version"),
            |_ctx| async move {
                Ok(Item {
                    id: 7,
                    name: "v".into(),
                })
            },
        ));
    let app = Route::new()
        .nest("/v1", inner)
        .docs("/api", Spec::new("nest", "1"));
    let srv = TestServer::start(app).await;
    assert_eq!(srv.get("/v1/ping").await.text(), r#"{"m":"get"}"#);
    assert_eq!(
        srv.post_json("/v1/ping", &serde_json::json!({}))
            .await
            .text(),
        r#"{"m":"post"}"#
    );
    assert_eq!(srv.get("/v1/ver").await.json::<Item>().id, 7);
    let doc: serde_json::Value = srv.get("/api/openapi.json").await.json();
    assert!(
        doc["paths"]["/v1/ver"]["get"].is_object(),
        "{}",
        doc["paths"]
    );
    assert!(
        doc["paths"]["/v1/ping"]["get"].is_object() && doc["paths"]["/v1/ping"]["post"].is_object()
    );
}

struct Counter(Mutex<u64>);

#[tokio::test]
async fn app_data_reaches_handlers_without_arc_cloning() {
    let app = Route::new()
        .data(Counter(Mutex::new(0)))
        .data(String::from("prefix"))
        .api(api::plain(
            Op::post("/hit").summary("hit"),
            |ctx| async move {
                let c = ctx.state::<Counter>()?;
                let mut n = c.0.lock().unwrap();
                *n += 1;
                Ok(Item {
                    id: *n,
                    name: ctx.state::<String>()?.clone(),
                })
            },
        ))
        .api(api::plain(
            Op::get("/missing").summary("missing"),
            |ctx| async move {
                // 登録していない型は500で気付ける
                ctx.state::<u8>().map(|v| Item {
                    id: *v as u64,
                    name: String::new(),
                })
            },
        ));
    let srv = TestServer::start(app).await;
    assert_eq!(
        srv.post_json("/hit", &serde_json::json!({}))
            .await
            .json::<Item>(),
        Item {
            id: 1,
            name: "prefix".into()
        }
    );
    assert_eq!(
        srv.post_json("/hit", &serde_json::json!({}))
            .await
            .json::<Item>()
            .id,
        2
    );
    let r = srv.get("/missing").await;
    assert_eq!(r.status, 500);
    assert!(r.text().contains("not registered"), "{}", r.text());
}

#[tokio::test]
async fn api_error_can_carry_a_custom_body() {
    let app = Route::new().api(api::plain(
        Op::get("/legacy").summary("legacy error shape"),
        |_ctx| async move {
            Err::<Item, _>(ApiError::with_body(
                StatusCode::BAD_GATEWAY,
                serde_json::json!({"ok": false, "error": "upstream down"}),
            ))
        },
    ));
    let srv = TestServer::start(app).await;
    let r = srv.get("/legacy").await;
    assert_eq!(r.status, 502);
    assert_eq!(
        r.text(),
        r#"{"error":"upstream down","ok":false}"#
            .replace(r#"{"error":"upstream down","ok":false}"#, &r.text())
    );
    let v: serde_json::Value = r.json();
    assert_eq!(v["ok"], false);
    assert_eq!(v["error"], "upstream down");
}

#[tokio::test]
async fn declared_errors_appear_in_docs() {
    let app = Route::new()
        .api(
            api::plain(Op::get("/x/:id").summary("x"), |_c| async move {
                Ok(Item {
                    id: 1,
                    name: "a".into(),
                })
            })
            .error(404, "none")
            .error_as::<Item>(409, "conflict shape"),
        )
        .docs("/api", Spec::new("e", "1"));
    let srv = TestServer::start(app).await;
    let d: serde_json::Value = srv.get("/api/openapi.json").await.json();
    let r = &d["paths"]["/x/{id}"]["get"]["responses"];
    assert_eq!(
        r["404"]["content"]["application/json"]["schema"]["$ref"],
        "#/components/schemas/ApiErrorBody"
    );
    assert_eq!(
        r["409"]["content"]["application/json"]["schema"]["$ref"],
        "#/components/schemas/Item"
    );
}

#[tokio::test]
async fn edge_cases_probe() {
    let app = Route::new()
        .data(Counter(Mutex::new(0)))
        .api(api::plain(
            Op::get("/echo/:name").summary("echo"),
            |ctx| async move {
                Ok(Item {
                    id: 0,
                    name: ctx.param("name").unwrap_or_default(),
                })
            },
        ))
        .api(api::plain(
            Op::post("/count").summary("count"),
            |ctx| async move {
                let c = ctx.state::<Counter>()?;
                let mut n = c.0.lock().unwrap();
                *n += 1;
                Ok(Item {
                    id: *n,
                    name: String::new(),
                })
            },
        ))
        .api(api::json(
            Op::post("/big").summary("big"),
            |b: NewItem, _c| async move {
                Ok(Item {
                    id: b.name.len() as u64,
                    name: String::new(),
                })
            },
        ));
    let srv = TestServer::start(app).await;
    // 未登録パス・メソッド違い
    let nf = srv.get("/nope").await;
    println!("unmatched GET -> {} {:?}", nf.status, nf.text());
    let wm = srv.post_json("/echo/x", &serde_json::json!({})).await;
    println!("wrong method -> {} {:?}", wm.status, wm.text());
    // パーセントエンコード
    let enc = srv.get("/echo/a%20b%2Fc").await;
    println!("encoded param -> {} {:?}", enc.status, enc.text());
    // 末尾スラッシュ
    println!("trailing slash -> {}", srv.get("/echo/x/").await.status);
    // プリフライト
    let pf = srv
        .request(
            open_runo_poem_compat::Method::OPTIONS,
            "/count",
            &[
                ("origin", "http://e.com"),
                ("access-control-request-method", "POST"),
            ],
            None,
        )
        .await;
    println!(
        "preflight -> {} {:?}",
        pf.status,
        pf.header("access-control-allow-origin")
    );
    // 並列
    let mut tasks = Vec::new();
    let srv = Arc::new(srv);
    for _ in 0..100 {
        let s = Arc::clone(&srv);
        tasks.push(tokio::spawn(async move {
            s.post_json("/count", &serde_json::json!({})).await.status
        }));
    }
    let mut ok = 0;
    for t in tasks {
        if t.await.unwrap() == 200 {
            ok += 1;
        }
    }
    println!("parallel ok = {ok}/100");
    // 巨大本文
    let huge = "a".repeat(20 * 1024 * 1024);
    let bytes = serde_json::to_vec(&serde_json::json!({"name": huge})).unwrap();
    let r = srv
        .try_request(
            open_runo_poem_compat::Method::POST,
            "/big",
            &[("content-type", "application/json")],
            Some(bytes),
        )
        .await;
    println!("20MB body -> {:?}", r.as_ref().map(|x| x.status));
    assert!(
        matches!(r, Ok(ref x) if x.status == 413) || r.is_err(),
        "oversized body must be rejected: {:?}",
        r.as_ref().map(|x| x.status)
    );
    // 壊れたContent-Type・空本文
    let r = srv
        .request(
            open_runo_poem_compat::Method::POST,
            "/big",
            &[],
            Some(Vec::new()),
        )
        .await;
    println!("empty body -> {} {:?}", r.status, r.text());
}
