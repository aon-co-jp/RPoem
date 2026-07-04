//! Authentication middleware.
//!
//! Accepts either of two credential forms on non-health routes:
//! - `X-Api-Key: <key>` — Phase 1 accepts any non-empty key; a full
//!   `open-runo-security::ApiKey` registry lookup is wired in Phase 2.
//! - `Authorization: Bearer <jwt>` — verified via
//!   [`open_runo_security::JwtCodec`] when `OPEN_RUNO_JWT_SECRET` is set.
//!   If the env var is unset, JWT auth is disabled and only `X-Api-Key`
//!   is accepted.
//!
//! Routes under `/health` and `/healthz` are exempt (checked inside the
//! middleware by inspecting the request URI path).
//!
//! ## RBAC (Cosmo Enterprise parity)
//!
//! When a [`RbacPolicy`] is attached (`with_rbac` / `OPEN_RUNO_RBAC=enforce`),
//! JWT callers are additionally authorized per route: the request's method +
//! path map to a `(Resource, Action)` pair (see [`required_permission`]) and
//! the token's `roles` must grant it, otherwise **403 Forbidden**.
//! API-key callers are ops-level credentials and bypass RBAC until the key
//! registry (with per-key roles) lands.

use crate::keyring::{KeyDecision, KeyGuardian};
use open_runo_security::oidc::OidcValidator;
use open_runo_security::rbac::{Action, RbacPolicy, Resource};
use open_runo_security::{Claims, JwtCodec};
use poem::{http::Method, http::StatusCode, Endpoint, Error, Middleware, Request, Result};
use std::sync::Arc;

/// Map a request to the permission it requires. `None` = no RBAC needed
/// (unknown/utility paths).
pub fn required_permission(method: &Method, path: &str) -> Option<(Resource, Action)> {
    let resource = if path.starts_with("/api/schemas") {
        Resource::Schema
    } else if path.starts_with("/api/federation") {
        Resource::Federation
    } else if path.starts_with("/api/ai") {
        Resource::AiRouting
    } else if path.starts_with("/api/db") {
        Resource::Database
    } else if path.starts_with("/api/persisted-queries") {
        Resource::PersistedQuery
    } else if path.starts_with("/api/events") {
        // Realtime updates stream federation/schema changes → read access.
        Resource::Federation
    } else if path.starts_with("/scim/")
        || path.starts_with("/api/cache")
        || path.starts_with("/api/backup")
        || path.starts_with("/api/migrate")
        || path.starts_with("/api/integrity")
    {
        // Provisioning, cache, backup, and integrity are administration.
        Resource::Admin
    } else {
        return None;
    };

    let action = match *method {
        Method::GET | Method::HEAD => Action::Read,
        Method::DELETE => Action::Delete,
        // /api/ai/route is an execution call, not a config write.
        _ if resource == Resource::AiRouting => Action::Read,
        _ => Action::Write,
    };

    Some((resource, action))
}

/// Middleware that requires an `X-Api-Key` header or a valid JWT bearer
/// token on non-health routes, with optional per-route RBAC for JWT callers.
#[derive(Debug, Clone, Default)]
pub struct ApiKeyAuth {
    jwt: Option<Arc<JwtCodec>>,
    oidc: Option<Arc<OidcValidator>>,
    rbac: Option<Arc<RbacPolicy>>,
    /// Static bearer secret for SCIM clients (IdPs can only send
    /// `Authorization: Bearer <fixed>`); grants `/scim/*` only.
    scim_token: Option<Arc<String>>,
    /// Self-operating key registry. While its table is empty, key checks
    /// stay permissive (dev mode); once keys exist it verifies, applies
    /// per-key RBAC roles, and quarantines anomalous keys automatically.
    guardian: Option<Arc<KeyGuardian>>,
}

impl ApiKeyAuth {
    /// Build with API-key-only auth (no JWT support).
    pub fn new() -> Self {
        Self { jwt: None, oidc: None, rbac: None, scim_token: None, guardian: None }
    }

    /// Build with JWT bearer-token support enabled alongside `X-Api-Key`.
    pub fn with_jwt(jwt: JwtCodec) -> Self {
        Self {
            jwt: Some(Arc::new(jwt)),
            oidc: None,
            rbac: None,
            scim_token: None,
            guardian: None,
        }
    }

    /// Enable OIDC SSO: bearer tokens are also validated against the IdP's
    /// JWKS (RS256). Works alongside (or instead of) HS256 `with_jwt`.
    #[must_use]
    pub fn with_oidc(mut self, oidc: OidcValidator) -> Self {
        self.oidc = Some(Arc::new(oidc));
        self
    }

    /// Enable per-route RBAC for JWT/OIDC callers.
    #[must_use]
    pub fn with_rbac(mut self, rbac: RbacPolicy) -> Self {
        self.rbac = Some(Arc::new(rbac));
        self
    }

    /// Accept a fixed bearer secret on `/scim/*` routes (IdP provisioning).
    #[must_use]
    pub fn with_scim_token(mut self, token: impl Into<String>) -> Self {
        self.scim_token = Some(Arc::new(token.into()));
        self
    }

    /// Attach the self-operating key registry (wired by `build_app`).
    #[must_use]
    pub fn with_guardian(mut self, guardian: Arc<KeyGuardian>) -> Self {
        self.guardian = Some(guardian);
        self
    }

    /// Convenience constructor:
    /// - `OPEN_RUNO_JWT_SECRET` → HS256 bearer support
    /// - `OPEN_RUNO_OIDC_ISSUER` + `OPEN_RUNO_OIDC_JWKS_FILE`
    ///   (+ optional `OPEN_RUNO_OIDC_AUDIENCE`) → OIDC SSO
    /// - `OPEN_RUNO_RBAC=enforce` → RBAC with built-in roles
    /// - `OPEN_RUNO_SCIM_TOKEN` → fixed bearer secret for `/scim/*`
    pub fn from_env() -> Self {
        let mut auth = match JwtCodec::from_env() {
            Some(jwt) => Self::with_jwt(jwt),
            None => Self::new(),
        };
        match OidcValidator::from_env() {
            Some(Ok(oidc)) => auth = auth.with_oidc(oidc),
            Some(Err(e)) => tracing::warn!(error = %e, "OIDC configuration invalid; SSO disabled"),
            None => {}
        }
        if let Ok(token) = std::env::var("OPEN_RUNO_SCIM_TOKEN") {
            if !token.is_empty() {
                auth = auth.with_scim_token(token);
            }
        }
        match std::env::var("OPEN_RUNO_RBAC").as_deref() {
            Ok("enforce") => auth.with_rbac(RbacPolicy::builtin()),
            _ => auth,
        }
    }
}

impl<E: Endpoint> Middleware<E> for ApiKeyAuth {
    type Output = ApiKeyAuthEndpoint<E>;

    fn transform(&self, ep: E) -> Self::Output {
        ApiKeyAuthEndpoint {
            ep,
            jwt: self.jwt.clone(),
            oidc: self.oidc.clone(),
            rbac: self.rbac.clone(),
            scim_token: self.scim_token.clone(),
            guardian: self.guardian.clone(),
        }
    }
}

#[derive(Debug)]
pub struct ApiKeyAuthEndpoint<E> {
    ep: E,
    jwt: Option<Arc<JwtCodec>>,
    oidc: Option<Arc<OidcValidator>>,
    rbac: Option<Arc<RbacPolicy>>,
    scim_token: Option<Arc<String>>,
    guardian: Option<Arc<KeyGuardian>>,
}

impl<E: Endpoint> Endpoint for ApiKeyAuthEndpoint<E> {
    type Output = E::Output;

    async fn call(&self, req: Request) -> Result<Self::Output> {
        let path = req.uri().path().to_string();

        // Health probes are always public.
        if path == "/health" || path == "/healthz" {
            return self.ep.call(req).await;
        }

        // 1. X-Api-Key header. With an empty registry any non-empty value
        //    passes (dev mode); once KeyGuardian holds keys it verifies,
        //    applies the key's RBAC roles, and auto-quarantines anomalies.
        let api_key = req.header("x-api-key").unwrap_or("").trim().to_string();
        if !api_key.is_empty() {
            let Some(guardian) = &self.guardian else {
                return self.ep.call(req).await;
            };
            match guardian.verify(&api_key, chrono::Utc::now()).await {
                KeyDecision::RegistryEmpty => return self.ep.call(req).await,
                KeyDecision::Ok { owner, roles } => {
                    if let Some(rbac) = &self.rbac {
                        if let Some((resource, action)) =
                            required_permission(req.method(), &path)
                        {
                            if let Err(e) = rbac.check(&roles, resource, action) {
                                return Err(Error::from_string(
                                    e.to_string(),
                                    StatusCode::FORBIDDEN,
                                ));
                            }
                        }
                    }
                    let mut req = req;
                    req.extensions_mut().insert::<Claims>(Claims {
                        sub: owner,
                        exp: 0,
                        roles,
                    });
                    return self.ep.call(req).await;
                }
                KeyDecision::Suspended => {
                    return Err(Error::from_string(
                        "API key quarantined by anomaly detection; retry after cooldown",
                        StatusCode::TOO_MANY_REQUESTS,
                    ));
                }
                KeyDecision::Rejected => {
                    return Err(Error::from_string(
                        "unknown, revoked, or expired API key",
                        StatusCode::UNAUTHORIZED,
                    ));
                }
            }
        }

        // 1.5 SCIM fixed bearer secret, valid only on /scim/* routes.
        if let (Some(expected), true) = (&self.scim_token, path.starts_with("/scim/")) {
            if let Some(auth_header) = req.header("authorization") {
                if let Some(token) = auth_header.strip_prefix("Bearer ") {
                    if token.trim() == expected.as_str() {
                        return self.ep.call(req).await;
                    }
                }
            }
        }

        // 2. Authorization: Bearer <token> — HS256 (JwtCodec) and/or
        //    OIDC SSO (JWKS/RS256), whichever validates first.
        if self.jwt.is_some() || self.oidc.is_some() {
            if let Some(auth_header) = req.header("authorization") {
                if let Some(token) = auth_header.strip_prefix("Bearer ") {
                    let token = token.trim();
                    let claims = self
                        .jwt
                        .as_ref()
                        .and_then(|j| j.decode(token).ok())
                        .or_else(|| self.oidc.as_ref().and_then(|o| o.decode(token).ok()));

                    match claims {
                        Some(claims) => {
                            // RBAC: bearer roles must grant the route's permission.
                            if let Some(rbac) = &self.rbac {
                                if let Some((resource, action)) =
                                    required_permission(req.method(), &path)
                                {
                                    if let Err(e) = rbac.check(&claims.roles, resource, action) {
                                        return Err(Error::from_string(
                                            e.to_string(),
                                            StatusCode::FORBIDDEN,
                                        ));
                                    }
                                }
                            }
                            // Expose claims to downstream handlers (audit log).
                            let mut req = req;
                            req.extensions_mut().insert::<Claims>(claims);
                            return self.ep.call(req).await;
                        }
                        None => {
                            return Err(Error::from_string(
                                "invalid or expired bearer token",
                                StatusCode::UNAUTHORIZED,
                            ));
                        }
                    }
                }
            }
        }

        Err(Error::from_string(
            "missing X-Api-Key header or Authorization: Bearer token",
            StatusCode::UNAUTHORIZED,
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use poem::{get, handler, http::StatusCode, test::TestClient, EndpointExt, Route};

    #[handler]
    fn secret() -> &'static str {
        "secret"
    }

    #[handler]
    fn health() -> &'static str {
        "ok"
    }

    #[tokio::test]
    async fn rejects_missing_key_on_api_route() {
        let app = Route::new().at("/api/test", get(secret)).with(ApiKeyAuth::new());
        let client = TestClient::new(app);
        client
            .get("/api/test")
            .send()
            .await
            .assert_status(StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn accepts_request_with_key() {
        let app = Route::new().at("/api/test", get(secret)).with(ApiKeyAuth::new());
        let client = TestClient::new(app);
        client
            .get("/api/test")
            .header("x-api-key", "dev-key-123")
            .send()
            .await
            .assert_status_is_ok();
    }

    #[tokio::test]
    async fn health_exempt_from_auth() {
        let app = Route::new().at("/health", get(health)).with(ApiKeyAuth::new());
        let client = TestClient::new(app);
        client.get("/health").send().await.assert_status_is_ok();
    }

    #[tokio::test]
    async fn accepts_valid_jwt_bearer_token() {
        let jwt = JwtCodec::new("test-secret");
        let token = jwt
            .encode("alice", vec!["admin".into()], chrono::Duration::hours(1))
            .unwrap();
        let app = Route::new()
            .at("/api/test", get(secret))
            .with(ApiKeyAuth::with_jwt(jwt));
        let client = TestClient::new(app);
        client
            .get("/api/test")
            .header("authorization", format!("Bearer {token}"))
            .send()
            .await
            .assert_status_is_ok();
    }

    #[tokio::test]
    async fn rejects_invalid_jwt_bearer_token() {
        let jwt = JwtCodec::new("test-secret");
        let app = Route::new()
            .at("/api/test", get(secret))
            .with(ApiKeyAuth::with_jwt(jwt));
        let client = TestClient::new(app);
        client
            .get("/api/test")
            .header("authorization", "Bearer not-a-real-token")
            .send()
            .await
            .assert_status(StatusCode::UNAUTHORIZED);
    }

    // ── RBAC ───────────────────────────────────────────────────────────

    fn rbac_app() -> (JwtCodec, impl Endpoint) {
        use open_runo_security::rbac::RbacPolicy;
        let jwt = JwtCodec::new("test-secret");
        let app = Route::new()
            .at("/api/schemas", get(secret).post(secret))
            .with(ApiKeyAuth::with_jwt(JwtCodec::new("test-secret")).with_rbac(RbacPolicy::builtin()));
        (jwt, app)
    }

    #[tokio::test]
    async fn rbac_viewer_can_read_but_not_write() {
        let (jwt, app) = rbac_app();
        let token = jwt
            .encode("viewer-user", vec!["viewer".into()], chrono::Duration::hours(1))
            .unwrap();
        let client = TestClient::new(app);

        client
            .get("/api/schemas")
            .header("authorization", format!("Bearer {token}"))
            .send()
            .await
            .assert_status_is_ok();

        client
            .post("/api/schemas")
            .header("authorization", format!("Bearer {token}"))
            .send()
            .await
            .assert_status(StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    async fn rbac_admin_can_write() {
        let (jwt, app) = rbac_app();
        let token = jwt
            .encode("admin-user", vec!["admin".into()], chrono::Duration::hours(1))
            .unwrap();
        let client = TestClient::new(app);
        client
            .post("/api/schemas")
            .header("authorization", format!("Bearer {token}"))
            .send()
            .await
            .assert_status_is_ok();
    }

    #[tokio::test]
    async fn rbac_does_not_apply_to_api_keys() {
        let (_jwt, app) = rbac_app();
        let client = TestClient::new(app);
        client
            .post("/api/schemas")
            .header("x-api-key", "ops-key")
            .send()
            .await
            .assert_status_is_ok();
    }

    #[test]
    fn permission_map_covers_api_surface() {
        use open_runo_security::rbac::{Action, Resource};
        assert_eq!(
            required_permission(&Method::GET, "/api/schemas/users"),
            Some((Resource::Schema, Action::Read))
        );
        assert_eq!(
            required_permission(&Method::POST, "/api/federation/compose"),
            Some((Resource::Federation, Action::Write))
        );
        assert_eq!(
            required_permission(&Method::POST, "/api/ai/route"),
            Some((Resource::AiRouting, Action::Read))
        );
        assert_eq!(
            required_permission(&Method::DELETE, "/api/db/t/k"),
            Some((Resource::Database, Action::Delete))
        );
        assert_eq!(
            required_permission(&Method::POST, "/api/persisted-queries"),
            Some((Resource::PersistedQuery, Action::Write))
        );
        assert_eq!(required_permission(&Method::GET, "/health"), None);
    }

    // ── OIDC SSO ───────────────────────────────────────────────────────

    #[tokio::test]
    async fn accepts_oidc_rs256_bearer_token() {
        use open_runo_security::oidc::OidcValidator;
        use serde_json::json;

        const PEM: &str = include_str!("../../open-runo-security/testdata/test_rsa.pem");
        const JWKS: &str = include_str!("../../open-runo-security/testdata/test_jwks.json");
        const ISS: &str = "https://idp.example.com/realms/open-runo";

        let mut header = jsonwebtoken::Header::new(jsonwebtoken::Algorithm::RS256);
        header.kid = Some("test-key-1".into());
        let token = jsonwebtoken::encode(
            &header,
            &json!({
                "sub": "sso-user",
                "iss": ISS,
                "exp": (chrono::Utc::now() + chrono::Duration::hours(1)).timestamp(),
                "roles": ["viewer"]
            }),
            &jsonwebtoken::EncodingKey::from_rsa_pem(PEM.as_bytes()).unwrap(),
        )
        .unwrap();

        let oidc = OidcValidator::from_jwks_json(ISS, None, JWKS).unwrap();
        let app = Route::new()
            .at("/api/test", get(secret))
            .with(ApiKeyAuth::new().with_oidc(oidc));
        let client = TestClient::new(app);

        client
            .get("/api/test")
            .header("authorization", format!("Bearer {token}"))
            .send()
            .await
            .assert_status_is_ok();

        client
            .get("/api/test")
            .header("authorization", "Bearer forged-token")
            .send()
            .await
            .assert_status(StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn scim_token_grants_scim_routes_only() {
        let app = Route::new()
            .at("/scim/v2/Users", get(secret))
            .at("/api/test", get(secret))
            .with(ApiKeyAuth::new().with_scim_token("provision-secret"));
        let client = TestClient::new(app);

        client
            .get("/scim/v2/Users")
            .header("authorization", "Bearer provision-secret")
            .send()
            .await
            .assert_status_is_ok();

        // Same token outside /scim/* → 401.
        client
            .get("/api/test")
            .header("authorization", "Bearer provision-secret")
            .send()
            .await
            .assert_status(StatusCode::UNAUTHORIZED);

        // Wrong token on /scim/* → 401.
        client
            .get("/scim/v2/Users")
            .header("authorization", "Bearer wrong")
            .send()
            .await
            .assert_status(StatusCode::UNAUTHORIZED);
    }
}
