# poem-cosmo-tauri

**GraphQL Federation platform built with Rust** (Poem/Tauri/Cosmo are never
direct dependencies — their functionality is hand-implemented for
compatibility on tokio+hyper) — WunderGraph Cosmo's paid-plan features,
delivered as OSS. Ships with its own self-learning AI (no external LLM
contract required). Successor to / consolidation target for the former
open-runo and poem-runo repositories. **Both Poem and Tauri — including
in-browser execution capability — are developed from scratch and
reproduced with full compatibility.**

📖 Other languages: [日本語](README-Japan.md) / [中文](README-Chinese.md) /
[한국어](README-Korea.md) / [Español](README-Spain.md) / [Français](README-France.md) /
[Deutsch](README-Germany.md) / [Italiano](README-Italy.md) / [Русский](README-Russia.md) /
[العربية](README-Arabic.md) ·
To integrate open-runo into another project, see **[PORTING.md](PORTING.md)**.

## What is open-runo?

As microservices multiply, REST APIs sprawl out of control (BFF hell, `/v1 /v2`
version explosion, unmanageable endpoint growth). open-runo solves this at the
root with **GraphQL Federation + VersionlessAPI**. Features that WunderGraph
Cosmo (written in Go) only offers on paid plans (Launch / Scale / Enterprise)
are implemented here in pure Rust — **entirely free, as OSS**.

## Feature comparison

| Feature | Cosmo free | Cosmo paid | **open-runo** |
|---|:---:|:---:|:---:|
| GraphQL Federation / Schema Registry | ✅ | ✅ | ✅ |
| GraphQL Subscriptions (WebSocket) | ✅ | ✅ | ✅ |
| Persisted Queries / Trusted Documents | — | ✅ | ✅ **free** |
| Fine-grained RBAC (per route) | — | ✅ | ✅ **free** |
| SSO (OIDC / JWKS RS256) | — | ✅ | ✅ **free** |
| SCIM 2.0 provisioning (Users/Groups) | — | ✅ | ✅ **free** |
| Audit log (stored via Git-on-SQL) | — | ✅ | ✅ **free** |
| Fine-grained rate limiting (token bucket) | — | ✅ | ✅ **free** |
| Response caching | — | ✅ | ✅ **free** |
| Multi-graph / namespaces | — | ✅ | ✅ **free** |
| Request / team-size / retention limits | Yes | Relaxed | **None at all** |

### Features unique to open-runo

- 🧠 **Self-learning AI** (zero external LLM cost) — automatic HTML page cache
  decisions (cold-start prediction via URL pattern generalization), rendering-cost
  learning, adaptive TTL, stale-while-revalidate pre-generation (users never see a MISS)
- 🔑 **KeyGuardian** — fully automated API key lifecycle: SCIM-linked auto issuance
  and revocation, automatic quarantine and recovery for anomalous key usage
- 🗄️ **DUAL DATABASE** — mirrored PostgreSQL + aruaru-db writes, with automatic
  consistency verification and self-healing
- 📦 **One-step move / one-step restore** — export all data + AI learning state
  to a single portable JSON, auto-backed up to two locations (local + a Google
  Drive sync folder), with a one-call `restore-latest`
- 🔀 **Engine conversion & distributed integration** — convert
  MySQL→PostgreSQL→CockroachDB with a single function call (with automatic
  verification), export SQL/CSV for Snowflake, unify internal distributed DBs
  via FederatedBackend
- ⚡ **VersionlessAPI** — a compatibility rule engine that avoids ever creating `/v1 /v2`
- 🖥️ **Desktop management app** compiled from Rust to WebAssembly (no Tauri,
  no Node.js, no TypeScript build chain)

## Quick start

```bash
git clone https://github.com/aon-co-jp/poem-cosmo-tauri
cd poem-cosmo-tauri
cargo test --workspace          # 342 tests (361 with --all-features)
cargo run -p open-runo-gateway  # start the combined REST + GraphQL server (poem-free)
```

```bash
# GraphQL (GraphiQL is served on GET /graphql)
curl -X POST http://localhost:8080/graphql \
     -H 'content-type: application/json' \
     -d '{"query":"{ health }"}'

# Schema registration (REST admin surface)
curl -X POST http://localhost:8080/api/schemas \
     -H 'x-api-key: dev-key' \
     -d '{"service_name":"users","sdl":"type User { id: ID! }"}'
```

### Trying poem-cosmo-tauri-specific features (the Poem/Tauri reimplementation, in practice)

These are unique to this repo, not `open-runo` (see §0.5 of
`docs/HYBRID_NETWORK_ARCHITECTURE.md`) -- they're the concrete output of
this repo's larger mission of reimplementing Poem itself.

```bash
# Verify gzip compression (auto-compresses based on Accept-Encoding)
curl -s -H 'Accept-Encoding: gzip' -o /dev/null -D - \
     http://localhost:8080/api/openapi.json | grep -i content-encoding
# => a "content-encoding: gzip" line confirms it's working

# Generic WebSocket echo (install wscat first: `npm i -g wscat`)
wscat -c ws://localhost:8080/api/ws-echo
# type anything and it's echoed straight back

# Upload a schema as a real multipart file instead of inlining SDL in JSON
echo 'type User { id: ID! }' > users.graphql
curl -X POST http://localhost:8080/api/schemas/upload \
     -H 'x-api-key: dev-key' \
     -F 'service_name=users' \
     -F 'sdl_file=@users.graphql'
```

These are the fastest way to see the gaps listed in `docs/poem-parity.md`
(gzip, generic WebSocket, multipart, etc.) actually working. See that
doc for gRPC health checks, MCP Server, and ACME (HTTP-01/TLS-ALPN-01).

### Using the management UI (WASM frontend)

`cargo run` alone starts the API server; the management UI it serves at
`GET /` (`apps/desktop-wasm`) needs a one-time build:

```bash
rustup target add wasm32-unknown-unknown         # once
cargo install wasm-bindgen-cli --version 0.2.126  # once (must match Cargo.lock's version)
make wasm-frontend                                # generates apps/desktop-wasm/www/pkg
cargo run -p open-runo-gateway                    # now serves the built UI too
```

Open `http://localhost:8080/` for a 10-page admin UI: Dashboard, Schema
Registry, Federation, AI Routing, DUAL DATABASE, SCIM, Persisted Queries,
Feature Flags, Cache & Backup, and Analytics (monthly request-count
metering + per-operation latency/error-rate, `docs/cosmo-parity.md` 4a) —
Rust compiled to WebAssembly, no Tauri/Node.js/TypeScript.

See **[PORTING.md](PORTING.md)** for enabling the AI HTML cache in your own app,
plus the full list of environment variables and endpoints.

## Workspace structure (15 crates)

Composed of `open-runo-router` (REST gateway / auth / audit / AI HTML cache /
self-maintenance), `open-runo-gateway` (GraphQL endpoint: Federation /
Subscriptions / Persisted Queries / response cache), `open-runo-federation`
(schema composition / breaking-change detection), `open-runo-db` (`DbBackend`
abstraction over 9 engines, DUAL / Federated / migrate), `open-runo-security`
(API keys / JWT / OIDC / RBAC / rate limiting), `open-runo-scim` (SCIM 2.0),
`open-runo-cache`, `open-runo-persisted-queries`, `open-runo-ai-routing`,
`open-runo-versionless-api`, and `open-runo-history` / `-backup` /
`-observability`. See [docs/architecture.md](docs/architecture.md) for details.

## Deployment

The same binary runs unmodified on a self-hosted server, a VPS, AWS, or Docker.
Scale from a minimal single-SQLite setup up to `--features full` (DUAL + Redis +
ClickHouse) via feature flags. There is no functionality gated behind a
"managed-only" tier.

## Related Projects

There is a target architecture combining `open-web-server` with this
repository, `open-runo`, PostgreSQL, `aruaru-db`, and `open-raid-z`,
designed to prevent network loss of paid-item and financial/securities
data in 3D online games (quadruple-redundant transport and quadruple-
redundant DB writes, revised 2026-07-11). poem-cosmo-tauri can participate
as a Federation Gateway / backend within that architecture (see
[open-web-server](https://github.com/aon-co-jp/open-web-server)'s
`README.md`/`CLAUDE.md` for the full picture).

## License

Apache-2.0 OR MIT (your choice). See [CONTRIBUTING.md](CONTRIBUTING.md) to contribute.
