//! 管理用 gRPC サービス `open_runo.admin.v1.Admin`(REST全廃・gRPC移行、CLAUDE.md「2026-11-01追記」)。
//!
//! RESTハンドラやGraphQLの`admin`と同じく、処理は通信方式に依存しない共通関数
//! ([`crate::admin_ops`]・[`crate::admin_sys`])に任せ、ここは「プロトコル⇔共通関数」の変換だけを持つ
//! (二重実装しない)。定義の正は`open-runo-proto`の`admin.proto`。
//!
//! 提供する通信路:
//! - 素のgRPC(HTTP/2、`grpc::serve_grpc_with` / TLS1.3は`grpc::serve_grpc_tls`)。CLI・呼び出し側向け。
//! - gRPC-web(HTTP/1.1でも動く、`application/grpc-web+proto`)。ブラウザのWASM向け。
//!   通常のHTTPルーター(`hyper_compat::Router`)に[`grpc_web_handler`]を載せて使う。
//!
//! 認証: gRPCメタデータ(=HTTPヘッダ)の`x-api-key`、またはセッション(`cookie`+`x-csrf-token`)を
//! [`authenticate_with_session`]で検査する(REST時代と同じKeyGuardian/セッション)。権限もREST時代と同じ
//! (認証済みなら実行可)。`SelfIssueKey`/`SessionLogin`/`SessionLogout`だけ入口として鍵なしで呼べる。
//! 更新系は共通関数が監査ログへ記録する。

use crate::admin_ops::{self, AdminError, ComposeService};
use crate::admin_sys::{self, RouteCandidate};
use crate::auth_hyper::authenticate_with_session;
use crate::keyring::KeyGuardian;
use crate::state::AppState;
use bytes::Bytes;
use futures::{Stream, StreamExt};
use hyper::{HeaderMap, Method};
use open_runo_api_types::{FeatureFlagRequest, RegisterSchemaRequest};
use open_runo_proto::prost::Message;
use open_runo_proto::{code, json_to_struct, v1, value_to_json};
use serde::Serialize;
use std::collections::BTreeMap;
use std::pin::Pin;
use std::sync::Arc;

/// 要求1件あたりの最大サイズ(これを超えるとRESOURCE_EXHAUSTED)。
pub const MAX_REQUEST_BYTES: usize = 4 * 1024 * 1024;

/// gRPCのエラー(コード+メッセージ)。
#[derive(Debug, Clone)]
pub struct GrpcErr {
    pub code: i32,
    pub message: String,
}

impl GrpcErr {
    pub fn new(code: i32, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }
}

impl From<AdminError> for GrpcErr {
    fn from(e: AdminError) -> Self {
        Self::new(code::from_http(e.status.as_u16()), e.message)
    }
}

type Stream_ = Pin<Box<dyn Stream<Item = Vec<u8>> + Send + Sync>>;

/// 応答本体。
pub enum Reply {
    Unary(Vec<u8>),
    Stream(Stream_),
}

/// 呼び出し結果(本体 + 応答メタデータ。`SessionLogin`の`set-cookie`など)。
pub struct Output {
    pub reply: Reply,
    pub headers: Vec<(&'static str, String)>,
}

impl Output {
    fn unary(bytes: Vec<u8>) -> Self {
        Self {
            reply: Reply::Unary(bytes),
            headers: Vec::new(),
        }
    }
}

/// `open_runo.admin.v1.Admin`の実体。
#[derive(Clone)]
pub struct AdminGrpc {
    state: Arc<AppState>,
    guardian: Arc<KeyGuardian>,
}

impl std::fmt::Debug for AdminGrpc {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("AdminGrpc")
    }
}

fn dec<M: Message + Default>(bytes: &[u8]) -> Result<M, GrpcErr> {
    M::decode(bytes)
        .map_err(|e| GrpcErr::new(code::INVALID_ARGUMENT, format!("bad request message: {e}")))
}

fn enc<M: Message>(m: &M) -> Output {
    Output::unary(m.encode_to_vec())
}

/// RESTと同じJSON形の結果を`google.protobuf.Struct`で返す。
fn st<T: Serialize>(v: &T) -> Output {
    let json = serde_json::to_value(v).unwrap_or(serde_json::Value::Null);
    enc(&json_to_struct(&json))
}

fn schema_version(v: open_runo_api_types::SchemaVersion) -> v1::SchemaVersion {
    v1::SchemaVersion {
        id: v.id,
        namespace: v.namespace,
        service_name: v.service_name,
        sdl: v.sdl,
        stage: v.stage,
        created_at: v.created_at,
    }
}

fn flag(f: open_runo_api_types::FeatureFlagResponse) -> v1::FeatureFlag {
    v1::FeatureFlag {
        name: f.name,
        enabled: f.enabled,
        rollout_percent: u32::from(f.rollout_percent),
        description: f.description,
    }
}

fn persisted(r: admin_ops::PersistedQueryRecord) -> v1::PersistedQuery {
    v1::PersistedQuery {
        hash: r.hash,
        query: r.query,
        registered_at: r.registered_at,
    }
}

impl AdminGrpc {
    pub fn new(state: Arc<AppState>, guardian: Arc<KeyGuardian>) -> Self {
        Self { state, guardian }
    }

    /// 呼び出し元(REST版の`actor`文字列と同じ形式)。認証失敗はUNAUTHENTICATED/PERMISSION_DENIED。
    async fn actor(&self, headers: &HeaderMap) -> Result<String, GrpcErr> {
        match authenticate_with_session(
            headers,
            &Method::POST,
            &self.guardian,
            &self.state.sessions,
        )
        .await
        {
            Ok(a) => Ok(format!(
                "{}{}",
                if a.via_session { "session:" } else { "" },
                a.owner
            )),
            Err(status) => Err(GrpcErr::new(
                code::from_http(status.as_u16()),
                "authentication required (x-api-key or session metadata)",
            )),
        }
    }

    /// `method`(例: `RegisterSchema`)を実行する。
    pub async fn call(
        &self,
        method: &str,
        headers: &HeaderMap,
        msg: &[u8],
    ) -> Result<Output, GrpcErr> {
        let state = &self.state;
        // 鍵なしで呼べる入口。
        match method {
            "SelfIssueKey" => {
                let r = admin_sys::self_issue_key(state, &self.guardian).await?;
                return Ok(enc(&v1::SelfIssued {
                    api_key: r.api_key,
                    expires_at: r.expires_at,
                }));
            },
            "SessionLogin" => {
                let key = headers
                    .get("x-api-key")
                    .and_then(|v| v.to_str().ok())
                    .unwrap_or("");
                let (login, cookie) =
                    admin_sys::session_login(&self.guardian, &state.sessions, key).await?;
                let mut out = enc(&v1::SessionLoginResponse {
                    owner: login.owner,
                    csrf_token: login.csrf_token,
                    expires_in_secs: login.expires_in_secs,
                });
                out.headers.push(("set-cookie", cookie));
                return Ok(out);
            },
            "SessionLogout" => {
                let sid = crate::session::session_id_from_cookie_header(headers);
                let cookie = admin_sys::session_logout(&state.sessions, sid.as_deref()).await;
                let mut out = enc(&v1::Empty {});
                out.headers.push(("set-cookie", cookie));
                return Ok(out);
            },
            _ => {},
        }

        let actor = self.actor(headers).await?;
        let a = actor.as_str();
        Ok(match method {
            // ── スキーマ / Federation ──
            "RegisterSchema" => {
                let r: v1::RegisterSchemaRequest = dec(msg)?;
                let v = admin_ops::register_schema(
                    state,
                    a,
                    RegisterSchemaRequest {
                        service_name: r.service_name,
                        sdl: r.sdl,
                        stage: r.stage.unwrap_or_else(|| "local".to_string()),
                        namespace: r.namespace,
                    },
                )
                .await;
                enc(&schema_version(v))
            },
            "GetSchema" => {
                let r: v1::GetSchemaRequest = dec(msg)?;
                let v = admin_ops::get_schema(
                    state,
                    r.namespace.as_deref(),
                    &r.service_name,
                    r.stage.as_deref(),
                )?;
                enc(&schema_version(v))
            },
            "SchemaHistory" => {
                let r: v1::SchemaHistoryRequest = dec(msg)?;
                let versions =
                    admin_ops::schema_history(state, r.namespace.as_deref(), &r.service_name)
                        .into_iter()
                        .map(schema_version)
                        .collect();
                enc(&v1::SchemaHistoryResponse { versions })
            },
            "FederationStatus" => {
                let s = admin_ops::federation_status(state);
                enc(&v1::FederationStatusResponse {
                    contributing_services: s.contributing_services,
                    type_count: s.type_count as u64,
                    field_count: s.field_count as u64,
                })
            },
            "ComposeFederation" => {
                let r: v1::ComposeRequest = dec(msg)?;
                let services = r
                    .services
                    .into_iter()
                    .map(|s| ComposeService {
                        service_name: s.service_name,
                        types: s
                            .types
                            .into_iter()
                            .map(|(k, v)| (k, v.fields))
                            .collect::<BTreeMap<_, _>>(),
                        sdl: s.sdl,
                    })
                    .collect();
                let out = admin_ops::compose(state, services)?;
                enc(&v1::ComposeResponse {
                    contributing_services: out.contributing_services,
                    types: out
                        .types
                        .into_iter()
                        .map(|(k, fields)| (k, v1::FieldList { fields }))
                        .collect(),
                    breaking_changes: out.breaking_changes,
                })
            },
            // ── フィーチャーフラグ ──
            "UpsertFeatureFlag" => {
                let r: v1::UpsertFeatureFlagRequest = dec(msg)?;
                let rollout = u8::try_from(r.rollout_percent.unwrap_or(100)).map_err(|_| {
                    GrpcErr::new(
                        code::INVALID_ARGUMENT,
                        "rollout_percent must be between 0 and 100",
                    )
                })?;
                let f = admin_ops::flag_upsert(
                    state,
                    a,
                    FeatureFlagRequest {
                        name: r.name,
                        enabled: r.enabled.unwrap_or(true),
                        rollout_percent: rollout,
                        description: r.description.unwrap_or_default(),
                    },
                )
                .await?;
                enc(&flag(f))
            },
            "ListFeatureFlags" => enc(&v1::ListFeatureFlagsResponse {
                flags: admin_ops::flag_list(state).into_iter().map(flag).collect(),
            }),
            "GetFeatureFlag" => {
                let r: v1::FlagName = dec(msg)?;
                enc(&flag(admin_ops::flag_get(state, &r.name)?))
            },
            "DeleteFeatureFlag" => {
                let r: v1::FlagName = dec(msg)?;
                admin_ops::flag_delete(state, a, &r.name).await?;
                enc(&v1::Empty {})
            },
            "EvaluateFeatureFlag" => {
                let r: v1::EvaluateFlagRequest = dec(msg)?;
                let e = admin_ops::flag_evaluate(state, &r.name, &r.bucket_key)?;
                enc(&v1::FlagEvaluation {
                    name: e.name,
                    bucket_key: e.bucket_key,
                    enabled: e.enabled,
                })
            },
            // ── 永続クエリ / キャッシュ ──
            "RegisterPersistedQuery" => {
                let r: v1::RegisterPersistedQueryRequest = dec(msg)?;
                enc(&persisted(
                    admin_ops::persisted_query_register(state, a, &r.query).await?,
                ))
            },
            "GetPersistedQuery" => {
                let r: v1::PersistedQueryHash = dec(msg)?;
                enc(&persisted(
                    admin_ops::persisted_query_get(state, &r.hash).await?,
                ))
            },
            "PurgeCache" => {
                let r: v1::PurgeRequest = dec(msg)?;
                enc(&v1::PurgeResponse {
                    purged: admin_ops::cache_purge(state, &state.page_cache, a, &r.path).await,
                })
            },
            "PurgeAllCache" => enc(&v1::PurgeResponse {
                purged: admin_ops::cache_purge_all(state, &state.page_cache, a).await,
            }),
            "CacheAiStats" => st(&admin_sys::ai_stats(&state.page_cache)),
            // ── DB ──
            "DbStatus" => st(&admin_sys::db_status(state)),
            "DbRouting" => st(&admin_sys::db_routing()),
            "DbList" => {
                let r: v1::DbTable = dec(msg)?;
                st(&admin_sys::db_list(state, &r.table).await?)
            },
            "DbGet" => {
                let r: v1::DbKey = dec(msg)?;
                st(&admin_sys::db_get(state, &r.table, &r.key).await?)
            },
            "DbGetAtCommit" => {
                let r: v1::DbAtCommit = dec(msg)?;
                st(&admin_sys::db_get_at_commit(state, &r.table, &r.key, &r.commit_id).await?)
            },
            "DbPut" => {
                let r: v1::DbPutRequest = dec(msg)?;
                let value = r
                    .value
                    .as_ref()
                    .map(value_to_json)
                    .unwrap_or(serde_json::Value::Null);
                st(&admin_sys::db_put(state, a, &r.table, &r.key, value).await?)
            },
            "DbDelete" => {
                let r: v1::DbKey = dec(msg)?;
                st(&admin_sys::db_delete(state, a, &r.table, &r.key).await?)
            },
            // ── バックアップ / 移行 / 整合性 ──
            "BackupExport" => st(&admin_sys::backup_export(state, &state.page_cache, a).await?),
            "BackupImport" => {
                let r: v1::BackupImportRequest = dec(msg)?;
                st(&admin_sys::backup_import(state, a, &r.path).await?)
            },
            "BackupRestoreLatest" => st(&admin_sys::backup_restore_latest(state, a).await?),
            "MigrateExportSql" => {
                let r: v1::MigrateExportSqlRequest = dec(msg)?;
                let dialect = admin_sys::parse_dialect(&r.dialect)?;
                st(&admin_sys::migrate_export_sql(state, a, dialect).await?)
            },
            "MigrateExportCsv" => st(&admin_sys::migrate_export_csv(state, a).await?),
            "IntegrityCheck" => st(&admin_sys::integrity_check(state, a).await?),
            // ── 分析 / AI経路 ──
            "AnalyticsRequestsPerMonth" => st(&admin_sys::requests_per_month(state)),
            "AnalyticsOperations" => st(&admin_sys::operations_summary(state)),
            "AiRoute" => {
                let r: v1::AiRouteRequest = dec(msg)?;
                let candidates: Vec<RouteCandidate> = r
                    .candidates
                    .into_iter()
                    .map(|c| RouteCandidate {
                        provider: c.provider,
                        estimated_cost_usd_per_1k_tokens: c.estimated_cost_usd_per_1k_tokens,
                        estimated_latency_ms: c.estimated_latency_ms,
                        is_local: c.is_local,
                        context_length: c.context_length,
                    })
                    .collect();
                st(&admin_sys::ai_route(
                    &r.policy,
                    r.min_context_length,
                    &candidates,
                )?)
            },
            // ── SCIM ──
            "ScimListUsers" => {
                let r: v1::ScimListRequest = dec(msg)?;
                st(&admin_sys::scim_list_users(state, r.filter.as_deref()).await?)
            },
            "ScimCreateUser" => {
                let r: v1::ScimCreateUserRequest = dec(msg)?;
                let body = admin_sys::scim_create_user_named(
                    state,
                    &self.guardian,
                    a,
                    &r.user_name,
                    r.roles,
                )
                .await?;
                enc(&json_to_struct(&body))
            },
            "ScimDeleteUser" => {
                let r: v1::ScimUserId = dec(msg)?;
                admin_sys::scim_delete_user(state, &self.guardian, a, &r.id).await?;
                enc(&v1::Empty {})
            },
            // ── イベント(サーバーストリーミング) ──
            "WatchEvents" => Output {
                reply: Reply::Stream(self.watch_events()),
                headers: Vec::new(),
            },
            other => {
                return Err(GrpcErr::new(
                    code::UNIMPLEMENTED,
                    format!("unknown method: {other}"),
                ))
            },
        })
    }

    /// 旧`/api/events`(履歴件数の増加)・`/api/ws-events`(スキーマ登録イベント)の置き換え。
    fn watch_events(&self) -> Stream_ {
        let rx = self.state.events.subscribe();
        let schema = futures::stream::unfold(rx, |mut rx| async move {
            loop {
                match rx.recv().await {
                    Ok(ev) => {
                        let m = v1::AdminEvent {
                            event: Some(v1::admin_event::Event::Schema(v1::SchemaEvent {
                                service_name: ev.service_name,
                                stage: ev.stage,
                                at: ev.at,
                            })),
                        };
                        return Some((m.encode_to_vec(), rx));
                    },
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => return None,
                }
            }
        });
        let state = Arc::clone(&self.state);
        let initial = state.history.lock().map(|h| h.log().len()).unwrap_or(0);
        let history = futures::stream::unfold((state, initial), |(state, mut last)| async move {
            loop {
                tokio::time::sleep(HISTORY_POLL).await;
                let current = state.history.lock().map(|h| h.log().len()).unwrap_or(last);
                if current > last {
                    last = current;
                    let m = v1::AdminEvent {
                        event: Some(v1::admin_event::Event::History(v1::HistoryEvent {
                            history_len: current as u64,
                        })),
                    };
                    return Some((m.encode_to_vec(), (state, last)));
                }
            }
        });
        Box::pin(futures::stream::select(schema, history))
    }
}

const HISTORY_POLL: std::time::Duration = std::time::Duration::from_secs(5);

// ── 通信路(HTTP) ────────────────────────────────────────────────────────

use crate::hyper_compat::{Body, Request, Response};
use http_body_util::{BodyExt, StreamBody};
use hyper::body::Frame;
use std::convert::Infallible;

/// どちらの通信路かで違う部分(Content-Typeとトレーラーの送り方)。
#[derive(Clone, Copy, PartialEq, Eq)]
enum Wire {
    /// HTTP/2のトレーラー。
    Grpc,
    /// 本体の末尾にトレーラーフレーム(`0x80`)を置く。
    Web,
    /// `application/grpc-web-text`: gRPC-webの各フレームをbase64にしたもの(公式の`grpc-web`
    /// JSクライアントの既定。バイナリのストリーミングが使えない古いブラウザ/XHR向け)。
    WebText,
}

/// 管理サービスのパスから`method`を取り出す(`/open_runo.admin.v1.Admin/<method>`)。
pub fn method_of(path: &str) -> Option<&str> {
    path.strip_prefix('/')?
        .strip_prefix(open_runo_proto::ADMIN_SERVICE)?
        .strip_prefix('/')
        .filter(|m| !m.is_empty() && !m.contains('/'))
}

fn head(wire: Wire, extra: &[(&'static str, String)]) -> hyper::http::response::Builder {
    let ct = match wire {
        Wire::Grpc => "application/grpc+proto",
        Wire::Web => "application/grpc-web+proto",
        Wire::WebText => "application/grpc-web-text+proto",
    };
    let mut b = hyper::Response::builder()
        .status(hyper::StatusCode::OK)
        .header("content-type", ct);
    for (k, v) in extra {
        b = b.header(*k, v.as_str());
    }
    b
}

/// base64(パディング付き)。`grpc-web-text`ではフレームごとに独立してエンコードする
/// (仕様上、クライアントはパディング付きのbase64チャンクの連結を受け取れる)。
fn b64(bytes: &[u8]) -> Bytes {
    use base64::Engine as _;
    Bytes::from(base64::engine::general_purpose::STANDARD.encode(bytes))
}

/// `grpc-web-text`の要求本体(パディング付きチャンクの連結でもよい)を元のバイト列へ戻す。
pub fn decode_text_body(body: &[u8]) -> Option<Vec<u8>> {
    use base64::Engine as _;
    let text: Vec<u8> = body
        .iter()
        .copied()
        .filter(|b| !b.is_ascii_whitespace())
        .collect();
    let mut out = Vec::new();
    let mut rest: &[u8] = &text;
    while !rest.is_empty() {
        // 次のパディング(`=`)の並びの終わりまでを1チャンクとして読む。
        let end = match rest.iter().position(|&b| b == b'=') {
            Some(i) => {
                let mut j = i;
                while j < rest.len() && rest[j] == b'=' {
                    j += 1;
                }
                j
            },
            None => rest.len(),
        };
        out.extend(
            base64::engine::general_purpose::STANDARD
                .decode(&rest[..end])
                .ok()?,
        );
        rest = &rest[end..];
    }
    Some(out)
}

/// データフレーム(gRPCのメッセージフレーム)を通信路の形にする。
fn data_frame(wire: Wire, message: &[u8]) -> Frame<Bytes> {
    let framed = open_runo_proto::encode_frame(message);
    match wire {
        Wire::WebText => Frame::data(b64(&framed)),
        _ => Frame::data(Bytes::from(framed)),
    }
}

fn finish(wire: Wire, status: i32, message: &str) -> Frame<Bytes> {
    match wire {
        Wire::WebText => Frame::data(b64(&open_runo_proto::encode_web_trailers(status, message))),
        Wire::Web => Frame::data(Bytes::from(open_runo_proto::encode_web_trailers(
            status, message,
        ))),
        Wire::Grpc => {
            let mut t = HeaderMap::new();
            t.insert("grpc-status", status.to_string().parse().expect("digits"));
            if let Ok(v) = open_runo_proto::percent_encode(message).parse() {
                t.insert("grpc-message", v);
            }
            Frame::trailers(t)
        },
    }
}

fn error_response(wire: Wire, e: &GrpcErr) -> Response {
    let frames = vec![Ok::<_, Infallible>(finish(wire, e.code, &e.message))];
    let body: Body = BodyExt::boxed(StreamBody::new(futures::stream::iter(frames)));
    head(wire, &[]).body(body).expect("fixed headers are valid")
}

/// gRPC / gRPC-web の1要求を処理する。
async fn handle(admin: &AdminGrpc, req: Request, wire: Wire) -> Response {
    let headers = req.headers().clone();
    let ct = headers
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_string();
    // gRPC-webの入口では、Content-Typeでバイナリ版とbase64版を切り替える。
    let wire = if wire == Wire::Web && ct.starts_with("application/grpc-web-text") {
        Wire::WebText
    } else {
        wire
    };
    let Some(method) = method_of(req.uri().path()).map(str::to_string) else {
        return error_response(wire, &GrpcErr::new(code::UNIMPLEMENTED, "unknown service"));
    };
    let ok_ct = match wire {
        Wire::Grpc => ct.starts_with("application/grpc") && !ct.starts_with("application/grpc-web"),
        Wire::Web | Wire::WebText => ct.starts_with("application/grpc-web"),
    };
    if !ok_ct {
        return error_response(
            wire,
            &GrpcErr::new(code::INVALID_ARGUMENT, "unsupported content-type"),
        );
    }
    let collected = match http_body_util::Limited::new(req.into_body(), MAX_REQUEST_BYTES + 5)
        .collect()
        .await
    {
        Ok(c) => c.to_bytes(),
        Err(_) => {
            return error_response(
                wire,
                &GrpcErr::new(code::RESOURCE_EXHAUSTED, "request too large"),
            );
        },
    };
    let collected = if wire == Wire::WebText {
        match decode_text_body(&collected) {
            Some(b) => Bytes::from(b),
            None => {
                return error_response(
                    wire,
                    &GrpcErr::new(code::INVALID_ARGUMENT, "invalid base64 body"),
                );
            },
        }
    } else {
        collected
    };
    let message = match open_runo_proto::split_frame(&collected) {
        Some((0, body, _)) => body.to_vec(),
        Some(_) => {
            return error_response(
                wire,
                &GrpcErr::new(code::UNIMPLEMENTED, "compressed messages are not supported"),
            );
        },
        None if collected.is_empty() => Vec::new(),
        None => {
            return error_response(
                wire,
                &GrpcErr::new(code::INVALID_ARGUMENT, "malformed gRPC frame"),
            );
        },
    };

    match admin.call(&method, &headers, &message).await {
        Err(e) => error_response(wire, &e),
        Ok(Output {
            reply: Reply::Unary(bytes),
            headers,
        }) => {
            let frames = vec![
                Ok::<_, Infallible>(data_frame(wire, &bytes)),
                Ok(finish(wire, code::OK, "")),
            ];
            let body: Body = BodyExt::boxed(StreamBody::new(futures::stream::iter(frames)));
            head(wire, &headers)
                .body(body)
                .expect("fixed headers are valid")
        },
        Ok(Output {
            reply: Reply::Stream(s),
            headers,
        }) => {
            let data = s.map(move |m| Ok::<_, Infallible>(data_frame(wire, &m)));
            let end =
                futures::stream::once(
                    async move { Ok::<_, Infallible>(finish(wire, code::OK, "")) },
                );
            let body: Body = BodyExt::boxed(StreamBody::new(data.chain(end)));
            head(wire, &headers)
                .body(body)
                .expect("fixed headers are valid")
        },
    }
}

/// 素のgRPC(HTTP/2)の1要求。`grpc::serve_grpc_with`から呼ばれる。
pub async fn handle_grpc(admin: &AdminGrpc, req: Request) -> Response {
    handle(admin, req, Wire::Grpc).await
}

/// gRPC-web用のHTTPハンドラ。`POST /open_runo.admin.v1.Admin/:method`に載せる。
/// (Content-Typeが`application/grpc-web*`のものだけ。[`browser_handler`]はConnectも受ける)
pub fn grpc_web_handler(admin: AdminGrpc) -> crate::hyper_compat::Handler {
    Arc::new(move |req, _params| {
        let admin = admin.clone();
        Box::pin(async move { handle(&admin, req, Wire::Web).await })
    })
}

/// ブラウザ向けの入口。同じパスに載せ、Content-Typeで振り分ける:
/// `application/grpc-web[-text]*` → gRPC-web、`application/proto|json`・`application/connect+*` →
/// [Connect](crate::connect)。どちらもプロキシ無しで普通のHTTPとして動く。
pub fn browser_handler(admin: AdminGrpc) -> crate::hyper_compat::Handler {
    Arc::new(move |req, _params| {
        let admin = admin.clone();
        Box::pin(async move {
            let ct = req
                .headers()
                .get("content-type")
                .and_then(|v| v.to_str().ok())
                .unwrap_or("")
                .to_string();
            if crate::connect::is_connect_content_type(&ct) {
                crate::connect::handle(&admin, req).await
            } else {
                handle(&admin, req, Wire::Web).await
            }
        })
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::keyring::GuardianConfig;

    fn admin() -> (AdminGrpc, Arc<AppState>) {
        let state = Arc::new(AppState::new());
        let guardian = Arc::new(KeyGuardian::new(
            Arc::clone(&state.db),
            GuardianConfig::from_env(),
        ));
        (AdminGrpc::new(Arc::clone(&state), guardian), state)
    }

    fn key_headers(key: &str) -> HeaderMap {
        let mut h = HeaderMap::new();
        h.insert("x-api-key", key.parse().unwrap());
        h
    }

    #[test]
    fn method_of_parses_only_the_admin_service() {
        assert_eq!(
            method_of("/open_runo.admin.v1.Admin/DbList"),
            Some("DbList")
        );
        assert_eq!(method_of("/open_runo.admin.v1.Admin/"), None);
        assert_eq!(method_of("/grpc.health.v1.Health/Check"), None);
        assert_eq!(method_of("/open_runo.admin.v1.Admin/a/b"), None);
    }

    #[tokio::test]
    async fn unauthenticated_calls_are_rejected_but_self_issue_is_open() {
        let (a, _s) = admin();
        // 不明メソッドは認証後にUNIMPLEMENTED(鍵レジストリが空の間は任意の鍵で通る)。
        let e = a
            .call("Nope", &key_headers("dev"), &[])
            .await
            .err()
            .unwrap();
        assert_eq!(e.code, code::UNIMPLEMENTED);
        let e = a
            .call("DbStatus", &HeaderMap::new(), &[])
            .await
            .err()
            .unwrap();
        assert_eq!(e.code, code::UNAUTHENTICATED);
        assert!(a.call("SelfIssueKey", &HeaderMap::new(), &[]).await.is_ok());
    }

    #[tokio::test]
    async fn schema_flow_and_error_codes() {
        let (a, _s) = admin();
        let h = key_headers("dev");
        let req = v1::RegisterSchemaRequest {
            service_name: "users".into(),
            sdl: "type Query { a: Int }".into(),
            stage: Some("staging".into()),
            namespace: None,
        }
        .encode_to_vec();
        let out = a.call("RegisterSchema", &h, &req).await.ok().unwrap();
        let Reply::Unary(b) = out.reply else {
            panic!("unary")
        };
        let v = v1::SchemaVersion::decode(&b[..]).unwrap();
        assert_eq!(
            (
                v.service_name.as_str(),
                v.stage.as_str(),
                v.namespace.as_str()
            ),
            ("users", "staging", "default")
        );

        let missing = v1::GetSchemaRequest {
            service_name: "nope".into(),
            stage: None,
            namespace: None,
        }
        .encode_to_vec();
        assert_eq!(
            a.call("GetSchema", &h, &missing).await.err().unwrap().code,
            code::NOT_FOUND
        );

        // 不正なフラグ値はINVALID_ARGUMENT、重複SCIMはALREADY_EXISTS。
        let bad = v1::UpsertFeatureFlagRequest {
            name: "x".into(),
            enabled: None,
            rollout_percent: Some(101),
            description: None,
        }
        .encode_to_vec();
        assert_eq!(
            a.call("UpsertFeatureFlag", &h, &bad)
                .await
                .err()
                .unwrap()
                .code,
            code::INVALID_ARGUMENT
        );
    }

    #[tokio::test]
    async fn watch_events_streams_a_registered_schema() {
        let (a, state) = admin();
        let h = key_headers("dev");
        let out = a.call("WatchEvents", &h, &[]).await.ok().unwrap();
        let Reply::Stream(mut s) = out.reply else {
            panic!("stream")
        };
        let _ = state.events.send(crate::state::SchemaEvent {
            service_name: "evt".into(),
            stage: "local".into(),
            at: "t".into(),
        });
        let first = tokio::time::timeout(std::time::Duration::from_secs(3), s.next())
            .await
            .expect("timed out")
            .expect("item");
        let ev = v1::AdminEvent::decode(&first[..]).unwrap();
        assert!(
            matches!(ev.event, Some(v1::admin_event::Event::Schema(s)) if s.service_name == "evt")
        );
    }

    // ── 実HTTP/2・実gRPC-webでの通し試験 ──

    use open_runo_proto::client::{ClientConfig, GrpcClient};

    async fn serve_h2c() -> (GrpcClient, Arc<AppState>, tokio::task::JoinHandle<()>) {
        let (a, state) = admin();
        let (addr, h) = crate::grpc::serve_grpc_with("127.0.0.1:0".parse().unwrap(), Some(a))
            .await
            .unwrap();
        let client = GrpcClient::new(ClientConfig {
            endpoint: format!("http://{addr}"),
            api_key: Some("dev".into()),
            ..Default::default()
        });
        (client, state, h)
    }

    #[tokio::test]
    async fn real_http2_unary_flow_and_status_codes() {
        let (c, _state, _h) = serve_h2c().await;
        let v: v1::SchemaVersion = c
            .unary(
                "RegisterSchema",
                &v1::RegisterSchemaRequest {
                    service_name: "users".into(),
                    sdl: "type Query { a: Int }".into(),
                    stage: None,
                    namespace: None,
                },
            )
            .await
            .unwrap();
        assert_eq!(v.stage, "local");
        let h: v1::SchemaHistoryResponse = c
            .unary(
                "SchemaHistory",
                &v1::SchemaHistoryRequest {
                    service_name: "users".into(),
                    namespace: None,
                },
            )
            .await
            .unwrap();
        assert_eq!(h.versions.len(), 1);

        // エラーはgrpc-statusで返る。
        let e = c
            .unary::<_, v1::SchemaVersion>(
                "GetSchema",
                &v1::GetSchemaRequest {
                    service_name: "nope".into(),
                    stage: None,
                    namespace: None,
                },
            )
            .await
            .unwrap_err();
        assert_eq!(e.code, code::NOT_FOUND);
        // 鍵なしはUNAUTHENTICATED。
        let anon = c.with_api_key(None);
        let e = anon
            .unary::<_, open_runo_proto::prost_types::Struct>("DbStatus", &v1::Empty {})
            .await
            .unwrap_err();
        assert_eq!(e.code, code::UNAUTHENTICATED);
        // Structの応答はJSONへ戻せる(件数は整数のまま)。
        let s: open_runo_proto::prost_types::Struct =
            c.unary("DbStatus", &v1::Empty {}).await.unwrap();
        assert_eq!(open_runo_proto::struct_to_json(&s)["backend"], "in-memory");
    }

    #[tokio::test]
    async fn real_http2_server_streaming_delivers_events() {
        let (c, state, _h) = serve_h2c().await;
        let tx = state.events.clone();
        tokio::spawn(async move {
            tokio::time::sleep(std::time::Duration::from_millis(300)).await;
            let _ = tx.send(crate::state::SchemaEvent {
                service_name: "evt".into(),
                stage: "local".into(),
                at: "t".into(),
            });
        });
        let evs: Vec<v1::AdminEvent> = c
            .stream_some(
                "WatchEvents",
                &v1::Empty {},
                1,
                std::time::Duration::from_secs(5),
            )
            .await
            .unwrap();
        assert_eq!(evs.len(), 1);
        assert!(
            matches!(&evs[0].event, Some(v1::admin_event::Event::Schema(s)) if s.service_name == "evt")
        );
    }

    #[tokio::test]
    async fn grpc_web_over_plain_http_matches_grpc() {
        use crate::hyper_compat::{serve, Router};
        let (a, _state) = admin();
        let app = Router::new().route(
            Method::POST,
            "/open_runo.admin.v1.Admin/:method",
            grpc_web_handler(a),
        );
        let (addr, _h) = serve(app, "127.0.0.1:0".parse().unwrap()).await.unwrap();
        let call = |method: &str, msg: Vec<u8>, key: Option<&str>| {
            let mut r = reqwest::Client::new()
                .post(format!("http://{addr}/open_runo.admin.v1.Admin/{method}"))
                .header("content-type", "application/grpc-web+proto")
                .body(open_runo_proto::encode_frame(&msg));
            if let Some(k) = key {
                r = r.header("x-api-key", k);
            }
            async move { r.send().await.unwrap() }
        };
        let resp = call("SelfIssueKey", v1::Empty {}.encode_to_vec(), None).await;
        assert_eq!(resp.headers()["content-type"], "application/grpc-web+proto");
        let body = resp.bytes().await.unwrap();
        let (flag, msg, rest) = open_runo_proto::split_frame(&body).unwrap();
        assert_eq!(flag, 0);
        let issued = v1::SelfIssued::decode(msg).unwrap();
        assert!(issued.api_key.starts_with("orn_"));
        let (tflag, tbody, _) = open_runo_proto::split_frame(rest).unwrap();
        assert_eq!(tflag, 0x80);
        assert_eq!(open_runo_proto::parse_web_trailers(tbody).0, 0);

        // エラーもトレーラーフレームで返る(HTTPは200)。
        let resp = call("DbStatus", v1::Empty {}.encode_to_vec(), None).await;
        assert_eq!(resp.status(), 200);
        let body = resp.bytes().await.unwrap();
        let (tflag, tbody, _) = open_runo_proto::split_frame(&body).unwrap();
        assert_eq!(tflag, 0x80);
        assert_eq!(
            open_runo_proto::parse_web_trailers(tbody).0,
            code::UNAUTHENTICATED
        );

        // 鍵を付ければ通る。
        let resp = call(
            "DbStatus",
            v1::Empty {}.encode_to_vec(),
            Some(&issued.api_key),
        )
        .await;
        let body = resp.bytes().await.unwrap();
        let (_, msg, _) = open_runo_proto::split_frame(&body).unwrap();
        let s = open_runo_proto::prost_types::Struct::decode(msg).unwrap();
        assert_eq!(open_runo_proto::struct_to_json(&s)["status"], "ok");
    }

    #[tokio::test]
    async fn reflection_lists_and_describes_the_admin_service() {
        let (c, _state, _h) = serve_h2c().await;
        // ServerReflectionRequest { list_services = 7 (string) } — 手書きprotobuf(grpc.rsと同じ流儀)
        let mut req = vec![0x3a, 0x01, b'*'];
        let (frames, _) = c
            .with_api_key(None)
            .call_raw_path(
                "/grpc.reflection.v1.ServerReflection/ServerReflectionInfo",
                &req,
            )
            .await
            .unwrap();
        let text = String::from_utf8_lossy(&frames[0]).to_string();
        assert!(text.contains("open_runo.admin.v1.Admin"), "{text}");
        // file_containing_symbol = 4 (string)
        let sym = b"open_runo.admin.v1.Admin";
        req = vec![0x22, sym.len() as u8];
        req.extend_from_slice(sym);
        let (frames, _) = c
            .call_raw_path(
                "/grpc.reflection.v1.ServerReflection/ServerReflectionInfo",
                &req,
            )
            .await
            .unwrap();
        let text = String::from_utf8_lossy(&frames[0]).to_string();
        assert!(
            text.contains("RegisterSchema") && text.contains("WatchEvents"),
            "{text}"
        );
    }

    #[test]
    fn text_body_decoder_accepts_concatenated_padded_chunks() {
        use base64::Engine as _;
        let e = base64::engine::general_purpose::STANDARD;
        let (a, b) = (e.encode(b"hello"), e.encode(b"world!!"));
        assert_eq!(
            decode_text_body(format!("{a}{b}").as_bytes()).unwrap(),
            b"helloworld!!"
        );
        assert_eq!(
            decode_text_body(format!("{a}\n{b}").as_bytes()).unwrap(),
            b"helloworld!!"
        );
        assert!(decode_text_body(b"@@@@").is_none());
    }

    #[tokio::test]
    async fn grpc_web_text_round_trips_like_binary() {
        use crate::hyper_compat::{serve, Router};
        use base64::Engine as _;
        let e64 = base64::engine::general_purpose::STANDARD;
        let (a, _state) = admin();
        let app = Router::new().route(
            Method::POST,
            "/open_runo.admin.v1.Admin/:method",
            grpc_web_handler(a),
        );
        let (addr, _h) = serve(app, "127.0.0.1:0".parse().unwrap()).await.unwrap();
        let url = |m: &str| format!("http://{addr}/open_runo.admin.v1.Admin/{m}");
        let post = |m: &str, msg: &[u8], key: Option<&str>| {
            let mut r = reqwest::Client::new()
                .post(url(m))
                .header("content-type", "application/grpc-web-text")
                .header("accept", "application/grpc-web-text")
                .body(e64.encode(open_runo_proto::encode_frame(msg)));
            if let Some(k) = key {
                r = r.header("x-api-key", k);
            }
            async move { r.send().await.unwrap() }
        };
        // 応答は base64(データフレーム)+base64(トレーラーフレーム)の連結。
        let resp = post("SelfIssueKey", &v1::Empty {}.encode_to_vec(), None).await;
        assert_eq!(
            resp.headers()["content-type"],
            "application/grpc-web-text+proto"
        );
        let raw = decode_text_body(&resp.bytes().await.unwrap()).unwrap();
        let (flag, msg, rest) = open_runo_proto::split_frame(&raw).unwrap();
        assert_eq!(flag, 0);
        let issued = v1::SelfIssued::decode(msg).unwrap();
        let (tflag, tbody, _) = open_runo_proto::split_frame(rest).unwrap();
        assert_eq!(tflag, 0x80);
        assert_eq!(open_runo_proto::parse_web_trailers(tbody).0, 0);

        // エラーもbase64のトレーラーで返る。
        let resp = post("DbStatus", &v1::Empty {}.encode_to_vec(), None).await;
        let raw = decode_text_body(&resp.bytes().await.unwrap()).unwrap();
        let (_, tbody, _) = open_runo_proto::split_frame(&raw).unwrap();
        assert_eq!(
            open_runo_proto::parse_web_trailers(tbody).0,
            code::UNAUTHENTICATED
        );

        // 鍵付きなら通る。壊れたbase64はINVALID_ARGUMENT。
        let resp = post(
            "DbStatus",
            &v1::Empty {}.encode_to_vec(),
            Some(&issued.api_key),
        )
        .await;
        let raw = decode_text_body(&resp.bytes().await.unwrap()).unwrap();
        let (_, msg, _) = open_runo_proto::split_frame(&raw).unwrap();
        let s = open_runo_proto::prost_types::Struct::decode(msg).unwrap();
        assert_eq!(open_runo_proto::struct_to_json(&s)["status"], "ok");
        let bad = reqwest::Client::new()
            .post(url("DbStatus"))
            .header("content-type", "application/grpc-web-text")
            .body("@@@@")
            .send()
            .await
            .unwrap();
        let raw = decode_text_body(&bad.bytes().await.unwrap()).unwrap();
        let (_, tbody, _) = open_runo_proto::split_frame(&raw).unwrap();
        assert_eq!(
            open_runo_proto::parse_web_trailers(tbody).0,
            code::INVALID_ARGUMENT
        );
    }
}
