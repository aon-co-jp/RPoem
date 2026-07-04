# VersionlessAPI Engine

Implemented in [`crates/open-runo-versionless-api`](../crates/open-runo-versionless-api).

## Scope (Phase 3)

- `CompatibilityRule`: `RenamedField`, `RemovedFieldDefault`, `Deprecated`.
- `apply_compatibility(payload, rules) -> Value`: transforms a JSON
  response so older clients see the shape they expect, without the service
  needing a `/v1`, `/v2`, ... split.
- `deprecated_fields(rules)`: surfaces which fields are deprecated (and
  since when) for documentation/observability, without altering the
  payload.

## Design intent

Rules are expressed from the *new* schema's point of view ("this field is
now called X, older clients still expect Y"), and applied when serving a
response to a client that has negotiated an older capability level. Client
capability negotiation itself (how a request is mapped to "which rules
apply") is not yet implemented — see README §4's "Client Capability
Detection" — and would likely live in `open-runo-router` as middleware that
calls into this crate.
