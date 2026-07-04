//! open-runo full-stack binary: REST gateway + GraphQL endpoint.
//!
//! Mounts the complete `open-runo-router` REST surface at `/` and the
//! federated GraphQL endpoint at `/graphql`, sharing one `AppState`.

use open_runo_core::Config;
use open_runo_router::{build_app, rate_limit::RateLimit, state::AppState};
use poem::Route;
use std::sync::Arc;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = Config::from_env().map_err(|e| format!("config error: {e}"))?;

    open_runo_observability::init_tracing(&config.log_level);

    tracing::info!(
        bind_addr = %config.bind_addr,
        env = %config.environment,
        "starting open-runo-gateway (REST + GraphQL)"
    );

    let state = Arc::new(AppState::new());
    let rate_limit = RateLimit::new(
        config.rate_limit_max_requests,
        config.rate_limit_window_secs as i64,
    );
    let rest_app = build_app(Arc::clone(&state), rate_limit);
    let graphql_app = open_runo_gateway::graphql_route(Arc::clone(&state));

    // Mount the REST surface at the root and the versionless GraphQL
    // gateway at `/graphql`, sharing the same `AppState`.
    let app = Route::new().nest("/", rest_app).nest("/graphql", graphql_app);

    let listener = poem::listener::TcpListener::bind(&config.bind_addr);
    poem::Server::new(listener).run(app).await?;

    Ok(())
}
