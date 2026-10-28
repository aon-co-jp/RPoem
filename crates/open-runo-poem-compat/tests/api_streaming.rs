//! クエリ配列・multipart・SSE・WebSocket・`Ctx::shared` の型駆動宣言の通し試験
//! (実サーバーを立てて実HTTP/実WebSocketで確認する)。

use futures::{SinkExt, StreamExt};
use open_runo_poem_compat::api::{self, FileField, NoFields};
use open_runo_poem_compat::openapi::{Operation as Op, Spec};
use open_runo_poem_compat::testing::TestServer;
use open_runo_poem_compat::{ApiError, Method, Route};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

// ───────────── クエリ配列 ─────────────

#[derive(Deserialize, JsonSchema)]
struct TagQuery {
    /// 絞り込むタグ(繰り返し指定できる)
    #[serde(default)]
    tag: Vec<String>,
    limit: Option<u32>,
}

#[derive(Serialize, JsonSchema)]
struct TagEcho {
    tags: Vec<String>,
    limit: Option<u32>,
}

#[tokio::test]
async fn query_accepts_repeated_keys_as_vec() {
    let app = Route::new().api(api::query(
        Op::get("/search").summary("検索"),
        |q: TagQuery, _ctx| async move {
            Ok(TagEcho {
                tags: q.tag,
                limit: q.limit,
            })
        },
    ));
    let srv = TestServer::start(app).await;

    let v: serde_json::Value = srv.get("/search?tag=a&tag=b%20c&limit=5").await.json();
    assert_eq!(v, serde_json::json!({"tags": ["a", "b c"], "limit": 5}));
    // 1つだけ・0個でも配列になる
    let v: serde_json::Value = srv.get("/search?tag=x").await.json();
    assert_eq!(v["tags"], serde_json::json!(["x"]));
    let v: serde_json::Value = srv.get("/search").await.json();
    assert_eq!(v["tags"], serde_json::json!([]));
    // 型に合わなければ400
    assert_eq!(srv.get("/search?limit=abc").await.status, 400);
}

// ───────────── Ctx::shared ─────────────

struct Greeting(String);

#[tokio::test]
async fn shared_returns_an_owned_arc_usable_in_spawned_tasks() {
    let app = Route::new().data(Greeting("hi".into())).api(api::plain(
        Op::get("/g").summary("g"),
        |ctx| async move {
            let g: Arc<Greeting> = ctx.shared()?;
            let out = tokio::spawn(async move { format!("{}!", g.0) })
                .await
                .map_err(|e| ApiError::internal(e.to_string()))?;
            Ok(out)
        },
    ));
    let srv = TestServer::start(app).await;
    assert_eq!(srv.get("/g").await.json::<String>(), "hi!");

    // 登録されていなければ500(開発時に気付ける)
    let app = Route::new().api(api::plain(Op::get("/g").summary("g"), |ctx| async move {
        let g: Arc<Greeting> = ctx.shared()?;
        Ok(g.0.clone())
    }));
    let srv = TestServer::start(app).await;
    assert_eq!(srv.get("/g").await.status, 500);
}

// ───────────── multipart ─────────────

#[derive(Deserialize, JsonSchema)]
struct UploadForm {
    /// 公開範囲
    visibility: String,
    #[serde(default)]
    labels: Vec<String>,
    size_hint: Option<u32>,
}

#[derive(Serialize, JsonSchema)]
struct UploadResult {
    visibility: String,
    labels: Vec<String>,
    size_hint: Option<u32>,
    files: Vec<UploadedInfo>,
}

#[derive(Serialize, JsonSchema)]
struct UploadedInfo {
    field: String,
    filename: Option<String>,
    bytes: usize,
}

fn multipart_body(boundary: &str, parts: &[(&str, Option<&str>, &[u8])]) -> Vec<u8> {
    let mut b = Vec::new();
    for (name, filename, data) in parts {
        b.extend_from_slice(format!("--{boundary}\r\n").as_bytes());
        match filename {
            Some(f) => b.extend_from_slice(format!("Content-Disposition: form-data; name=\"{name}\"; filename=\"{f}\"\r\nContent-Type: application/octet-stream\r\n\r\n").as_bytes()),
            None => b.extend_from_slice(format!("Content-Disposition: form-data; name=\"{name}\"\r\n\r\n").as_bytes()),
        }
        b.extend_from_slice(data);
        b.extend_from_slice(b"\r\n");
    }
    b.extend_from_slice(format!("--{boundary}--\r\n").as_bytes());
    b
}

fn upload_app() -> Route {
    Route::new().api(api::multipart(
        Op::post("/upload").summary("アップロード"),
        &[
            FileField::required("avatar", "アイコン画像"),
            FileField::optional("attachments", "添付").multiple(),
        ],
        |m: api::Multipart<UploadForm>, _ctx| async move {
            Ok(UploadResult {
                visibility: m.form.visibility.clone(),
                labels: m.form.labels.clone(),
                size_hint: m.form.size_hint,
                files: m
                    .files
                    .iter()
                    .map(|f| UploadedInfo {
                        field: f.field.clone(),
                        filename: f.filename.clone(),
                        bytes: f.data.len(),
                    })
                    .collect(),
            })
        },
    ))
}

#[tokio::test]
async fn multipart_gives_typed_fields_and_files() {
    let srv = TestServer::start(upload_app()).await;
    let ct = "multipart/form-data; boundary=XBOUNDX";
    let body = multipart_body(
        "XBOUNDX",
        &[
            ("visibility", None, b"private"),
            ("labels", None, b"a"),
            ("labels", None, b"b"),
            ("size_hint", None, b"42"),
            ("avatar", Some("me.png"), &[0u8, 1, 2, 3, 255]),
            ("attachments", Some("a.txt"), b"hello"),
            ("attachments", Some("b.txt"), b"world!"),
        ],
    );
    let res = srv
        .request(Method::POST, "/upload", &[("content-type", ct)], Some(body))
        .await;
    assert_eq!(res.status, 200, "{}", res.text());
    let v: serde_json::Value = res.json();
    assert_eq!(v["visibility"], "private");
    assert_eq!(v["labels"], serde_json::json!(["a", "b"]));
    assert_eq!(v["size_hint"], 42);
    let files = v["files"].as_array().unwrap();
    assert_eq!(files.len(), 3);
    assert_eq!(files[0]["field"], "avatar");
    assert_eq!(files[0]["bytes"], 5); // バイナリ(0や255を含む)も欠けない
    assert_eq!(files[2]["filename"], "b.txt");

    // 必須ファイルが無ければ400
    let body = multipart_body("XBOUNDX", &[("visibility", None, b"public")]);
    let res = srv
        .request(Method::POST, "/upload", &[("content-type", ct)], Some(body))
        .await;
    assert_eq!(res.status, 400);
    assert!(res.text().contains("avatar"), "{}", res.text());
    // テキストフィールドが型に合わなければ400
    let body = multipart_body(
        "XBOUNDX",
        &[
            ("visibility", None, b"x"),
            ("size_hint", None, b"abc"),
            ("avatar", Some("a"), b"1"),
        ],
    );
    assert_eq!(
        srv.request(Method::POST, "/upload", &[("content-type", ct)], Some(body))
            .await
            .status,
        400
    );
    // multipartでなければ400
    assert_eq!(
        srv.request(
            Method::POST,
            "/upload",
            &[("content-type", "application/json")],
            Some(b"{}".to_vec())
        )
        .await
        .status,
        400
    );
}

#[tokio::test]
async fn multipart_respects_max_body() {
    let app = upload_app().max_body(1024);
    let srv = TestServer::start(app).await;
    let ct = "multipart/form-data; boundary=XBOUNDX";
    let big = vec![7u8; 4096];
    let body = multipart_body(
        "XBOUNDX",
        &[
            ("visibility", None, b"x"),
            ("avatar", Some("big.bin"), &big),
        ],
    );
    // 途中で拒否して切断した場合(Err)もよい
    if let Ok(res) = srv
        .try_request(Method::POST, "/upload", &[("content-type", ct)], Some(body))
        .await
    {
        assert_eq!(res.status, 413, "{}", res.text());
    }
}

#[tokio::test]
async fn multipart_with_no_text_fields_uses_nofields() {
    let app =
        Route::new().api(api::multipart(
            Op::post("/one").summary("1ファイル"),
            &[FileField::required("f", "ファイル")],
            |m: api::Multipart<NoFields>, _ctx| async move {
                Ok(m.file("f").map_or(0, |f| f.data.len()))
            },
        ));
    let srv = TestServer::start(app).await;
    let body = multipart_body("B", &[("f", Some("x.bin"), b"12345")]);
    let res = srv
        .request(
            Method::POST,
            "/one",
            &[("content-type", "multipart/form-data; boundary=B")],
            Some(body),
        )
        .await;
    assert_eq!(res.json::<usize>(), 5);
}

#[tokio::test]
async fn multipart_docs_list_text_and_file_fields() {
    let app = upload_app().docs("/api", Spec::new("t", "1"));
    let srv = TestServer::start(app).await;
    let doc: serde_json::Value = srv.get("/api/openapi.json").await.json();
    let schema =
        &doc["paths"]["/upload"]["post"]["requestBody"]["content"]["multipart/form-data"]["schema"];
    let all = schema["allOf"].as_array().expect("allOf");
    assert_eq!(all[0]["$ref"], "#/components/schemas/UploadForm");
    assert_eq!(all[1]["properties"]["avatar"]["format"], "binary");
    assert_eq!(all[1]["properties"]["attachments"]["type"], "array");
    assert_eq!(all[1]["required"], serde_json::json!(["avatar"]));
    assert!(doc["components"]["schemas"]["UploadForm"]["properties"]["labels"].is_object());
    assert!(doc["paths"]["/upload"]["post"]["responses"]["413"].is_object());
    // TypeScriptクライアントは FormData を受ける
    let ts = srv.get("/api/client.ts").await.text();
    assert!(ts.contains("body: FormData"), "{ts}");
}

// ───────────── SSE ─────────────

#[derive(Serialize, JsonSchema)]
struct Tick {
    n: u32,
    label: String,
}

#[tokio::test]
async fn sse_streams_typed_events_and_is_documented() {
    let app = Route::new()
        .api(api::sse(
            Op::get("/ticks/:count").summary("カウント"),
            |ctx| async move {
                let count: u32 = ctx.param_as("count")?;
                if count > 100 {
                    return Err(ApiError::bad_request("count must be <= 100"));
                }
                Ok(futures::stream::iter((1..=count).map(|n| Tick {
                    n,
                    label: format!("tick-{n}"),
                })))
            },
        ))
        .docs("/api", Spec::new("t", "1"));
    let srv = TestServer::start(app).await;

    let res = srv.get("/ticks/3").await;
    assert_eq!(res.status, 200);
    assert_eq!(res.header("content-type"), Some("text/event-stream"));
    let text = res.text();
    let events: Vec<serde_json::Value> = text
        .lines()
        .filter_map(|l| l.strip_prefix("data: "))
        .map(|d| serde_json::from_str(d).unwrap())
        .collect();
    assert_eq!(events.len(), 3, "{text}");
    assert_eq!(events[2], serde_json::json!({"n": 3, "label": "tick-3"}));

    // ストリームを始める前のエラーは普通のJSONエラー
    let res = srv.get("/ticks/1000").await;
    assert_eq!(res.status, 400);
    assert_eq!(
        res.json::<serde_json::Value>()["error"],
        "count must be <= 100"
    );

    // 文書: text/event-stream のスキーマ=Tick
    let doc: serde_json::Value = srv.get("/api/openapi.json").await.json();
    let ok = &doc["paths"]["/ticks/{count}"]["get"]["responses"]["200"]["content"]
        ["text/event-stream"]["schema"];
    assert_eq!(ok["$ref"], "#/components/schemas/Tick");
    // GraphQL: Subscription、TypeScript: EventSource
    let sdl = srv.get("/api/openapi.graphql").await.text();
    assert!(sdl.contains("type Subscription"), "{sdl}");
    assert!(sdl.contains("Tick"), "{sdl}");
    let ts = srv.get("/api/client.ts").await.text();
    assert!(ts.contains("new EventSource("), "{ts}");
    assert!(ts.contains("onEvent: (e: Tick) => void"), "{ts}");
}

#[tokio::test]
async fn sse_stops_when_the_client_disconnects() {
    // 無限に流れるストリームでも、切断されたら生産側のタスクが終わる。
    let alive = Arc::new(std::sync::atomic::AtomicBool::new(true));
    struct Guard(Arc<std::sync::atomic::AtomicBool>);
    impl Drop for Guard {
        fn drop(&mut self) {
            self.0.store(false, std::sync::atomic::Ordering::SeqCst);
        }
    }
    let a2 = Arc::clone(&alive);
    let app = Route::new().api(api::sse(
        Op::get("/forever").summary("無限"),
        move |_ctx| {
            let a = Arc::clone(&a2);
            async move {
                let guard = Guard(a);
                Ok(Box::pin(futures::stream::unfold(
                    (0u32, guard),
                    |(n, g)| async move {
                        tokio::time::sleep(std::time::Duration::from_millis(5)).await;
                        Some((
                            Tick {
                                n,
                                label: String::new(),
                            },
                            (n + 1, g),
                        ))
                    },
                )))
            }
        },
    ));
    let srv = TestServer::start(app).await;
    {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let mut s = tokio::net::TcpStream::connect(srv.addr).await.unwrap();
        s.write_all(format!("GET /forever HTTP/1.1\r\nhost: {}\r\n\r\n", srv.addr).as_bytes())
            .await
            .unwrap();
        let mut buf = [0u8; 256];
        let n = s.read(&mut buf).await.unwrap();
        assert!(n > 0);
    } // ここで切断
    for _ in 0..100 {
        if !alive.load(std::sync::atomic::Ordering::SeqCst) {
            return;
        }
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    }
    panic!("the SSE producer kept running after the client disconnected");
}

// ───────────── WebSocket ─────────────

#[derive(Deserialize, JsonSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
enum ChatIn {
    Say { text: String },
    Ping,
}

#[derive(Serialize, JsonSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
enum ChatOut {
    Said { room: String, text: String },
    Pong,
    Error { message: String },
}

fn ws_app() -> Route {
    Route::new()
        .api(api::websocket(
            Op::get("/rooms/:room/ws").summary("チャット"),
            |mut ws: api::WsSession<ChatIn, ChatOut>, ctx| async move {
                let room = ctx.param("room").unwrap_or_default();
                while let Some(msg) = ws.recv().await {
                    let reply = match msg {
                        Ok(ChatIn::Say { text }) => ChatOut::Said {
                            room: room.clone(),
                            text,
                        },
                        Ok(ChatIn::Ping) => ChatOut::Pong,
                        Err(e) => ChatOut::Error {
                            message: e.to_string(),
                        },
                    };
                    if ws.send(&reply).await.is_err() {
                        break;
                    }
                }
            },
        ))
        .docs("/api", Spec::new("t", "1"))
}

#[tokio::test]
async fn websocket_exchanges_typed_messages_with_path_params() {
    use tokio_tungstenite::tungstenite::Message;
    let srv = TestServer::start(ws_app()).await;
    let (mut ws, resp) =
        tokio_tungstenite::connect_async(format!("ws://{}/rooms/lobby/ws", srv.addr))
            .await
            .expect("ws connect");
    assert_eq!(resp.status().as_u16(), 101);

    ws.send(Message::Text(
        r#"{"type":"say","text":"こんにちは"}"#.into(),
    ))
    .await
    .unwrap();
    let Message::Text(t) = ws.next().await.unwrap().unwrap() else {
        panic!("expected text")
    };
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&t).unwrap(),
        serde_json::json!({"type":"said","room":"lobby","text":"こんにちは"})
    );

    ws.send(Message::Text(r#"{"type":"ping"}"#.into()))
        .await
        .unwrap();
    let Message::Text(t) = ws.next().await.unwrap().unwrap() else {
        panic!("expected text")
    };
    assert_eq!(t.to_string(), r#"{"type":"pong"}"#);

    // 型に合わないメッセージはエラー応答で、接続は続く
    ws.send(Message::Text(r#"{"type":"nope"}"#.into()))
        .await
        .unwrap();
    let Message::Text(t) = ws.next().await.unwrap().unwrap() else {
        panic!("expected text")
    };
    assert!(t.contains(r#""type":"error""#), "{t}");
    ws.send(Message::Text(r#"{"type":"ping"}"#.into()))
        .await
        .unwrap();
    let Message::Text(t) = ws.next().await.unwrap().unwrap() else {
        panic!("expected text")
    };
    assert_eq!(t.to_string(), r#"{"type":"pong"}"#);
    ws.close(None).await.unwrap();
}

#[tokio::test]
async fn websocket_rejects_plain_http_and_is_documented() {
    let srv = TestServer::start(ws_app()).await;
    // アップグレードでない普通のGETは400
    assert_eq!(srv.get("/rooms/x/ws").await.status, 400);

    let doc: serde_json::Value = srv.get("/api/openapi.json").await.json();
    let op = &doc["paths"]["/rooms/{room}/ws"]["get"];
    assert!(op["responses"]["101"].is_object());
    assert_eq!(
        op["x-websocket"]["client"]["$ref"],
        "#/components/schemas/ChatIn"
    );
    assert_eq!(
        op["x-websocket"]["server"]["$ref"],
        "#/components/schemas/ChatOut"
    );
    assert!(
        doc["components"]["schemas"]["ChatIn"].is_object()
            && doc["components"]["schemas"]["ChatOut"].is_object()
    );

    let ts = srv.get("/api/client.ts").await.text();
    assert!(ts.contains("new WebSocket("), "{ts}");
    assert!(ts.contains("send: (m: ChatIn) => void"), "{ts}");
    assert!(ts.contains("onMessage: (m: ChatOut) => void"), "{ts}");
    let sdl = srv.get("/api/openapi.graphql").await.text();
    assert!(sdl.contains("type Subscription"), "{sdl}");
}

/// 生成したTypeScriptクライアントを `RPOEM_DUMP_TS=<path>` に書き出す(`tsc` で型検査する用)。
/// `cargo test -p open-runo-poem-compat --test api_streaming dump_typescript -- --ignored`
#[tokio::test]
#[ignore]
async fn dump_typescript() {
    let Ok(path) = std::env::var("RPOEM_DUMP_TS") else {
        return;
    };
    let app = upload_app()
        .api(api::sse(
            Op::get("/ticks/:count").summary("カウント"),
            |_ctx| async move { Ok(futures::stream::iter(Vec::<Tick>::new())) },
        ))
        .api(api::websocket(
            Op::get("/rooms/:room/ws").summary("チャット"),
            |_ws: api::WsSession<ChatIn, ChatOut>, _ctx| async move {},
        ))
        .api(api::query(
            Op::get("/search").summary("検索"),
            |q: TagQuery, _ctx| async move {
                Ok(TagEcho {
                    tags: q.tag,
                    limit: q.limit,
                })
            },
        ))
        .docs("/api", Spec::new("t", "1"));
    let srv = TestServer::start(app).await;
    std::fs::write(path, srv.get("/api/client.ts").await.text()).unwrap();
}
