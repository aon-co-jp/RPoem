//! open-runo full-stack binary: REST gateway + GraphQL endpoint.
//!
//! Mounts the complete `open-runo-router` REST surface (including the WASM
//! frontend bundle) at `/` and the versionless GraphQL endpoint at
//! `/graphql`, sharing one `AppState`. Runs on the poem-free
//! `hyper_compat` stack (see CLAUDE.md HANDOFF).
//!
//! **Scope note**: GraphQL Subscriptions over WebSocket are not available
//! on this binary yet (see `open_runo_gateway::graphql_hyper`'s doc
//! comment) — only `GET /graphql` (GraphiQL) and `POST /graphql` (query
//! execution).

use open_runo_appserver::server::{ServerConfig, ThreadedProxyServer};
use open_runo_appserver::tenant_bridge::SupervisedTenantRegistry;
use open_runo_core::Config;
use open_runo_router::keyring::{GuardianConfig, KeyGuardian};
use open_runo_router::{build_hyper_app, hyper_compat, state::AppState};
use std::sync::Arc;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = Config::from_env().map_err(|e| format!("config error: {e}"))?;

    open_runo_observability::init_tracing_with_otlp(
        &config.log_level,
        config.otlp_endpoint.as_deref(),
        "open-runo-gateway",
    );

    tracing::info!(
        bind_addr = %config.bind_addr,
        env = %config.environment,
        "starting open-runo-gateway (GraphQL + SCIM/MCP + admin UI)"
    );

    let state = Arc::new(AppState::new());
    let request_metrics = Arc::clone(&state.request_metrics);
    // 管理用 gRPC(`open_runo.admin.v1.Admin`)。通信路は2つ: 素のgRPC(HTTP/2、
    // `OPEN_RUNO_GRPC_BIND_ADDR`で別ポート)と、ブラウザ(管理画面WASM)向けのgRPC-web
    // (本体のHTTPルーター上の`POST /open_runo.admin.v1.Admin/:method`)。
    let grpc_guardian = Arc::new(KeyGuardian::new(
        Arc::clone(&state.db),
        GuardianConfig::from_env(),
    ));
    let admin_grpc = open_runo_router::grpc_admin::AdminGrpc::new(
        Arc::clone(&state),
        Arc::clone(&grpc_guardian),
    );
    let (graphiql, graphql_post) =
        open_runo_gateway::graphql_hyper::graphql_handlers(Arc::clone(&state));
    let (graphql_sdl, graphql_ws) =
        open_runo_gateway::graphql_hyper::graphql_extra_handlers(Arc::clone(&state));
    let (demo_playground, demo_graphql) = open_runo_gateway::demo::demo_handlers();

    // アプリケーションサーバー層(「第二のTomcat」)の多重テナント管理。
    // `SupervisedTenantRegistry`(2026-08-06、`process_lifecycle`と統合)は
    // 「テナント登録(host→backend_addr)」と「そのバックエンドプロセス自体
    // の起動・監視」を1つのレジストリでオプトインに束ねる——`appserver_
    // tenants`(プロキシ専用登録、後方互換)と`appserver_processes`
    // (プロセス管理、新規)の両管理APIが**同じレジストリを共有**するため、
    // どちらのAPI経由で登録したホストも同じ`Dispatcher`/プロキシリスナー
    // から一貫して到達可能になる(スレッド間共有・実行時追加/削除が可能、
    // ドメインを追加するたびに新しいプロセスを個別インストールする必要が
    // 無くなる「分身の術」構想、2026-07-16)。管理API自体は常時有効、
    // 実際のプロキシリスナー(`OPEN_RUNO_APPSERVER_PROXY_BIND`)は
    // 明示設定時のみ。
    let appserver_guardian = Arc::new(KeyGuardian::new(
        Arc::clone(&state.db),
        GuardianConfig::from_env(),
    ));
    let appserver_registry = Arc::new(SupervisedTenantRegistry::new());

    let graphql_limiter = open_runo_router::middleware_hyper::build_shared_rate_limiter(
        config.rate_limit_max_requests,
        config.rate_limit_window_secs as i64,
    )
    .await;

    // ブラウザ向けの入口(gRPC-web / gRPC-web-text / Connect を同じパスで受ける)。
    // 別オリジンのブラウザ(connect-rustのWASMクライアント等)から呼べるようCORSも付ける。
    let grpc_web = open_runo_router::middleware_hyper::with_cors(
        open_runo_router::middleware_hyper::with_tracing(
            open_runo_router::middleware_hyper::with_metrics(
                open_runo_router::middleware_hyper::with_shared_rate_limit(
                    open_runo_router::grpc_admin::browser_handler(admin_grpc.clone()),
                    Arc::clone(&graphql_limiter),
                ),
                Arc::clone(&request_metrics),
            ),
        ),
    );
    let mut app = build_hyper_app(
        state,
        config.rate_limit_max_requests,
        config.rate_limit_window_secs as i64,
    )
    .await
    .route(
        hyper::Method::POST,
        "/open_runo.admin.v1.Admin/:method",
        grpc_web,
    )
    .route(hyper::Method::GET, "/graphql", graphiql)
    .route(hyper::Method::GET, "/graphql/schema.graphql", graphql_sdl)
    .route(hyper::Method::GET, "/graphql/ws", graphql_ws)
    .route(
        hyper::Method::POST,
        "/graphql",
        // REST全廃後は管理操作もここを通るため、旧`/api/*`と同じく
        // 流量制限→メトリクス→トレース→CORS→圧縮の順で包む(流量制限の枠は
        // `build_hyper_app`側とは別に、同じ設定値で持つ)。
        open_runo_router::middleware_hyper::with_compression(
            open_runo_router::middleware_hyper::with_cors(
                open_runo_router::middleware_hyper::with_tracing(
                    open_runo_router::middleware_hyper::with_metrics(
                        open_runo_router::middleware_hyper::with_shared_rate_limit(
                            graphql_post,
                            graphql_limiter,
                        ),
                        request_metrics,
                    ),
                ),
            ),
        ),
    )
    // Fixed, self-contained GraphQL Federation demo (§ see
    // `open_runo_gateway::demo` module doc): composes two sample
    // subgraphs with the real `open-runo-federation` core and serves
    // real, queryable GraphQL over fixed sample data — no separate
    // upstream services or seeded database required.
    .route(hyper::Method::GET, "/demo", demo_playground)
    .route(hyper::Method::POST, "/demo/graphql", demo_graphql);
    for (method, pattern, handler) in open_runo_gateway::appserver_tenants::routes(
        appserver_registry.dispatcher(),
        Arc::clone(&appserver_guardian),
    ) {
        app = app.route(method, pattern, handler);
    }
    for (method, pattern, handler) in open_runo_gateway::appserver_processes::routes(
        Arc::clone(&appserver_registry),
        appserver_guardian,
    ) {
        app = app.route(method, pattern, handler);
    }

    // `ThreadedProxyServer`はDropで自らのワーカースレッドを停止するため、
    // `main`が`handle.await`でブロックしている間ずっと生存させる必要が
    // ある(先に束縛を外に置き、有効時のみ`Some`にする)。
    let _appserver_proxy = match std::env::var("OPEN_RUNO_APPSERVER_PROXY_BIND").ok() {
        Some(bind) => {
            let server = ThreadedProxyServer::start(
                &bind,
                appserver_registry.dispatcher(),
                ServerConfig::default(),
            )?;
            tracing::info!(bind_addr = %bind, "appserver tenant proxy listening (shared, multi-domain, no per-domain install)");
            Some(server)
        },
        None => None,
    };

    // 素のgRPC(HTTP/2)。本番ではTLS1.3必須(S3で`OPEN_RUNO_GRPC_TLS_*`を追加)。
    let _grpc_server = match std::env::var("OPEN_RUNO_GRPC_BIND_ADDR").ok() {
        Some(bind) => {
            let addr = bind
                .parse()
                .map_err(|e| format!("invalid OPEN_RUNO_GRPC_BIND_ADDR {bind:?}: {e}"))?;
            let (bound, handle) =
                open_runo_router::grpc::serve_grpc_with(addr, Some(admin_grpc.clone())).await?;
            tracing::info!(%bound, "admin gRPC listening (h2c)");
            Some(handle)
        },
        None => None,
    };

    // `/graphql` is registered after `build_hyper_app`, so browsers' CORS preflight needs an OPTIONS route here too.
    let app = app.with_cors_preflight();

    let addr = config
        .bind_addr
        .parse()
        .map_err(|e| format!("invalid bind_addr {:?}: {e}", config.bind_addr))?;
    let (bound, handle) = hyper_compat::serve(app, addr).await?;
    tracing::info!(%bound, "open-runo-gateway listening");
    handle.await?;

    Ok(())
}
