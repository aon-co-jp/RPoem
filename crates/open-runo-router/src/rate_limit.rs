//! Rate-limiting middleware wrapping [`open_runo_security::RateLimiter`].
//!
//! Keys requests by the `X-Forwarded-For` / `X-Real-IP` header when present
//! (the expected setup behind a reverse proxy / load balancer), falling
//! back to a single shared bucket otherwise. A future revision can key by
//! `req.remote_addr()` directly once open-runo terminates TLS itself rather
//! than always sitting behind a proxy.

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

use std::sync::Arc;

use chrono::Utc;
use open_runo_security::RateLimiter;
use poem::{http::StatusCode, Endpoint, Error, Middleware, Request, Result};

/// Poem middleware enforcing a per-key request rate limit.
///
/// Construct once at startup from [`open_runo_core::Config`]'s
/// `rate_limit_max_requests` / `rate_limit_window_secs` and apply via
/// [`poem::EndpointExt::with`].
#[derive(Debug, Clone)]
pub struct RateLimit {
    limiter: Arc<RateLimiter>,
}

impl RateLimit {
    pub fn new(max_requests: u32, window_secs: i64) -> Self {
        Self {
            limiter: Arc::new(RateLimiter::new(
                max_requests,
                chrono::Duration::seconds(window_secs),
            )),
        }
    }
}

impl<E: Endpoint> Middleware<E> for RateLimit {
    type Output = RateLimitEndpoint<E>;

    fn transform(&self, ep: E) -> Self::Output {
        RateLimitEndpoint {
            ep,
            limiter: self.limiter.clone(),
        }
    }
}

#[derive(Debug)]
pub struct RateLimitEndpoint<E> {
    ep: E,
    limiter: Arc<RateLimiter>,
}

impl<E: Endpoint> Endpoint for RateLimitEndpoint<E> {
    type Output = E::Output;

    async fn call(&self, req: Request) -> Result<Self::Output> {
        let key = client_key(&req);
        self.limiter
            .check(&key, Utc::now())
            .map_err(|e| Error::from_string(e.to_string(), StatusCode::TOO_MANY_REQUESTS))?;
        self.ep.call(req).await
    }
}

/// Derives the rate-limit bucket key for a request. Falls back to a single
/// shared `"anonymous"` bucket when no forwarding header is present, which
/// is adequate for local development but means every un-proxied client
/// shares one budget — acceptable for Phase 1.
fn client_key(req: &Request) -> String {
    req.header("x-forwarded-for")
        .or_else(|| req.header("x-real-ip"))
        .map(str::to_string)
        .unwrap_or_else(|| "anonymous".to_string())
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
    async fn allows_up_to_the_limit_then_blocks() {
        let app = Route::new().at("/", get(ok)).with(RateLimit::new(2, 60));
        let client = TestClient::new(app);

        client.get("/").send().await.assert_status_is_ok();
        client.get("/").send().await.assert_status_is_ok();
        client
            .get("/")
            .send()
            .await
            .assert_status(StatusCode::TOO_MANY_REQUESTS);
    }

    #[tokio::test]
    async fn separate_keys_get_separate_budgets() {
        let app = Route::new().at("/", get(ok)).with(RateLimit::new(1, 60));
        let client = TestClient::new(app);

        client
            .get("/")
            .header("x-forwarded-for", "1.1.1.1")
            .send()
            .await
            .assert_status_is_ok();
        client
            .get("/")
            .header("x-forwarded-for", "2.2.2.2")
            .send()
            .await
            .assert_status_is_ok();
        client
            .get("/")
            .header("x-forwarded-for", "1.1.1.1")
            .send()
            .await
            .assert_status(StatusCode::TOO_MANY_REQUESTS);
    }
}
