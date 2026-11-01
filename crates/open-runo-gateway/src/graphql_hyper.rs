//! Poem-free `/graphql` endpoint, built on `open_runo_router::hyper_compat`
//! instead of `async_graphql_poem`. `async-graphql` itself has no Poem
//! dependency — only its poem integration crate did — so this is a
//! straight port of `graphql_handler`/`graphiql` from `lib.rs` onto plain
//! hyper request/response types.
//!
//! **Scope note**: GraphQL Subscriptions over WebSocket (`/graphql/ws`,
//! the `graphql-ws` protocol) are not ported here yet — hyper's raw
//! `Upgrade` handling for WebSocket is a separate, larger piece of work.
//! `graphql_route` (poem-based, still available in `lib.rs`) remains the
//! only way to get subscriptions until that lands.

use crate::remote_subgraphs::{RemoteSubgraphs, Routed};
use crate::{
    cache_key, is_cacheable_query, persisted_query_hash, CacheConfig, GatewayCache, OpenRunoSchema,
};
use open_runo_cache::InMemoryTtlCache;
use open_runo_persisted_queries::{EnforcementMode, PersistedQueryStore};
use open_runo_router::auth_hyper::authenticate_with_session;
use open_runo_router::hyper_compat::{
    html_response, json_response, read_json_body, text_response, websocket_upgrade, Handler,
    WsMessage,
};
use open_runo_router::keyring::{GuardianConfig, KeyGuardian};
use open_runo_router::state::AppState;
use std::sync::Arc;

/// `GET /graphql` — the GraphiQL playground (static HTML, no auth).
pub fn graphiql_handler() -> Handler {
    Arc::new(move |_req, _params| {
        Box::pin(async move {
            let html = async_graphql::http::GraphiQLSource::build()
                .endpoint("/graphql")
                .finish();
            html_response(hyper::StatusCode::OK, html)
        })
    })
}

/// `POST /graphql` — poem-free port of `graphql_handler`. Same persisted-
/// query resolution and response-cache behavior as the poem version.
pub fn graphql_post_handler(
    schema: OpenRunoSchema,
    store: Arc<PersistedQueryStore>,
    cache: Arc<GatewayCache>,
    remotes: RemoteSubgraphs,
    state: Arc<AppState>,
    guardian: Arc<KeyGuardian>,
) -> Handler {
    Arc::new(move |req, _params| {
        let remotes = remotes.clone();
        let state = Arc::clone(&state);
        let guardian = Arc::clone(&guardian);
        let schema = schema.clone();
        let store = Arc::clone(&store);
        let cache = Arc::clone(&cache);
        Box::pin(async move {
            // 管理操作(`admin`)用の認証: 成功時のみ`Actor`をコンテキストへ渡す。
            // 失敗しても公開フィールドは従来どおり動く(`admin`だけ401相当で失敗)。
            let actor =
                authenticate_with_session(req.headers(), req.method(), &guardian, &state.sessions)
                    .await
                    .ok();
            let http = crate::admin::HttpCtx {
                api_key: req
                    .headers()
                    .get("x-api-key")
                    .and_then(|v| v.to_str().ok())
                    .unwrap_or("")
                    .to_string(),
                session_id: open_runo_router::session::session_id_from_cookie_header(req.headers()),
                set_cookie: Default::default(),
            };
            let set_cookie = Arc::clone(&http.set_cookie);
            let mut request: async_graphql::Request = match read_json_body(req).await {
                Ok(v) => v,
                Err(resp) => return resp,
            };
            request = request.data(http).data(Arc::clone(&guardian));
            if let Some(actor) = actor {
                request = request.data(actor);
            }

            let hash = persisted_query_hash(&request);
            let raw = if request.query.trim().is_empty() {
                None
            } else {
                Some(request.query.clone())
            };

            if hash.is_some() || store.mode() == EnforcementMode::Enforce {
                match store.resolve(hash.as_deref(), raw.as_deref()).await {
                    Ok(document) => request.query = document,
                    Err(e) => {
                        let resp = async_graphql::Response::from_errors(vec![
                            async_graphql::ServerError::new(e.to_string(), None),
                        ]);
                        return json_response(hyper::StatusCode::OK, &resp);
                    },
                }
            }

            // Remote subgraph forwarding (OPEN_RUNO_SUBGRAPHS): see `remote_subgraphs`.
            match remotes
                .route(
                    &request.query,
                    request.operation_name.as_deref(),
                    serde_json::to_value(&request.variables).unwrap_or(serde_json::Value::Null),
                )
                .await
            {
                Routed::Local => {},
                Routed::Forwarded(v) => return json_response(hyper::StatusCode::OK, &v),
                Routed::Rejected(msg) => {
                    let resp = async_graphql::Response::from_errors(vec![
                        async_graphql::ServerError::new(msg, None),
                    ]);
                    return json_response(hyper::StatusCode::OK, &resp);
                },
            }

            let use_cache = cache.config.enabled && is_cacheable_query(&request.query);
            let key = if use_cache {
                Some(cache_key(&request))
            } else {
                None
            };

            if let Some(key) = &key {
                if let Ok(Some(cached)) = cache.store.get(key).await {
                    if let Ok(response) = sonic_rs::from_str::<async_graphql::Response>(&cached) {
                        return json_response(hyper::StatusCode::OK, &response);
                    }
                }
            }

            let response = schema.execute(request).await;

            if let Some(key) = &key {
                if response.is_ok() {
                    if let Ok(serialized) = serde_json::to_string(&response) {
                        let _ = cache.store.set(key, &serialized, cache.config.ttl).await;
                    }
                }
            }

            let mut resp = json_response(hyper::StatusCode::OK, &response);
            let cookie = set_cookie
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .take();
            if let Some(value) = cookie.and_then(|c| c.parse().ok()) {
                resp.headers_mut().insert(hyper::header::SET_COOKIE, value);
            }
            resp
        })
    })
}

/// `GET /graphql/schema.graphql` — GraphQLスキーマをSDLで配信する(旧`/api/openapi*`の
/// 置き換え。認証不要、内容は型定義だけでデータは含まない)。
pub fn schema_sdl_handler(schema: OpenRunoSchema) -> Handler {
    Arc::new(move |_req, _params| {
        let sdl = schema.sdl();
        Box::pin(
            async move { text_response(hyper::StatusCode::OK, "text/plain; charset=utf-8", sdl) },
        )
    })
}

/// `GET /graphql/ws` — GraphQL Subscriptions(graphql-transport-ws / graphql-ws)。
/// 認証はアップグレード要求のヘッダ(`X-Api-Key`/セッションCookie)か、ブラウザ向けに
/// `connection_init`のペイロード(`{"x-api-key": "..."}`)で行う。
pub fn graphql_ws_handler(
    schema: OpenRunoSchema,
    state: Arc<AppState>,
    guardian: Arc<KeyGuardian>,
) -> Handler {
    Arc::new(move |req, _params| {
        let schema = schema.clone();
        let state = Arc::clone(&state);
        let guardian = Arc::clone(&guardian);
        Box::pin(async move {
            use futures::StreamExt;

            let protocol = req
                .headers()
                .get("sec-websocket-protocol")
                .and_then(|v| v.to_str().ok())
                .map(|v| {
                    if v.split(',').any(|p| p.trim() == "graphql-ws") {
                        async_graphql::http::WebSocketProtocols::SubscriptionsTransportWS
                    } else {
                        async_graphql::http::WebSocketProtocols::GraphQLWS
                    }
                })
                .unwrap_or(async_graphql::http::WebSocketProtocols::GraphQLWS);
            // アップグレード要求は`GET`なのでCSRFトークンは要らない(Cookie認証でも通る)。
            let header_actor =
                authenticate_with_session(req.headers(), req.method(), &guardian, &state.sessions)
                    .await
                    .ok();

            websocket_upgrade(req, move |mut conn| {
                Box::pin(async move {
                    let (tx, rx) = tokio::sync::mpsc::unbounded_channel::<String>();
                    let input = futures::stream::unfold(rx, |mut rx| async move {
                        rx.recv().await.map(|m| (m, rx))
                    });
                    let init_guardian = Arc::clone(&guardian);
                    let ws = async_graphql::http::WebSocket::new(schema, input, protocol)
                        .on_connection_init(move |payload: serde_json::Value| async move {
                            let mut data = async_graphql::Data::default();
                            let key = payload
                                .get("x-api-key")
                                .or_else(|| payload.get("apiKey"))
                                .and_then(serde_json::Value::as_str)
                                .map(str::trim)
                                .filter(|k| !k.is_empty());
                            let actor = match key {
                                Some(k) => {
                                    match init_guardian.verify(k, chrono::Utc::now()).await {
                                        open_runo_router::keyring::KeyDecision::RegistryEmpty => {
                                            Some(open_runo_router::auth_hyper::Actor {
                                                owner: "dev".to_string(),
                                                roles: vec![],
                                                via_session: false,
                                            })
                                        },
                                        open_runo_router::keyring::KeyDecision::Ok {
                                            owner,
                                            roles,
                                        } => Some(open_runo_router::auth_hyper::Actor {
                                            owner,
                                            roles,
                                            via_session: false,
                                        }),
                                        _ => None,
                                    }
                                },
                                None => header_actor,
                            };
                            match actor {
                                Some(a) => {
                                    data.insert(a);
                                    Ok(data)
                                },
                                None => Err(async_graphql::Error::new(
                                    "authentication required (X-Api-Key or session cookie)",
                                )),
                            }
                        });
                    let mut ws = Box::pin(ws);
                    loop {
                        tokio::select! {
                            incoming = conn.recv() => match incoming {
                                Some(WsMessage::Text(t)) => {
                                    if tx.send(t).is_err() { break; }
                                },
                                Some(WsMessage::Binary(_)) => {},
                                None => break,
                            },
                            outgoing = ws.next() => match outgoing {
                                Some(async_graphql::http::WsMessage::Text(t)) => {
                                    if conn.send_text(t).await.is_err() { break; }
                                },
                                Some(async_graphql::http::WsMessage::Close(..)) | None => {
                                    let _ = conn.close().await;
                                    break;
                                },
                            },
                        }
                    }
                })
            })
        })
    })
}

/// `GET /graphql/schema.graphql`(SDL配信)と`GET /graphql/ws`(Subscriptions)のハンドラ。
/// スキーマ・鍵管理は`graphql_handlers`と同じ`state`を共有する。
pub fn graphql_extra_handlers(state: Arc<AppState>) -> (Handler, Handler) {
    let schema = crate::build_schema(Arc::clone(&state));
    let guardian = Arc::new(KeyGuardian::new(
        Arc::clone(&state.db),
        GuardianConfig::from_env(),
    ));
    (
        schema_sdl_handler(schema.clone()),
        graphql_ws_handler(schema, state, guardian),
    )
}

/// Build the poem-free `/graphql` GET+POST handlers, pre-wired with
/// `state`. Register both on a `hyper_compat::Router` at the same path
/// (the router dispatches by method, so `GET /graphql` → GraphiQL and
/// `POST /graphql` → query execution can share one path string).
pub fn graphql_handlers(state: Arc<AppState>) -> (Handler, Handler) {
    let schema = crate::build_schema(Arc::clone(&state));
    let guardian = Arc::new(KeyGuardian::new(
        Arc::clone(&state.db),
        GuardianConfig::from_env(),
    ));
    let store = Arc::new(PersistedQueryStore::new(
        Arc::clone(&state.db),
        crate::pq_mode_from_env(),
    ));
    let cache = Arc::new(GatewayCache {
        store: Arc::new(InMemoryTtlCache::new()),
        config: CacheConfig::from_env(),
    });
    (
        graphiql_handler(),
        graphql_post_handler(
            schema,
            store,
            cache,
            RemoteSubgraphs::from_env(),
            state,
            guardian,
        ),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use open_runo_router::hyper_compat::{serve, Router};

    #[tokio::test]
    async fn graphiql_serves_html() {
        let state = Arc::new(AppState::new());
        let (get_h, _post_h) = graphql_handlers(state);
        let router = Router::new().route(hyper::Method::GET, "/graphql", get_h);
        let (addr, _handle) = serve(router, "127.0.0.1:0".parse().unwrap())
            .await
            .expect("bind ephemeral port");

        let resp = reqwest::Client::new()
            .get(format!("http://{addr}/graphql"))
            .send()
            .await
            .expect("request should succeed");
        assert_eq!(resp.status(), reqwest::StatusCode::OK);
        assert_eq!(
            resp.headers().get("content-type").unwrap(),
            "text/html; charset=utf-8"
        );
        let body = resp.text().await.unwrap();
        assert!(body.contains("graphiql") || body.contains("GraphiQL"));
    }

    #[tokio::test]
    async fn graphql_post_executes_health_query() {
        let state = Arc::new(AppState::new());
        let (_get_h, post_h) = graphql_handlers(state);
        let router = Router::new().route(hyper::Method::POST, "/graphql", post_h);
        let (addr, _handle) = serve(router, "127.0.0.1:0".parse().unwrap())
            .await
            .expect("bind ephemeral port");

        let resp = reqwest::Client::new()
            .post(format!("http://{addr}/graphql"))
            .json(&serde_json::json!({ "query": "{ health }" }))
            .send()
            .await
            .expect("request should succeed");
        assert_eq!(resp.status(), reqwest::StatusCode::OK);
        let body: serde_json::Value = resp.json().await.expect("valid json body");
        assert_eq!(body["data"]["health"], "ok");
    }

    #[tokio::test]
    async fn graphql_post_reports_query_errors() {
        let state = Arc::new(AppState::new());
        let (_get_h, post_h) = graphql_handlers(state);
        let router = Router::new().route(hyper::Method::POST, "/graphql", post_h);
        let (addr, _handle) = serve(router, "127.0.0.1:0".parse().unwrap())
            .await
            .expect("bind ephemeral port");

        let resp = reqwest::Client::new()
            .post(format!("http://{addr}/graphql"))
            .json(&serde_json::json!({ "query": "{ notAField }" }))
            .send()
            .await
            .expect("request should succeed");
        assert_eq!(resp.status(), reqwest::StatusCode::OK);
        let body: serde_json::Value = resp.json().await.expect("valid json body");
        assert!(body["errors"].as_array().is_some_and(|e| !e.is_empty()));
    }
}
