# Federation Engine

Implemented in [`crates/open-runo-federation`](../crates/open-runo-federation).

## Scope (Phase 2)

- `ServiceSchema`: a backend service's exposed types/fields.
- `compose(&[ServiceSchema]) -> Result<ComposedSchema>`: merges N service
  schemas into one federated schema, rejecting duplicate service
  registration.
- `detect_breaking_changes(previous, next) -> Vec<String>`: flags removed
  types/fields between two composed schemas.

## Not yet implemented

Query planning, distributed execution, and full GraphQL/gRPC/OpenAPI
adapter support (see README §2) are out of scope for the current
composition-only implementation. These build on top of `ComposedSchema`
once `open-runo-router` starts dispatching requests through the Federation
Engine (Phase 2 continuation).

## Relationship to Schema Registry

`open-runo-federation` computes *what* a composed schema looks like;
`open-runo-schema-registry` (see `docs/database.md`... actually see its own
crate docs) is responsible for *persisting* schema versions and their
promotion across environments. Wiring composition output into registry
storage is planned but not yet implemented — today they're used
independently.
