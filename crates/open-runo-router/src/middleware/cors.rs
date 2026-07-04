//! CORS middleware for the gateway.
//!
//! Wraps [`poem::middleware::Cors`] with open-runo's defaults so browser-based
//! clients (the Tauri desktop app's webview, future web dashboards) can call
//! `/api/*` and `/api/events` cross-origin. Allowed origins are configurable
//! via `OPEN_RUNO_CORS_ALLOWED_ORIGINS` (comma-separated); unset means
//! "allow any origin", which is fine for local development but should be
//! locked down in production deployments.

use poem::middleware::Cors;
use std::env;

/// Build the [`Cors`] middleware from environment configuration.
///
/// - `OPEN_RUNO_CORS_ALLOWED_ORIGINS` — comma-separated list of allowed
///   origins (e.g. `https://app.example.com,tauri://localhost`). If unset
///   or empty, all origins are allowed (`Cors::new()` default).
pub fn build_cors() -> Cors {
    let origins = env::var("OPEN_RUNO_CORS_ALLOWED_ORIGINS").unwrap_or_default();

    let mut cors = Cors::new()
        .allow_methods([
            poem::http::Method::GET,
            poem::http::Method::POST,
            poem::http::Method::PUT,
            poem::http::Method::DELETE,
            poem::http::Method::OPTIONS,
        ])
        .allow_headers(["x-api-key", "authorization", "content-type"])
        .max_age(3600);

    let allowed: Vec<&str> = origins
        .split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .collect();

    for origin in allowed {
        cors = cors.allow_origin(origin);
    }

    cors
}

#[cfg(test)]
mod tests {
    use super::*;
    use poem::{get, handler, test::TestClient, EndpointExt, Route};

    #[handler]
    fn ok() -> &'static str {
        "ok"
    }

    #[tokio::test]
    async fn preflight_request_gets_cors_headers() {
        let app = Route::new().at("/api/test", get(ok)).with(build_cors());
        let client = TestClient::new(app);

        let resp = client
            .options("/api/test")
            .header("origin", "https://example.com")
            .header("access-control-request-method", "GET")
            .send()
            .await;
        resp.assert_status_is_ok();
        resp.assert_header_exist("access-control-allow-origin");
    }
}
