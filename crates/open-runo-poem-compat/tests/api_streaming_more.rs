//! 逐次アップロード・SSEのkeep-alive/イベント名・WebSocketのバイナリの通し試験。

use futures::{SinkExt, StreamExt};
use open_runo_poem_compat::api::{self, FileField};
use open_runo_poem_compat::openapi::{Operation as Op, Spec};
use open_runo_poem_compat::testing::TestServer;
use open_runo_poem_compat::{Method, Route};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Serialize, JsonSchema)]
struct Tick {
    n: u32,
    label: String,
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

// ───────────── 逐次アップロード ─────────────

#[derive(Serialize, JsonSchema)]
struct UploadSummary {
    title: String,
    bytes: usize,
    chunks: usize,
    largest_chunk: usize,
}

#[derive(Deserialize, JsonSchema)]
#[allow(dead_code)] // 文書(スキーマ)宣言用で、値はハンドラ側で UploadStream から読む
struct TitleForm {
    title: String,
}

fn upload_stream_app() -> Route {
    Route::new().api(api::upload::<TitleForm, _, _, _>(
        Op::post("/big").summary("大きなファイル"),
        &[FileField::required("file", "本体")],
        |mut up, _ctx| async move {
            let (mut title, mut bytes, mut chunks, mut largest) =
                (String::new(), 0usize, 0usize, 0usize);
            while let Some(field) = up.next_field().await? {
                if field.filename.is_some() {
                    while let Some(c) = up.chunk().await? {
                        bytes += c.len();
                        chunks += 1;
                        largest = largest.max(c.len());
                    }
                } else if field.name == "title" {
                    title = up.text(1024).await?;
                }
            }
            Ok(UploadSummary {
                title,
                bytes,
                chunks,
                largest_chunk: largest,
            })
        },
    ))
}

#[tokio::test]
async fn upload_reads_a_big_file_in_chunks_not_all_at_once() {
    let srv = TestServer::start(upload_stream_app()).await;
    let big = vec![9u8; 8 * 1024 * 1024];
    let body = multipart_body(
        "XB",
        &[("title", None, b"movie"), ("file", Some("m.bin"), &big)],
    );
    let res = srv
        .request(
            Method::POST,
            "/big",
            &[("content-type", "multipart/form-data; boundary=XB")],
            Some(body),
        )
        .await;
    assert_eq!(res.status, 200, "{}", res.text());
    let v: serde_json::Value = res.json();
    assert_eq!(v["title"], "movie");
    assert_eq!(v["bytes"], big.len());
    assert!(v["chunks"].as_u64().unwrap() > 1, "{v}");
    assert!(
        v["largest_chunk"].as_u64().unwrap() < (big.len() / 2) as u64,
        "ファイル全体を1つに集めている: {v}"
    );
}

#[tokio::test]
async fn upload_enforces_limits_and_content_type_and_is_documented() {
    let srv = TestServer::start(upload_stream_app().max_body(64 * 1024)).await;
    let big = vec![1u8; 512 * 1024];
    let body = multipart_body("XB", &[("file", Some("m.bin"), &big)]);
    if let Ok(res) = srv
        .try_request(
            Method::POST,
            "/big",
            &[("content-type", "multipart/form-data; boundary=XB")],
            Some(body),
        )
        .await
    {
        assert_eq!(res.status, 413, "{}", res.text());
    }
    assert_eq!(
        srv.request(
            Method::POST,
            "/big",
            &[("content-type", "text/plain")],
            Some(b"x".to_vec())
        )
        .await
        .status,
        400
    );

    let doc_srv = TestServer::start(upload_stream_app().docs("/api", Spec::new("t", "1"))).await;
    let doc: serde_json::Value = doc_srv.get("/api/openapi.json").await.json();
    assert!(
        doc["paths"]["/big"]["post"]["requestBody"]["content"]["multipart/form-data"].is_object()
    );
}

// ───────────── SSE: keep-alive・イベント名 ─────────────

#[tokio::test]
async fn sse_sends_keep_alive_comments_while_idle() {
    let app = Route::new().api(api::sse_opts(
        Op::get("/slow").summary("遅い"),
        Some(std::time::Duration::from_millis(40)),
        |_ctx| async move {
            Ok(Box::pin(futures::stream::unfold(0u8, |i| async move {
                if i == 2 {
                    return None;
                }
                tokio::time::sleep(std::time::Duration::from_millis(250)).await;
                Some((
                    Tick {
                        n: i as u32,
                        label: "x".into(),
                    },
                    i + 1,
                ))
            })))
        },
    ));
    let srv = TestServer::start(app).await;
    let text = srv.get("/slow").await.text();
    assert!(text.matches(": keep-alive").count() >= 3, "{text}");
    assert_eq!(text.matches("data: ").count(), 2, "{text}");

    // keep-alive 無しも選べる
    let app = Route::new().api(api::sse_opts(
        Op::get("/q").summary("q"),
        None,
        |_ctx| async move {
            Ok(futures::stream::iter(vec![Tick {
                n: 1,
                label: "a".into(),
            }]))
        },
    ));
    let srv = TestServer::start(app).await;
    assert!(!srv.get("/q").await.text().contains("keep-alive"));
}

#[tokio::test]
async fn sse_named_events_carry_event_names_and_are_documented() {
    let app = Route::new()
        .api(api::sse_named(
            Op::get("/feed").summary("フィード"),
            &["tick", "done"],
            None,
            |_ctx| async move {
                Ok(futures::stream::iter(vec![
                    api::Named::new(
                        "tick",
                        Tick {
                            n: 1,
                            label: "a".into(),
                        },
                    ),
                    api::Named::new(
                        "secret",
                        Tick {
                            n: 2,
                            label: "hidden".into(),
                        },
                    ), // 宣言に無い名前は送らない
                    api::Named::new(
                        "done",
                        Tick {
                            n: 3,
                            label: "z".into(),
                        },
                    ),
                ]))
            },
        ))
        .docs("/api", Spec::new("t", "1"));
    let srv = TestServer::start(app).await;
    let text = srv.get("/feed").await.text();
    assert!(text.contains("event: tick\ndata: {\"n\":1"), "{text}");
    assert!(text.contains("event: done\n"), "{text}");
    assert!(
        !text.contains("secret") && !text.contains("hidden"),
        "{text}"
    );
    let doc: serde_json::Value = srv.get("/api/openapi.json").await.json();
    assert_eq!(
        doc["paths"]["/feed"]["get"]["x-sse-events"],
        serde_json::json!(["tick", "done"])
    );
    let ts = srv.get("/api/client.ts").await.text();
    assert!(ts.contains(r#"es.addEventListener("tick""#), "{ts}");
    assert!(ts.contains(r#"name: "tick" | "done""#), "{ts}");
}

// ───────────── WebSocket: バイナリ ─────────────

#[derive(Deserialize, JsonSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
enum In {
    Say { text: String },
    Ping,
}

#[derive(Serialize, JsonSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
enum Out {
    Said { text: String },
    Pong,
    Error { message: String },
}

fn echo_app() -> Route {
    Route::new()
        .api(
            api::websocket(
                Op::get("/echo").summary("エコー"),
                |mut ws: api::WsSession<In, Out>, _ctx| async move {
                    while let Some(m) = ws.recv_any().await {
                        let ok = match m {
                            api::WsIncoming::Message(In::Ping) => ws.send(&Out::Pong).await,
                            api::WsIncoming::Message(In::Say { text }) => {
                                ws.send(&Out::Said { text }).await
                            },
                            api::WsIncoming::Binary(mut b) => {
                                b.reverse();
                                ws.send_binary(&b).await
                            },
                            api::WsIncoming::Invalid(e) => {
                                ws.send(&Out::Error {
                                    message: e.to_string(),
                                })
                                .await
                            },
                        };
                        if ok.is_err() {
                            break;
                        }
                    }
                },
            )
            .binary(Some("音声チャンク(PCM)"), Some("反転した音声チャンク")),
        )
        .docs("/api", Spec::new("t", "1"))
}

#[tokio::test]
async fn websocket_binary_messages_round_trip_and_are_documented() {
    use tokio_tungstenite::tungstenite::Message;
    let srv = TestServer::start(echo_app()).await;
    let (mut ws, _) = tokio_tungstenite::connect_async(format!("ws://{}/echo", srv.addr))
        .await
        .unwrap();
    ws.send(Message::Binary(vec![1u8, 2, 3, 250].into()))
        .await
        .unwrap();
    let Message::Binary(b) = ws.next().await.unwrap().unwrap() else {
        panic!("expected binary")
    };
    assert_eq!(b, vec![250, 3, 2, 1]);
    // テキストとバイナリが混在できる
    ws.send(Message::Text(r#"{"type":"ping"}"#.into()))
        .await
        .unwrap();
    let Message::Text(t) = ws.next().await.unwrap().unwrap() else {
        panic!("expected text")
    };
    assert_eq!(t.to_string(), r#"{"type":"pong"}"#);

    let doc: serde_json::Value = srv.get("/api/openapi.json").await.json();
    assert_eq!(
        doc["paths"]["/echo"]["get"]["x-websocket-binary"]["client"],
        "音声チャンク(PCM)"
    );
    let ts = srv.get("/api/client.ts").await.text();
    assert!(
        ts.contains("sendBinary: (data: ArrayBuffer | Uint8Array) => void"),
        "{ts}"
    );
    assert!(
        ts.contains("onBinary?: (data: ArrayBuffer) => void"),
        "{ts}"
    );
}

/// 生成TSを `RPOEM_DUMP_TS2=<path>` へ書き出す(`tsc --strict` の型検査用)。
#[tokio::test]
#[ignore]
async fn dump_typescript() {
    let Ok(path) = std::env::var("RPOEM_DUMP_TS2") else {
        return;
    };
    let app = echo_app()
        .api(api::sse_named(
            Op::get("/feed").summary("f"),
            &["a", "b"],
            None,
            |_ctx| async move { Ok(futures::stream::iter(Vec::<api::Named<Tick>>::new())) },
        ))
        .api(api::upload::<TitleForm, UploadSummary, _, _>(
            Op::post("/big").summary("b"),
            &[FileField::required("file", "x")],
            |_u, _c| async move {
                Ok(UploadSummary {
                    title: String::new(),
                    bytes: 0,
                    chunks: 0,
                    largest_chunk: 0,
                })
            },
        ));
    let srv = TestServer::start(app).await;
    std::fs::write(path, srv.get("/api/client.ts").await.text()).unwrap();
}
