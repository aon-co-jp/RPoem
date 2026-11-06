//! 5つの通信方式が同じ`Service`を同じように呼べることの確認(ネットワーク無しで`handle`を直接呼ぶ)。
use super::*;
use http_body_util::Full;
use prost::Message;
use prost_reflect::DescriptorPool;
use prost_types::field_descriptor_proto::{Label, Type};
use prost_types::{
    DescriptorProto, FieldDescriptorProto, FileDescriptorProto, FileDescriptorSet,
    MethodDescriptorProto, ServiceDescriptorProto,
};

const SVC: &str = "demo.v1.Demo";

/// `message Echo { string text = 1; }` と `service Demo { Echo / Fail / Ticks }` の記述子を手で作る。
fn pool() -> DescriptorPool {
    let m = |name: &str, stream: bool| MethodDescriptorProto {
        name: Some(name.into()),
        input_type: Some(".demo.v1.Echo".into()),
        output_type: Some(".demo.v1.Echo".into()),
        server_streaming: Some(stream),
        ..Default::default()
    };
    let file = FileDescriptorProto {
        name: Some("demo/v1/demo.proto".into()),
        package: Some("demo.v1".into()),
        syntax: Some("proto3".into()),
        message_type: vec![DescriptorProto {
            name: Some("Echo".into()),
            field: vec![FieldDescriptorProto {
                name: Some("text".into()),
                number: Some(1),
                label: Some(Label::Optional as i32),
                r#type: Some(Type::String as i32),
                json_name: Some("text".into()),
                ..Default::default()
            }],
            ..Default::default()
        }],
        service: vec![ServiceDescriptorProto {
            name: Some("Demo".into()),
            method: vec![m("Echo", false), m("Fail", false), m("Ticks", true), m("FailMid", true), m("Forever", true)],
            ..Default::default()
        }],
        ..Default::default()
    };
    DescriptorPool::from_file_descriptor_set(FileDescriptorSet { file: vec![file] }).unwrap()
}

#[derive(Clone, PartialEq, prost::Message)]
struct Echo {
    #[prost(string, tag = "1")]
    text: String,
}

struct Demo {
    pool: DescriptorPool,
}

#[async_trait]
impl Service for Demo {
    fn service_name(&self) -> &str {
        SVC
    }
    fn method(&self, name: &str) -> Option<MethodInfo> {
        let svc = self.pool.get_service_by_name(SVC)?;
        let m = svc.methods().find(|m| m.name() == name)?;
        Some(MethodInfo::with_descriptors(
            m.is_server_streaming(),
            m.input(),
            m.output(),
        ))
    }
    async fn call(
        &self,
        method: &str,
        headers: &HeaderMap,
        message: &[u8],
    ) -> Result<Output, Status> {
        if headers.get("x-api-key").and_then(|v| v.to_str().ok()) != Some("secret") {
            return Err(Status::new(code::UNAUTHENTICATED, "key required"));
        }
        let req = Echo::decode(message)
            .map_err(|e| Status::new(code::INVALID_ARGUMENT, e.to_string()))?;
        match method {
            "Echo" => {
                let mut out = Output::unary(
                    Echo {
                        text: format!("echo:{}", req.text),
                    }
                    .encode_to_vec(),
                );
                out.headers.push(("x-served-by".into(), "demo".into()));
                Ok(out)
            }
            "Fail" => Err(Status::new(code::NOT_FOUND, "見つかりません")),
            "Ticks" => Ok(Output::stream(Box::pin(futures::stream::iter(
                (1..=3).map(|i| {
                    Echo {
                        text: format!("tick{i}"),
                    }
                    .encode_to_vec()
                }),
            )))),
            "FailMid" => Ok(Output {
                reply: Reply::TryStream(Box::pin(futures::stream::iter(vec![
                    Ok(echo("a")),
                    Ok(echo("b")),
                    Err(Status::new(code::ABORTED, "途中で失敗")),
                    Ok(echo("never")),
                ]))),
                headers: Vec::new(),
            }),
            "Forever" => {
                let guard = DropFlag;
                Ok(Output::stream(Box::pin(futures::stream::unfold(
                    guard,
                    |g| async move {
                        tokio::time::sleep(std::time::Duration::from_millis(5)).await;
                        Some((echo("t"), g))
                    },
                ))))
            }
            _ => Err(Status::new(code::UNIMPLEMENTED, "no such method")),
        }
    }
}

static DROPPED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
struct DropFlag;
impl Drop for DropFlag {
    fn drop(&mut self) {
        DROPPED.store(true, std::sync::atomic::Ordering::SeqCst);
    }
}

fn svc() -> Demo {
    Demo { pool: pool() }
}

fn request(
    method: &str,
    ct: &str,
    key: Option<&str>,
    body: Vec<u8>,
) -> hyper::Request<Full<Bytes>> {
    let mut b = hyper::Request::builder()
        .method("POST")
        .uri(format!("http://x/{SVC}/{method}"))
        .header("content-type", ct);
    if let Some(k) = key {
        b = b.header("x-api-key", k);
    }
    b.body(Full::new(Bytes::from(body))).unwrap()
}

struct Out {
    status: u16,
    headers: HeaderMap,
    body: Vec<u8>,
    trailers: Option<HeaderMap>,
}

async fn run(req: hyper::Request<Full<Bytes>>) -> Out {
    let resp = handle(&svc(), req).await;
    let (parts, body) = resp.into_parts();
    let collected = body.collect().await.unwrap();
    let trailers = collected.trailers().cloned();
    Out {
        status: parts.status.as_u16(),
        headers: parts.headers,
        body: collected.to_bytes().to_vec(),
        trailers,
    }
}

fn echo(text: &str) -> Vec<u8> {
    Echo { text: text.into() }.encode_to_vec()
}

#[tokio::test]
async fn connect_unary_proto_and_json_and_error_headers() {
    // proto
    let o = run(request(
        "Echo",
        "application/proto",
        Some("secret"),
        echo("hi"),
    ))
    .await;
    assert_eq!(
        (o.status, o.headers["content-type"].to_str().unwrap()),
        (200, "application/proto")
    );
    assert_eq!(o.headers["x-served-by"], "demo");
    assert_eq!(Echo::decode(&o.body[..]).unwrap().text, "echo:hi");
    // JSON(proto3-JSON)
    let o = run(request(
        "Echo",
        "application/json",
        Some("secret"),
        br#"{"text":"hi"}"#.to_vec(),
    ))
    .await;
    assert_eq!(o.status, 200);
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&o.body).unwrap()["text"],
        "echo:hi"
    );
    // エラー: HTTPステータス+JSON、さらにgRPCのヘッダも付く(ハイブリッド)。
    let o = run(request(
        "Fail",
        "application/json",
        Some("secret"),
        b"{}".to_vec(),
    ))
    .await;
    assert_eq!(o.status, 404);
    assert_eq!(o.headers["grpc-status"], "5");
    assert_eq!(
        core::percent_decode(o.headers["grpc-message"].to_str().unwrap()),
        "見つかりません"
    );
    let v: serde_json::Value = serde_json::from_slice(&o.body).unwrap();
    assert_eq!(v["code"], "not_found");
    // 重複キー=400(RS-JSON の安全モード)。
    let o = run(request(
        "Echo",
        "application/json",
        Some("secret"),
        br#"{"text":"a","text":"b"}"#.to_vec(),
    ))
    .await;
    assert_eq!(o.status, 400);
    // 鍵なし=401。壊れたJSON=400。
    assert_eq!(
        run(request("Echo", "application/json", None, b"{}".to_vec()))
            .await
            .status,
        401
    );
    assert_eq!(
        run(request(
            "Echo",
            "application/json",
            Some("secret"),
            b"{nope".to_vec()
        ))
        .await
        .status,
        400
    );
}

#[tokio::test]
async fn connect_streaming_uses_envelopes_and_an_end_frame() {
    let body = core::encode_frame(br#"{"text":"x"}"#);
    let o = run(request(
        "Ticks",
        "application/connect+json",
        Some("secret"),
        body,
    ))
    .await;
    assert_eq!(o.status, 200);
    assert_eq!(o.headers["content-type"], "application/connect+json");
    let mut rest: &[u8] = &o.body;
    let mut msgs = Vec::new();
    let mut end = None;
    while let Some((flag, b, tail)) = core::split_frame(rest) {
        if flag == 0 {
            msgs.push(
                serde_json::from_slice::<serde_json::Value>(b).unwrap()["text"]
                    .as_str()
                    .unwrap()
                    .to_string(),
            );
        } else if flag == 0x02 {
            end = Some(core::parse_connect_end(b));
        }
        rest = tail;
    }
    assert_eq!(msgs, ["tick1", "tick2", "tick3"]);
    assert_eq!(end, Some(None), "正常終了は`{{}}`の終端フレーム");
    // ストリーミングのエラーは、HTTP 200+終端フレームの`error`で返る。
    let o = run(request(
        "Ticks",
        "application/connect+proto",
        None,
        core::encode_frame(&echo("x")),
    ))
    .await;
    assert_eq!(o.status, 200);
    let (flag, b, _) = core::split_frame(&o.body).unwrap();
    assert_eq!(flag, 0x02);
    assert_eq!(
        core::parse_connect_end(b).unwrap().code,
        code::UNAUTHENTICATED
    );
    // 単項に connect+、ストリームに application/proto は取り違え。
    assert_eq!(
        run(request(
            "Echo",
            "application/connect+proto",
            Some("secret"),
            core::encode_frame(&echo("x"))
        ))
        .await
        .status,
        400
    );
    assert_eq!(
        run(request(
            "Ticks",
            "application/proto",
            Some("secret"),
            echo("x")
        ))
        .await
        .status,
        400
    );
}

#[tokio::test]
async fn grpc_web_and_text_carry_the_status_in_a_trailer_frame() {
    let o = run(request(
        "Echo",
        "application/grpc-web+proto",
        Some("secret"),
        core::encode_frame(&echo("hi")),
    ))
    .await;
    assert_eq!(o.headers["content-type"], "application/grpc-web+proto");
    let (flag, msg, rest) = core::split_frame(&o.body).unwrap();
    assert_eq!(
        (flag, Echo::decode(msg).unwrap().text.as_str()),
        (0, "echo:hi")
    );
    let (tflag, t, _) = core::split_frame(rest).unwrap();
    assert_eq!((tflag, core::parse_web_trailers(t).0), (0x80, code::OK));

    // text: 本文もbase64、応答もbase64(データ+トレーラーの連結)。
    let body = core::b64_encode(&core::encode_frame(&echo("hi")));
    let o = run(request(
        "Echo",
        "application/grpc-web-text",
        Some("secret"),
        body,
    ))
    .await;
    assert_eq!(o.headers["content-type"], "application/grpc-web-text+proto");
    let raw = core::decode_text_body(&o.body).unwrap();
    let (_, msg, rest) = core::split_frame(&raw).unwrap();
    assert_eq!(Echo::decode(msg).unwrap().text, "echo:hi");
    assert_eq!(
        core::parse_web_trailers(core::split_frame(rest).unwrap().1).0,
        code::OK
    );

    // エラーはHTTP 200+トレーラーフレーム。壊れたbase64はINVALID_ARGUMENT。
    let o = run(request(
        "Fail",
        "application/grpc-web+proto",
        Some("secret"),
        core::encode_frame(&echo("x")),
    ))
    .await;
    assert_eq!(o.status, 200);
    let (_, t, _) = core::split_frame(&o.body).unwrap();
    assert_eq!(
        core::parse_web_trailers(t),
        (code::NOT_FOUND, "見つかりません".to_string())
    );
    let o = run(request(
        "Echo",
        "application/grpc-web-text",
        Some("secret"),
        b"@@@@".to_vec(),
    ))
    .await;
    let raw = core::decode_text_body(&o.body).unwrap();
    assert_eq!(
        core::parse_web_trailers(core::split_frame(&raw).unwrap().1).0,
        code::INVALID_ARGUMENT
    );
}

#[tokio::test]
async fn plain_grpc_puts_the_status_in_http2_trailers() {
    let o = run(request(
        "Echo",
        "application/grpc",
        Some("secret"),
        core::encode_frame(&echo("hi")),
    ))
    .await;
    let (_, msg, _) = core::split_frame(&o.body).unwrap();
    assert_eq!(Echo::decode(msg).unwrap().text, "echo:hi");
    assert_eq!(o.trailers.as_ref().unwrap()["grpc-status"], "0");
    let o = run(request(
        "Fail",
        "application/grpc",
        Some("secret"),
        core::encode_frame(&echo("x")),
    ))
    .await;
    assert_eq!(o.trailers.as_ref().unwrap()["grpc-status"], "5");
    // サーバーストリーミングもトレーラーで終わる。
    let o = run(request(
        "Ticks",
        "application/grpc",
        Some("secret"),
        core::encode_frame(&echo("x")),
    ))
    .await;
    let mut n = 0;
    let mut rest: &[u8] = &o.body;
    while let Some((_, _, tail)) = core::split_frame(rest) {
        n += 1;
        rest = tail;
    }
    assert_eq!(n, 3);
    assert_eq!(o.trailers.as_ref().unwrap()["grpc-status"], "0");
}

#[tokio::test]
async fn unsupported_requests_are_rejected_clearly() {
    // 知らないContent-Type=400(Connect形式のエラー)。
    let o = run(request("Echo", "text/plain", Some("secret"), vec![])).await;
    assert_eq!(o.status, 400);
    // 未知メソッド=501、圧縮=501。
    assert_eq!(
        run(request(
            "Nope",
            "application/json",
            Some("secret"),
            b"{}".to_vec()
        ))
        .await
        .status,
        501
    );
    let mut req = request("Echo", "application/proto", Some("secret"), echo("x"));
    req.headers_mut()
        .insert("content-encoding", "gzip".parse().unwrap());
    assert_eq!(run(req).await.status, 501);
    // 別サービスのパス=501。
    let req = hyper::Request::builder()
        .method("POST")
        .uri("http://x/other.Svc/Echo")
        .header("content-type", "application/proto")
        .body(Full::new(Bytes::new()))
        .unwrap();
    assert_eq!(run(req).await.status, 501);
}

#[test]
fn method_of_parses_only_this_service() {
    assert_eq!(method_of(SVC, "/demo.v1.Demo/Echo"), Some("Echo"));
    assert_eq!(method_of(SVC, "/demo.v1.Demo/"), None);
    assert_eq!(method_of(SVC, "/other/Echo"), None);
    assert_eq!(method_of(SVC, "/demo.v1.Demo/a/b"), None);
}

/// 本文をフレームに分け、(メッセージ数, 終端の状態)を返す。終端が無ければ`None`。
fn frames(proto: Protocol, body: &[u8]) -> (usize, Option<Status>) {
    let mut rest = body;
    let mut n = 0;
    let mut end = None;
    while let Some((flag, b, tail)) = core::split_frame(rest) {
        match (proto, flag) {
            (_, 0) => n += 1,
            (Protocol::GrpcWeb, 0x80) => {
                let (c, m) = core::parse_web_trailers(b);
                end = Some(Status::new(c, m));
            }
            (_, 0x02) => {
                end = Some(
                    core::parse_connect_end(b).unwrap_or_else(|| Status::new(code::OK, "")),
                )
            }
            _ => {}
        }
        rest = tail;
    }
    (n, end)
}

#[tokio::test]
async fn mid_stream_error_closes_with_an_end_frame_in_every_protocol() {
    // Connect: 先の2件は届き、その後`error`つき終端フレーム(HTTPは200のまま)。
    let o = run(request(
        "FailMid",
        "application/connect+json",
        Some("secret"),
        core::encode_frame(br#"{}"#),
    ))
    .await;
    assert_eq!(o.status, 200);
    let (n, end) = frames(Protocol::ConnectStream(Codec::Json), &o.body);
    assert_eq!(n, 2);
    assert_eq!(end.unwrap().code, code::ABORTED);
    // gRPC-web: `0x80`フレームのgrpc-status。
    let o = run(request(
        "FailMid",
        "application/grpc-web+proto",
        Some("secret"),
        core::encode_frame(&echo("x")),
    ))
    .await;
    let (n, end) = frames(Protocol::GrpcWeb, &o.body);
    assert_eq!(n, 2);
    assert_eq!(end.unwrap().code, code::ABORTED);
    // gRPC: HTTP/2トレーラー。
    let o = run(request(
        "FailMid",
        "application/grpc",
        Some("secret"),
        core::encode_frame(&echo("x")),
    ))
    .await;
    assert_eq!(o.trailers.unwrap()["grpc-status"], code::ABORTED.to_string());
}

async fn forever(limits: StreamLimits) -> (usize, Status) {
    let resp = handle_with(
        &svc(),
        request(
            "Forever",
            "application/connect+proto",
            Some("secret"),
            core::encode_frame(&echo("x")),
        ),
        limits,
    )
    .await;
    let body = resp.into_body().collect().await.unwrap().to_bytes();
    let (n, end) = frames(Protocol::ConnectStream(Codec::Proto), &body);
    (n, end.expect("limit must end with an end frame"))
}

#[tokio::test]
async fn stream_limits_end_the_stream_with_a_status() {
    let (n, st) = forever(StreamLimits {
        max_messages: Some(4),
        max_duration: None,
    })
    .await;
    assert_eq!((n, st.code), (4, code::RESOURCE_EXHAUSTED));
    let (n, st) = forever(StreamLimits {
        max_messages: None,
        max_duration: Some(std::time::Duration::from_millis(100)),
    })
    .await;
    assert!(n >= 1 && n < 100, "n={n}");
    assert_eq!(st.code, code::DEADLINE_EXCEEDED);
}

#[tokio::test]
async fn dropping_the_response_body_stops_the_stream() {
    DROPPED.store(false, std::sync::atomic::Ordering::SeqCst);
    let resp = handle(
        &svc(),
        request(
            "Forever",
            "application/connect+json",
            Some("secret"),
            core::encode_frame(b"{}"),
        ),
    )
    .await;
    let mut body = resp.into_body();
    // 1メッセージ受け取ってから、クライアントが切断したことにする(本文を捨てる)。
    let first = body.frame().await.unwrap().unwrap();
    assert!(first.is_data());
    drop(body);
    assert!(
        DROPPED.load(std::sync::atomic::Ordering::SeqCst),
        "切断でストリーム(とその資源)が解放される"
    );
}
