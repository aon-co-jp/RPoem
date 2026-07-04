//! open-runo Gateway Router — binary entrypoint.

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
        "starting open-runo-router"
    );

    let state = Arc::new(AppState::new());
    let rate_limit = RateLimit::new(
        config.rate_limit_max_requests,
        config.rate_limit_window_secs as i64,
    );
    // REST-only binary. For REST + GraphQL in one process, run the
    // `open-runo-gateway` binary instead (it mounts this app plus /graphql).
    let app = Route::new().nest("/", build_app(Arc::clone(&state), rate_limit));

    let listener = poem::listener::TcpListener::bind(&config.bind_addr);
    poem::Server::new(listener).run(app).await?;

    Ok(())
}
