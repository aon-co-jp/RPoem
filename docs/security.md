# Security Layer

Implemented in [`crates/open-runo-security`](../crates/open-runo-security).

## Scope (Phase 5)

- `ApiKey::validate(now)`: rejects revoked or expired keys with a
  descriptive `AppError::Validation`, rather than a bare boolean.
- `RateLimiter`: fixed-window limiter (`max_requests` per `window`),
  per-key, safe under concurrent access (`Mutex`, poison-recovering).

## Design notes

The rate limiter uses a fixed window rather than token/leaky bucket for
simplicity; this means a client can burst up to `2 * max_requests` across a
window boundary. If that matters for a given deployment, replace
`RateLimiter`'s internals — the `check(key, now) -> Result<()>` call site
in callers does not need to change.

## Not yet implemented

Authentication flows (OAuth/OIDC/session handling), authorization
(RBAC/ABAC), secret management, and audit logging (README §10) are not yet
implemented. This crate currently covers the two pieces (`ApiKey`,
`RateLimiter`) needed to unblock Phase 1 gateway hardening.
