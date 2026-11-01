//! Connect プロトコル(https://connectrpc.com/docs/protocol/)で `open_runo.admin.v1.Admin` を提供する。
//!
//! gRPC-web と違い、Connect は**プロキシ無しで普通の HTTP/1.1・HTTP/2 の POST** としてブラウザ
//! (connect-rust の WASM クライアント、connect-es、curl)から呼べる。単項は枠(フレーム)も
//! トレーラーも無い素の本文、エラーは HTTP ステータス+JSON、ストリーミングはエンベロープ+
//! 終端フレーム(`0x02`)。本文は binary proto(`application/proto`)と proto3-JSON
//! (`application/json`)の両方に対応し、JSON⇔proto の変換は`admin.proto`の記述子から
//! `prost-reflect`で行う(メッセージごとの手書き変換はしない)。
//!
//! 認証・権限・監査は gRPC と同じ [`AdminGrpc::call`] を通るので、通信方式で挙動は変わらない
//! (`x-api-key` またはセッション、`SelfIssueKey`/`SessionLogin`/`SessionLogout`だけ鍵なし)。
//! 圧縮(`content-encoding`)は未対応(identityのみ。指定されたら`unimplemented`)。

use crate::grpc_admin::{AdminGrpc, GrpcErr, Output, Reply, MAX_REQUEST_BYTES};
use crate::hyper_compat::{Body, Request, Response};
use bytes::Bytes;
use futures::StreamExt;
use http_body_util::{BodyExt, StreamBody};
use hyper::body::Frame;
use open_runo_proto::{code, FILE_DESCRIPTOR_SET};
use prost_reflect::{DescriptorPool, DynamicMessage, MessageDescriptor, SerializeOptions};
use std::convert::Infallible;
use std::sync::OnceLock;

fn pool() -> &'static DescriptorPool {
    static POOL: OnceLock<DescriptorPool> = OnceLock::new();
    POOL.get_or_init(|| {
        DescriptorPool::decode(FILE_DESCRIPTOR_SET).expect("admin descriptor set must decode")
    })
}

struct MethodInfo {
    input: MessageDescriptor,
    output: MessageDescriptor,
    server_streaming: bool,
}

fn method_info(method: &str) -> Option<MethodInfo> {
    let svc = pool().get_service_by_name(open_runo_proto::ADMIN_SERVICE)?;
    let m = svc.methods().find(|m| m.name() == method)?;
    Some(MethodInfo {
        input: m.input(),
        output: m.output(),
        server_streaming: m.is_server_streaming(),
    })
}

/// 本文の形式。
#[derive(Clone, Copy, PartialEq, Eq)]
enum Codec {
    Proto,
    Json,
}

/// Connect の `Content-Type` を解釈する。`(コーデック, ストリーミングか)`。
fn parse_content_type(ct: &str) -> Option<(Codec, bool)> {
    let base = ct.split(';').next().unwrap_or("").trim();
    match base {
        "application/proto" => Some((Codec::Proto, false)),
        "application/json" => Some((Codec::Json, false)),
        "application/connect+proto" => Some((Codec::Proto, true)),
        "application/connect+json" => Some((Codec::Json, true)),
        _ => None,
    }
}

/// Connect の `Content-Type` か。gRPC-web との振り分けに使う。
pub fn is_connect_content_type(ct: &str) -> bool {
    parse_content_type(ct).is_some()
}

/// gRPCコード → (Connectのコード名, HTTPステータス)。
fn connect_code(c: i32) -> (&'static str, u16) {
    match c {
        code::INVALID_ARGUMENT => ("invalid_argument", 400),
        code::NOT_FOUND => ("not_found", 404),
        code::ALREADY_EXISTS => ("already_exists", 409),
        code::PERMISSION_DENIED => ("permission_denied", 403),
        code::RESOURCE_EXHAUSTED => ("resource_exhausted", 429),
        code::UNIMPLEMENTED => ("unimplemented", 501),
        code::INTERNAL => ("internal", 500),
        code::UNAUTHENTICATED => ("unauthenticated", 401),
        14 => ("unavailable", 503),
        _ => ("unknown", 500),
    }
}

fn error_json(e: &GrpcErr) -> serde_json::Value {
    let (name, _) = connect_code(e.code);
    serde_json::json!({ "code": name, "message": e.message })
}

fn fixed(status: u16, content_type: &str, body: Vec<u8>) -> Response {
    hyper::Response::builder()
        .status(status)
        .header("content-type", content_type)
        .body(crate::hyper_compat::fixed_body(Bytes::from(body)))
        .expect("fixed headers are valid")
}

fn error_response(e: &GrpcErr) -> Response {
    let (_, status) = connect_code(e.code);
    fixed(
        status,
        "application/json",
        serde_json::to_vec(&error_json(e)).unwrap_or_default(),
    )
}

/// 要求本文(proto/JSON)→ 内部の proto バイト列。
fn to_proto(codec: Codec, desc: &MessageDescriptor, body: &[u8]) -> Result<Vec<u8>, GrpcErr> {
    match codec {
        Codec::Proto => Ok(body.to_vec()),
        Codec::Json => {
            let text: &[u8] = if body.is_empty() { b"{}" } else { body };
            let mut de = serde_json::Deserializer::from_slice(text);
            let msg = DynamicMessage::deserialize(desc.clone(), &mut de).map_err(|e| {
                GrpcErr::new(code::INVALID_ARGUMENT, format!("bad JSON request: {e}"))
            })?;
            de.end().map_err(|e| {
                GrpcErr::new(code::INVALID_ARGUMENT, format!("bad JSON request: {e}"))
            })?;
            Ok(prost_reflect::prost::Message::encode_to_vec(&msg))
        },
    }
}

/// 内部の proto バイト列 → 応答本文(proto/JSON)。
fn from_proto(codec: Codec, desc: &MessageDescriptor, bytes: &[u8]) -> Result<Vec<u8>, GrpcErr> {
    match codec {
        Codec::Proto => Ok(bytes.to_vec()),
        Codec::Json => {
            let msg = DynamicMessage::decode(desc.clone(), bytes)
                .map_err(|e| GrpcErr::new(code::INTERNAL, format!("bad response message: {e}")))?;
            let mut out = Vec::new();
            let mut ser = serde_json::Serializer::new(&mut out);
            // proto3-JSONの標準: camelCase、既定値も出力しない(`stringify_64_bit_integers`は既定でtrue)。
            msg.serialize_with_options(&mut ser, &SerializeOptions::new())
                .map_err(|e| GrpcErr::new(code::INTERNAL, format!("cannot encode JSON: {e}")))?;
            Ok(out)
        },
    }
}

fn envelope(flags: u8, body: &[u8]) -> Bytes {
    let mut out = Vec::with_capacity(5 + body.len());
    out.push(flags);
    out.extend_from_slice(&(body.len() as u32).to_be_bytes());
    out.extend_from_slice(body);
    Bytes::from(out)
}

/// Connect の1要求を処理する(`POST /open_runo.admin.v1.Admin/<method>`)。
pub async fn handle(admin: &AdminGrpc, req: Request) -> Response {
    let Some(method) = crate::grpc_admin::method_of(req.uri().path()).map(str::to_string) else {
        return error_response(&GrpcErr::new(code::UNIMPLEMENTED, "unknown service"));
    };
    if req.method() != hyper::Method::POST {
        return error_response(&GrpcErr::new(code::UNIMPLEMENTED, "only POST is supported"));
    }
    let headers = req.headers().clone();
    let ct = headers
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_string();
    let Some((codec, stream_req)) = parse_content_type(&ct) else {
        return error_response(&GrpcErr::new(
            code::INVALID_ARGUMENT,
            "unsupported content-type",
        ));
    };
    let Some(info) = method_info(&method) else {
        return error_response(&GrpcErr::new(
            code::UNIMPLEMENTED,
            format!("unknown method: {method}"),
        ));
    };
    // 単項は application/proto|json、ストリーミングは application/connect+proto|json で呼ぶ決まり。
    if stream_req != info.server_streaming {
        return error_response(&GrpcErr::new(
            code::INVALID_ARGUMENT,
            if info.server_streaming {
                "this method streams: use application/connect+proto or application/connect+json"
            } else {
                "this method is unary: use application/proto or application/json"
            },
        ));
    }
    let enc_header = if stream_req {
        "connect-content-encoding"
    } else {
        "content-encoding"
    };
    if let Some(enc) = headers.get(enc_header).and_then(|v| v.to_str().ok()) {
        if !enc.eq_ignore_ascii_case("identity") {
            return error_response(&GrpcErr::new(
                code::UNIMPLEMENTED,
                "compression is not supported",
            ));
        }
    }

    let collected = match http_body_util::Limited::new(req.into_body(), MAX_REQUEST_BYTES + 5)
        .collect()
        .await
    {
        Ok(c) => c.to_bytes(),
        Err(_) => {
            return error_response(&GrpcErr::new(code::RESOURCE_EXHAUSTED, "request too large"))
        },
    };
    let body: &[u8] = if stream_req {
        // ストリーミング要求はエンベロープ1個(サーバーストリーミングなので要求は1メッセージ)。
        match open_runo_proto::split_frame(&collected) {
            Some((0, b, _)) => b,
            Some(_) => {
                return error_response(&GrpcErr::new(
                    code::UNIMPLEMENTED,
                    "compressed messages are not supported",
                ));
            },
            None if collected.is_empty() => &[],
            None => {
                return error_response(&GrpcErr::new(code::INVALID_ARGUMENT, "malformed envelope"))
            },
        }
    } else {
        &collected
    };
    let proto = match to_proto(codec, &info.input, body) {
        Ok(p) => p,
        Err(e) => return error_response(&e),
    };

    match admin.call(&method, &headers, &proto).await {
        Err(e) => {
            if stream_req {
                // ストリーミングでは、エラーも終端エンベロープで返す(HTTPは200)。
                let end = serde_json::json!({ "error": error_json(&e) });
                let frames = vec![Ok::<_, Infallible>(Frame::data(envelope(
                    0x02,
                    end.to_string().as_bytes(),
                )))];
                let body: Body = BodyExt::boxed(StreamBody::new(futures::stream::iter(frames)));
                hyper::Response::builder()
                    .status(200)
                    .header("content-type", stream_content_type(codec))
                    .body(body)
                    .expect("fixed headers are valid")
            } else {
                error_response(&e)
            }
        },
        Ok(Output {
            reply: Reply::Unary(bytes),
            headers: extra,
        }) => match from_proto(codec, &info.output, &bytes) {
            Ok(b) => {
                let ct = if codec == Codec::Json {
                    "application/json"
                } else {
                    "application/proto"
                };
                let mut resp = fixed(200, ct, b);
                for (k, v) in extra {
                    if let Ok(v) = v.parse() {
                        resp.headers_mut().append(k, v);
                    }
                }
                resp
            },
            Err(e) => error_response(&e),
        },
        Ok(Output {
            reply: Reply::Stream(s),
            headers: extra,
        }) => {
            let out_desc = info.output.clone();
            let data = s.map(move |m| {
                let b = from_proto(codec, &out_desc, &m).unwrap_or_default();
                Ok::<_, Infallible>(Frame::data(envelope(0, &b)))
            });
            let end = futures::stream::once(async {
                Ok::<_, Infallible>(Frame::data(envelope(0x02, b"{}")))
            });
            let body: Body = BodyExt::boxed(StreamBody::new(data.chain(end)));
            let mut b = hyper::Response::builder()
                .status(200)
                .header("content-type", stream_content_type(codec));
            for (k, v) in extra {
                b = b.header(k, v);
            }
            b.body(body).expect("fixed headers are valid")
        },
    }
}

fn stream_content_type(codec: Codec) -> &'static str {
    match codec {
        Codec::Proto => "application/connect+proto",
        Codec::Json => "application/connect+json",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hyper_compat::{serve, Router};
    use crate::keyring::{GuardianConfig, KeyGuardian};
    use crate::state::AppState;
    use hyper::Method;
    use open_runo_proto::prost::Message;
    use open_runo_proto::v1;
    use std::sync::Arc;

    async fn server() -> (String, Arc<AppState>) {
        let state = Arc::new(AppState::new());
        let guardian = Arc::new(KeyGuardian::new(
            Arc::clone(&state.db),
            GuardianConfig::from_env(),
        ));
        let admin = AdminGrpc::new(Arc::clone(&state), guardian);
        let app = Router::new().route(
            Method::POST,
            "/open_runo.admin.v1.Admin/:method",
            crate::grpc_admin::browser_handler(admin),
        );
        let (addr, h) = serve(app, "127.0.0.1:0".parse().unwrap()).await.unwrap();
        std::mem::forget(h);
        (format!("http://{addr}/open_runo.admin.v1.Admin"), state)
    }

    #[test]
    fn descriptor_knows_streaming_flags() {
        assert!(method_info("WatchEvents").unwrap().server_streaming);
        assert!(!method_info("DbStatus").unwrap().server_streaming);
        assert!(method_info("Nope").is_none());
    }

    #[tokio::test]
    async fn connect_unary_json_and_proto_work_like_grpc() {
        let (base, _s) = server().await;
        let c = reqwest::Client::new();

        // JSON(curlで叩ける形): 鍵の自己発行は認証不要。proto3-JSONはcamelCase。
        let r = c
            .post(format!("{base}/SelfIssueKey"))
            .header("content-type", "application/json")
            .header("connect-protocol-version", "1")
            .body("{}")
            .send()
            .await
            .unwrap();
        assert_eq!(r.status(), 200);
        assert_eq!(r.headers()["content-type"], "application/json");
        let v: serde_json::Value = r.json().await.unwrap();
        let key = v["apiKey"].as_str().unwrap().to_string();
        assert!(key.starts_with("orn_"));

        // JSONでSchemaを登録(camelCaseの要求)。応答もcamelCase。
        let r = c
            .post(format!("{base}/RegisterSchema"))
            .header("content-type", "application/json")
            .header("x-api-key", &key)
            .body(r#"{"serviceName":"users","sdl":"type Query { a: Int }","stage":"staging"}"#)
            .send()
            .await
            .unwrap();
        assert_eq!(r.status(), 200);
        let v: serde_json::Value = r.json().await.unwrap();
        assert_eq!(v["serviceName"], "users");
        assert_eq!(v["stage"], "staging");

        // binary proto。
        let req = v1::SchemaHistoryRequest {
            service_name: "users".into(),
            namespace: None,
        };
        let r = c
            .post(format!("{base}/SchemaHistory"))
            .header("content-type", "application/proto")
            .header("x-api-key", &key)
            .body(req.encode_to_vec())
            .send()
            .await
            .unwrap();
        assert_eq!(r.headers()["content-type"], "application/proto");
        let h = v1::SchemaHistoryResponse::decode(&r.bytes().await.unwrap()[..]).unwrap();
        assert_eq!(h.versions.len(), 1);

        // Struct(任意JSON)はJSONの値のまま返る。
        let r = c
            .post(format!("{base}/DbStatus"))
            .header("content-type", "application/json")
            .header("x-api-key", &key)
            .body("{}")
            .send()
            .await
            .unwrap();
        let v: serde_json::Value = r.json().await.unwrap();
        assert_eq!(v["backend"], "in-memory");
    }

    #[tokio::test]
    async fn connect_errors_use_http_status_and_json_codes() {
        let (base, _s) = server().await;
        let c = reqwest::Client::new();
        // 鍵なし → 401 unauthenticated。
        let r = c
            .post(format!("{base}/DbStatus"))
            .header("content-type", "application/json")
            .body("{}")
            .send()
            .await
            .unwrap();
        assert_eq!(r.status(), 401);
        let v: serde_json::Value = r.json().await.unwrap();
        assert_eq!(v["code"], "unauthenticated");
        // 未登録 → 404 not_found。
        let r = c
            .post(format!("{base}/GetSchema"))
            .header("content-type", "application/json")
            .header("x-api-key", "dev")
            .body(r#"{"serviceName":"nope"}"#)
            .send()
            .await
            .unwrap();
        assert_eq!(r.status(), 404);
        assert_eq!(
            r.json::<serde_json::Value>().await.unwrap()["code"],
            "not_found"
        );
        // 壊れたJSON → 400 invalid_argument、未知メソッド → 501 unimplemented。
        let r = c
            .post(format!("{base}/GetSchema"))
            .header("content-type", "application/json")
            .header("x-api-key", "dev")
            .body("{nope")
            .send()
            .await
            .unwrap();
        assert_eq!(r.status(), 400);
        let r = c
            .post(format!("{base}/Nope"))
            .header("content-type", "application/json")
            .header("x-api-key", "dev")
            .body("{}")
            .send()
            .await
            .unwrap();
        assert_eq!(r.status(), 501);
        // 単項に connect+json は不可(ストリーミング用)。
        let r = c
            .post(format!("{base}/DbStatus"))
            .header("content-type", "application/connect+json")
            .header("x-api-key", "dev")
            .body("")
            .send()
            .await
            .unwrap();
        assert_eq!(r.status(), 400);
    }

    #[tokio::test]
    async fn connect_server_streaming_sends_envelopes_and_end_stream() {
        let (base, state) = server().await;
        let tx = state.events.clone();
        tokio::spawn(async move {
            tokio::time::sleep(std::time::Duration::from_millis(300)).await;
            let _ = tx.send(crate::state::SchemaEvent {
                service_name: "evt".into(),
                stage: "local".into(),
                at: "t".into(),
            });
        });
        let mut r = reqwest::Client::new()
            .post(format!("{base}/WatchEvents"))
            .header("content-type", "application/connect+json")
            .header("x-api-key", "dev")
            .body(open_runo_proto::encode_frame(b"{}"))
            .send()
            .await
            .unwrap();
        assert_eq!(r.headers()["content-type"], "application/connect+json");
        // 最初のエンベロープ(イベント)が届く。
        let mut buf = Vec::new();
        let first = tokio::time::timeout(std::time::Duration::from_secs(5), async {
            loop {
                if let Some((flag, body, _)) = open_runo_proto::split_frame(&buf) {
                    return (flag, body.to_vec());
                }
                let chunk = r.chunk().await.unwrap().expect("stream ended early");
                buf.extend_from_slice(&chunk);
            }
        })
        .await
        .expect("timed out");
        assert_eq!(first.0, 0);
        let ev: serde_json::Value = serde_json::from_slice(&first.1).unwrap();
        assert_eq!(ev["schema"]["serviceName"], "evt");
    }
}
