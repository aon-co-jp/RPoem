//! `open-runo-router`: the Gateway Router, open-runo's fast entrypoint.
//!
//! ## Endpoints
//!
//! REST全廃(CLAUDE.md「2026-11-01追記」)により、管理操作の`/api/*`はすべて
//! GraphQL(`open-runo-gateway`の`POST /graphql`、`admin_ops`/`admin_sys`の共通関数)へ
//! 移した。ここに残るのは次のものだけ:
//!
//! | Method | Path                              | Description                        |
//! |--------|-----------------------------------|------------------------------------|
//! | GET    | `/health`, `/healthz`             | Service health check               |
//! | GET    | `/.well-known/acme-challenge/:t`  | ACME HTTP-01 challenge response    |
//! | POST   | `/mcp`                            | Model Context Protocol (JSON-RPC)  |
//! | *      | `/scim/v2/Users`, `/Groups`       | SCIM 2.0 (RFC 7644, IdP向け標準)   |
//! | POST   | `/internal/db/mutate`             | open-web-server-ledger からの内部経路 |
//! | GET    | `/`, `/pkg/*`, `/manifest.json`   | 管理画面(WASM)の静的配信          |
//!
//! `/scim/v2/*`は`X-Api-Key`(またはセッション)が必要。

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]
#![recursion_limit = "256"]

pub mod acme;
pub mod admin_ops;
pub mod admin_sys;
pub mod audit;
pub mod auth_hyper;
pub mod body_limit;
pub mod connect;
pub mod edfs;
pub mod grpc;
pub mod grpc_admin;
pub mod handlers_hyper;
pub mod hyper_compat;
pub mod keyring;
pub mod maintenance;
pub mod mcp;
pub mod middleware;
pub mod middleware_hyper;
pub mod multipart_stream;
pub mod openapi;
pub mod self_update;
pub mod session;
pub mod state;
pub mod udp_notice;
pub mod validation;
pub mod webhook;

use keyring::{GuardianConfig, KeyGuardian};
use open_runo_core::Config;
use state::AppState;
use std::sync::Arc;

/// Resolve the bind address for the gateway from [`Config`].
pub fn bind_addr(config: &Config) -> &str {
    &config.bind_addr
}

// ── App builder (hyper_compat) ───────────────────────────────────────────

/// Build the poem-free [`hyper_compat::Router`] for the gateway. Registers
/// every REST/SCIM/SSE handler, wrapped in CORS + rate-limit + tracing (in
/// that order, outermost first). GraphQL Subscriptions over WebSocket are
/// not available here (see `open-runo-gateway`'s `graphql_hyper` module
/// doc comment); everything else runs poem-free.
pub async fn build_hyper_app(
    state: Arc<AppState>,
    rate_limit_max: u32,
    rate_limit_window_secs: i64,
) -> hyper_compat::Router {
    use hyper::Method;
    use hyper_compat::{Handler, Router};

    let guardian = Arc::new(KeyGuardian::new(
        Arc::clone(&state.db),
        GuardianConfig::from_env(),
    ));
    let page_cache = Arc::clone(&state.page_cache);
    // `OPEN_RUNO_RATE_LIMIT_REDIS_URL` (+ the `redis-rate-limit` feature)
    // switches this to a Redis-backed limiter shared across every instance
    // behind a load balancer; unset (the default) keeps the existing
    // in-process-only behavior. See `middleware_hyper::
    // build_shared_rate_limiter`'s doc comment for why this closes a real
    // multi-instance gap (docs/deployment-scaling.md).
    let limiter =
        middleware_hyper::build_shared_rate_limiter(rate_limit_max, rate_limit_window_secs).await;

    maintenance::spawn(Arc::clone(&state), Arc::clone(&page_cache));

    // Wrap every non-health route: rate-limit → tracing → CORS → compression
    // (CORS around the limiter/handler so preflight requests never reach
    // them; compression outermost since it operates on whatever response
    // body the inner stack ultimately produced, including CORS preflight
    // and rate-limit-exceeded responses).
    // Periodically flush buffered request metrics to their configured sink
    // (ClickHouse if `OPEN_RUNO_CLICKHOUSE_URL` + the `clickhouse` feature
    // are set, otherwise an in-memory sink nothing reads back from).
    // Deliberately not awaited/blocking here -- same fire-and-forget
    // background-task shape as `maintenance::spawn` just above.
    open_runo_observability::spawn_periodic_flush(
        Arc::clone(&state.request_metrics),
        std::time::Duration::from_secs(30),
    );

    // UDP-IP冗長経路(即時通知)の受信リスナー。`OPEN_RUNO_UDP_NOTICE_BIND`
    // 等が未設定なら何もしない(既定オフ、udp_notice.rsのdoc参照)。
    // 2026-07-23まで、送信側は実装済みなのに受信側がこのエコシステムの
    // どこにも存在しなかった欠落を埋める。
    let _ = udp_notice::spawn_from_env(Arc::clone(&state.udp_notice_stats)).await;

    let wrap = |h: Handler| -> Handler {
        middleware_hyper::with_static_cache_headers(middleware_hyper::with_compression(
            middleware_hyper::with_cors(middleware_hyper::with_tracing(
                middleware_hyper::with_metrics(
                    middleware_hyper::with_shared_rate_limit(h, Arc::clone(&limiter)),
                    Arc::clone(&state.request_metrics),
                ),
            )),
        ))
    };

    Router::new()
        .route(
            Method::GET,
            "/health",
            wrap(hyper_compat::health_handler(Arc::clone(&state))),
        )
        .route(
            Method::GET,
            "/healthz",
            wrap(hyper_compat::health_handler(Arc::clone(&state))),
        )
        .route(
            Method::GET,
            "/.well-known/acme-challenge/:token",
            wrap(acme::challenge_response_handler(Arc::clone(
                &state.acme_challenges,
            ))),
        )
        .route(
            Method::POST,
            "/mcp",
            wrap(mcp::mcp_handler(Arc::clone(&state), Arc::clone(&guardian))),
        )
        .route(
            Method::POST,
            "/internal/db/mutate",
            wrap(handlers_hyper::db_mutate_handler(Arc::clone(&state))),
        )
        .route(
            Method::GET,
            "/scim/v2/Users",
            wrap(handlers_hyper::scim_list_users_handler(
                Arc::clone(&state),
                Arc::clone(&guardian),
            )),
        )
        .route(
            Method::POST,
            "/scim/v2/Users",
            wrap(handlers_hyper::scim_create_user_handler(
                Arc::clone(&state),
                Arc::clone(&guardian),
            )),
        )
        .route(
            Method::GET,
            "/scim/v2/Users/:id",
            wrap(handlers_hyper::scim_get_user_handler(
                Arc::clone(&state),
                Arc::clone(&guardian),
            )),
        )
        .route(
            Method::PUT,
            "/scim/v2/Users/:id",
            wrap(handlers_hyper::scim_replace_user_handler(
                Arc::clone(&state),
                Arc::clone(&guardian),
            )),
        )
        .route(
            Method::DELETE,
            "/scim/v2/Users/:id",
            wrap(handlers_hyper::scim_delete_user_handler(
                Arc::clone(&state),
                Arc::clone(&guardian),
            )),
        )
        .route(
            Method::GET,
            "/scim/v2/Groups",
            wrap(handlers_hyper::scim_list_groups_handler(
                Arc::clone(&state),
                Arc::clone(&guardian),
            )),
        )
        .route(
            Method::POST,
            "/scim/v2/Groups",
            wrap(handlers_hyper::scim_create_group_handler(
                Arc::clone(&state),
                Arc::clone(&guardian),
            )),
        )
        .route(
            Method::GET,
            "/scim/v2/Groups/:id",
            wrap(handlers_hyper::scim_get_group_handler(
                Arc::clone(&state),
                Arc::clone(&guardian),
            )),
        )
        .route(
            Method::PUT,
            "/scim/v2/Groups/:id",
            wrap(handlers_hyper::scim_replace_group_handler(
                Arc::clone(&state),
                Arc::clone(&guardian),
            )),
        )
        .route(
            Method::DELETE,
            "/scim/v2/Groups/:id",
            wrap(handlers_hyper::scim_delete_group_handler(
                Arc::clone(&state),
                Arc::clone(&guardian),
            )),
        )
        // ── WASM frontend bundle (apps/desktop-wasm/www) ─────────────────
        // Directory is configurable via OPEN_RUNO_STATIC_DIR so the
        // binary can be run from any working directory; defaults to the
        // conventional path relative to the repo root (dev convenience).
        .route(
            Method::GET,
            "/",
            wrap(hyper_compat::static_file_handler(
                static_dir().join("index.html"),
                "text/html; charset=utf-8",
            )),
        )
        .route(
            Method::GET,
            "/pkg/open_runo_desktop_wasm.js",
            wrap(hyper_compat::static_file_handler(
                static_dir().join("pkg/open_runo_desktop_wasm.js"),
                "text/javascript",
            )),
        )
        .route(
            Method::GET,
            "/pkg/open_runo_desktop_wasm_bg.wasm",
            wrap(hyper_compat::static_file_handler(
                static_dir().join("pkg/open_runo_desktop_wasm_bg.wasm"),
                "application/wasm",
            )),
        )
        // PWA manifest + icon, so the app can be "installed" from the
        // browser (standalone window, home-screen icon) — the closest
        // no-Tauri equivalent to Tauri's native app-bundle experience.
        .route(
            Method::GET,
            "/manifest.json",
            wrap(hyper_compat::static_file_handler(
                static_dir().join("manifest.json"),
                "application/manifest+json",
            )),
        )
        .route(
            Method::GET,
            "/icon.svg",
            wrap(hyper_compat::static_file_handler(
                static_dir().join("icon.svg"),
                "image/svg+xml",
            )),
        )
        // Every route above registers only its actual method; without this,
        // a cross-origin browser's CORS preflight (OPTIONS) has no handler
        // to reach and the router's own dispatch fallback answers with a
        // bare 405 before with_cors (or any middleware) ever runs. See
        // Router::with_cors_preflight's doc comment.
        .with_cors_preflight()
}

/// Resolve the WASM frontend's static asset directory. Defaults to
/// `apps/desktop-wasm/www` relative to the current working directory
/// (the convention for `cargo run` from the repo root); override with
/// `OPEN_RUNO_STATIC_DIR` for other layouts (e.g. a packaged deploy).
/// Falls back to a `poem-cosmo-tauri/`-prefixed variant for launchers
/// that run `cargo run --manifest-path poem-cosmo-tauri/Cargo.toml`
/// from a parent directory (e.g. this repo's sibling-checkout layout).
fn static_dir() -> std::path::PathBuf {
    if let Ok(dir) = std::env::var("OPEN_RUNO_STATIC_DIR") {
        return std::path::PathBuf::from(dir);
    }
    let direct = std::path::PathBuf::from("apps/desktop-wasm/www");
    if direct.join("index.html").exists() {
        return direct;
    }
    std::path::PathBuf::from("poem-cosmo-tauri/apps/desktop-wasm/www")
}

#[cfg(test)]
mod hyper_app_tests {
    use super::*;

    #[tokio::test]
    async fn hyper_app_serves_health_and_protected_routes() {
        let state = Arc::new(AppState::new());
        let app = build_hyper_app(state, 1_000, 60).await;
        let (addr, _handle) = hyper_compat::serve(app, "127.0.0.1:0".parse().unwrap())
            .await
            .expect("bind ephemeral port");
        let client = reqwest::Client::new();

        let resp = client
            .get(format!("http://{addr}/health"))
            .send()
            .await
            .expect("request should succeed");
        assert_eq!(resp.status(), reqwest::StatusCode::OK);

        let resp = client
            .get(format!("http://{addr}/scim/v2/Users"))
            .send()
            .await
            .expect("request should succeed");
        assert_eq!(resp.status(), reqwest::StatusCode::UNAUTHORIZED);

        let resp = client
            .get(format!("http://{addr}/scim/v2/Users"))
            .header("x-api-key", "test-key")
            .send()
            .await
            .expect("request should succeed");
        assert_eq!(resp.status(), reqwest::StatusCode::OK);

        // CORS header present on a normal (non-preflight) response too.
        assert!(resp.headers().contains_key("access-control-allow-origin"));
    }

    /// A cross-origin browser sending `X-Api-Key` (a non-simple header)
    /// must first send an `OPTIONS` preflight and get a `2xx` with CORS
    /// headers back before it will send the real request at all. Every
    /// route in `build_hyper_app` registers only its own method (GET/
    /// POST/etc.), never OPTIONS -- without `Router::with_cors_preflight`
    /// wired in, this preflight hit the router's own bare-405 fallback
    /// and no cross-origin browser client could ever call any
    /// `api_key_security()`-protected route. Found via an actual
    /// cross-origin browser test (see CLAUDE.md HANDOFF, 2026-07-11).
    #[tokio::test]
    async fn hyper_app_answers_cors_preflight_on_a_real_protected_route() {
        let state = Arc::new(AppState::new());
        let app = build_hyper_app(state, 1_000, 60).await;
        let (addr, _handle) = hyper_compat::serve(app, "127.0.0.1:0".parse().unwrap())
            .await
            .expect("bind ephemeral port");

        let resp = reqwest::Client::new()
            .request(
                reqwest::Method::OPTIONS,
                format!("http://{addr}/scim/v2/Users"),
            )
            .header("origin", "https://example.com")
            .header("access-control-request-method", "GET")
            .header("access-control-request-headers", "x-api-key")
            .send()
            .await
            .expect("request should succeed");
        assert_eq!(resp.status(), reqwest::StatusCode::OK);
        assert!(resp.headers().contains_key("access-control-allow-origin"));
        assert!(resp.headers().contains_key("access-control-allow-headers"));
    }

    #[tokio::test]
    async fn hyper_app_enforces_shared_rate_limit_across_routes() {
        let state = Arc::new(AppState::new());
        let app = build_hyper_app(state, 2, 60).await;
        let (addr, _handle) = hyper_compat::serve(app, "127.0.0.1:0".parse().unwrap())
            .await
            .expect("bind ephemeral port");
        let client = reqwest::Client::new();

        client
            .get(format!("http://{addr}/health"))
            .send()
            .await
            .unwrap();
        client
            .get(format!("http://{addr}/healthz"))
            .send()
            .await
            .unwrap();
        let resp = client
            .get(format!("http://{addr}/health"))
            .send()
            .await
            .expect("request should succeed");
        assert_eq!(resp.status(), reqwest::StatusCode::TOO_MANY_REQUESTS);
    }
}
