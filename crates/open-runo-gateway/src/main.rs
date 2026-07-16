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
use open_runo_appserver::SharedDispatcher;
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
        "starting open-runo-gateway (REST + GraphQL)"
    );

    let state = Arc::new(AppState::new());
    let (graphiql, graphql_post) = open_runo_gateway::graphql_hyper::graphql_handlers(Arc::clone(&state));

    // アプリケーションサーバー層(「第二のTomcat」)の多重テナント管理。
    // `SharedDispatcher`はスレッド間共有・実行時追加/削除が可能なため、
    // ドメインを追加するたびに新しいpoem-cosmo-tauriプロセスを個別
    // インストールする必要が無くなる(「分身の術」構想、2026-07-16)。
    // 管理API(`/admin/appserver-tenants`)自体は常時有効、実際の
    // プロキシリスナー(`OPEN_RUNO_APPSERVER_PROXY_BIND`)は明示設定時のみ。
    let appserver_guardian = Arc::new(KeyGuardian::new(Arc::clone(&state.db), GuardianConfig::from_env()));
    let appserver_dispatcher = Arc::new(SharedDispatcher::new());

    let mut app = build_hyper_app(state, config.rate_limit_max_requests, config.rate_limit_window_secs as i64)
        .await
        .route(hyper::Method::GET, "/graphql", graphiql)
        .route(hyper::Method::POST, "/graphql", graphql_post);
    for (method, pattern, handler) in
        open_runo_gateway::appserver_tenants::routes(Arc::clone(&appserver_dispatcher), appserver_guardian)
    {
        app = app.route(method, pattern, handler);
    }

    // `ThreadedProxyServer`はDropで自らのワーカースレッドを停止するため、
    // `main`が`handle.await`でブロックしている間ずっと生存させる必要が
    // ある(先に束縛を外に置き、有効時のみ`Some`にする)。
    let _appserver_proxy = match std::env::var("OPEN_RUNO_APPSERVER_PROXY_BIND").ok() {
        Some(bind) => {
            let server = ThreadedProxyServer::start(&bind, Arc::clone(&appserver_dispatcher), ServerConfig::default())?;
            tracing::info!(bind_addr = %bind, "appserver tenant proxy listening (shared, multi-domain, no per-domain install)");
            Some(server)
        }
        None => None,
    };

    let addr = config
        .bind_addr
        .parse()
        .map_err(|e| format!("invalid bind_addr {:?}: {e}", config.bind_addr))?;
    let (bound, handle) = hyper_compat::serve(app, addr).await?;
    tracing::info!(%bound, "open-runo-gateway listening");
    handle.await?;

    Ok(())
}
