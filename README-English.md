# open-runo

## Beyond WunderGraph Cosmo — Rust + Poem Graph Federation Platform

open-runo is a next-generation Graph Federation, API orchestration, and AI-native gateway platform built with **Rust + Poem**.

---

## Why open-runo Exists

### The REST API Problem

As microservices grow, REST APIs multiply and cause serious problems:

- **BFF hell** — every UI change requires a new backend API to be written, deployed, and maintained
- **Overfetching / N+1** — clients receive more data than needed, or must make many round-trips to assemble one view
- **Documentation rot** — Swagger/OpenAPI specs drift from reality; nobody trusts them
- **Endpoint sprawl** — authentication, security, and rate limiting become impossible to manage consistently
- **Version explosion** — `/v1`, `/v2`, `/v3` accumulate forever and cannot be retired

### What WunderGraph Cosmo (Go) Solves

WunderGraph Cosmo is an excellent platform that solves these problems via **GraphQL Federation**: it unifies multiple REST APIs, gRPC services, and GraphQL services into a single endpoint, eliminating BFF development.

**But Go has fundamental limitations:**

| Problem | Detail |
|---------|--------|
| Garbage collection | Causes unpredictable latency spikes in production |
| Memory safety | Not guaranteed at compile time; runtime bugs remain possible |
| No AI-native routing | No concept of LLM routing or AI Gateway |
| No VersionlessAPI | The root cause of version explosion is not addressed |
| No DUAL DATABASE | No distributed DB strategy or aruaru-db integration |
| No Desktop App | Management UI is web-only |

### REST API vs Cosmo vs open-runo — Feature Comparison

| Problem / Feature | REST API only | Cosmo (Go) | **open-runo (Rust)** |
|---|:---:|:---:|:---:|
| BFF development required | ❌ Every time | ✅ Eliminated | ✅ Eliminated |
| Overfetching / N+1 | ❌ Occurs | ✅ Resolved | ✅ Resolved |
| Documentation rot | ❌ Swagger drifts | ✅ Schema managed | ✅ Schema + Git history |
| Endpoint sprawl | ❌ Fragmented | ✅ Single endpoint | ✅ Single endpoint |
| Unified auth & security | ❌ Per-service | ✅ Gateway-level | ✅ Gateway-level |
| Version explosion (/v1 /v2…) | ❌ Accumulates forever | ⚠️ Not solved | ✅ **VersionlessAPI** |
| GC latency spikes | ⚠️ Depends | ❌ Go GC causes spikes | ✅ **No GC** |
| Compile-time memory safety | ⚠️ Depends | ❌ Runtime risk remains | ✅ **Rust static guarantee** |
| AI-native LLM routing | ❌ None | ❌ None | ✅ **Auto-select provider** |
| DUAL DATABASE (PG + aruaru-db) | ❌ None | ❌ None | ✅ **Supported** |
| Git-like schema / DB history | ❌ None | ❌ None | ✅ **Commit / Rollback** |
| Distributed auto-backup | ❌ None | ❌ None | ✅ **S3 / VPS / Peer** |
| Tauri Desktop App | ❌ None | ❌ None | ✅ **TS + Bootstrap 5** |

See [`docs/why-open-runo.md`](docs/why-open-runo.md) for the full analysis.

---

## Project Vision

open-runo aims to become a next-generation infrastructure layer combining:

- Graph Federation Gateway (Cosmo-equivalent, Rust-speed)
- API Gateway (unified auth, rate limiting, security)
- Schema Registry (Git-like history, stage promotion)
- AI Gateway (Cloud LLM + Local LLM automatic routing)
- Distributed DUAL Database (PostgreSQL + aruaru-db)
- VersionlessAPI Platform (long-term compatibility without versioning)
- Observability Platform (OpenTelemetry / Prometheus / Grafana)
- Quality Gate System (Rust compile-time validation + CI gates)
- Tauri Desktop App (TypeScript + HTML5 + Bootstrap 5)

The goal is not merely to replace a router, but to **fundamentally solve the REST API problem** with a full AI-native development and operation platform.

---

## Core Goals

- Build a Rust-native Graph Federation Gateway
- Use Poem as the modern Rust web framework
- Reduce runtime bugs with strong typing and compile-time validation
- Support distributed routing and schema orchestration
- Provide AI-native routing for cloud LLMs and local LLMs
- Support PostgreSQL and aruaru-db integration
- Provide VersionlessAPI for long-term compatibility
- Support distributed automatic backup
- Support Git-like database and schema history
- Strengthen quality gates before deployment

---

## Main Architecture

```text
open-runo
├── Gateway Router
├── Federation Engine
├── Schema Registry
├── AI Routing Engine
├── VersionlessAPI Engine
├── Database Coordination Layer
├── Distributed Backup Engine
├── Git-like History Engine
├── Observability System
├── Security Layer
└── Quality Gate Pipeline
```

---

## 1. Gateway Router

The Gateway Router is the high-performance entry point of open-runo.

Responsibilities:

- HTTP routing
- GraphQL routing
- Federation query routing
- Authentication middleware
- Authorization middleware
- Rate limiting
- Load balancing
- Intelligent caching
- Request validation
- Error normalization
- Zero-downtime routing updates

Technology:

- Rust
- Poem
- Tokio
- Tower-compatible middleware design

---

## 2. Federation Engine

The Federation Engine combines multiple backend services into one unified API layer.

Supported sources:

- GraphQL services
- PostgreSQL
- aruaru-db
- gRPC services
- OpenAPI-compatible services
- Rust internal services
- AI services

Core functions:

- Schema composition
- Schema validation
- Conflict detection
- Query planning
- Distributed execution
- Federation compatibility checks
- Breaking-change detection

---

## 3. Schema Registry

The Schema Registry manages API schemas, versions, compatibility, and history.

Features:

- Schema registration
- Schema diff
- Breaking-change detection
- Schema approval workflow
- Git-like schema history
- Rollback support
- Environment-specific schema promotion

Example environments:

- local
- development
- staging
- production

---

## 4. VersionlessAPI Engine

open-runo includes a VersionlessAPI design to reduce API version fragmentation.

Instead of constantly creating `/v1`, `/v2`, `/v3` endpoints, open-runo aims to support API evolution through:

- Backward-compatible schema changes
- Compatibility mapping
- Field-level deprecation
- Automatic transformation rules
- Client capability detection
- Schema history tracking
- Safe migration windows

The purpose is to reduce API maintenance cost and prevent unnecessary breaking changes.

---

## 5. AI Routing Engine

open-runo is designed as an AI-native platform.

The AI Routing Engine can route requests to the most suitable AI provider or local model.

Supported provider categories:

- OpenAI
- Anthropic Claude
- Google Gemini
- DeepSeek
- Local LLM
- Custom OpenAI-compatible API
- Self-hosted inference server

Routing policies:

- Cost optimization
- Latency optimization
- Model capability matching
- Context length matching
- Fallback routing
- Local-first routing
- Privacy-first routing
- Hardware-aware routing

Example use cases:

- AI code generation
- AI debugging
- AI teacher mode
- AI agent orchestration
- Automated documentation generation
- Automated test generation

---

## 6. Database Coordination Layer

open-runo is not designed as a simple database wrapper. It is designed to coordinate multiple database strategies.

Supported database concepts:

- PostgreSQL integration
- aruaru-db original database design
- Distributed database architecture
- Database migration and transformation
- Schema history
- Data history
- Distributed automatic backup
- Git-like DB change tracking

Planned database targets:

- PostgreSQL
- aruaru-db
- CockroachDB-compatible concepts
- SQLite for local development
- Object storage for backup archives

---

## 7. Distributed Automatic Backup

open-runo includes a distributed backup strategy for application data, schema data, configuration data, and metadata.

Backup targets:

- Local storage
- Remote VPS
- S3-compatible object storage
- Another open-runo node
- Git-compatible archive repository

Features:

- Scheduled backup
- Incremental backup
- Schema backup
- Configuration backup
- Encrypted backup
- Integrity check
- Restore test
- Disaster recovery workflow

---

## 8. Git-like Database and Schema History

open-runo aims to provide Git-like history management for schemas, database changes, and configuration.

Features:

- Commit-like change records
- Diff view
- Rollback
- Branch-like environment separation
- Migration review
- Change approval
- Audit log

This helps reduce accidental production changes and makes debugging easier.

---

## 9. Observability System

open-runo includes production-grade observability from the beginning.

Features:

- Metrics
- Logs
- Traces
- Request timeline
- Error analytics
- Slow query detection
- AI cost tracking
- Federation performance analysis
- Database performance analysis

Planned integrations:

- OpenTelemetry
- Prometheus
- Grafana
- Loki-compatible logging

---

## 10. Security Layer

Security is a core design principle.

Features:

- Authentication
- Authorization
- API key management
- Token validation
- Secret management
- Rate limiting
- Request validation
- Audit logging
- Encrypted backup
- Secure configuration management

---

## 11. Quality Gate Pipeline

open-runo emphasizes fewer small mistakes, fewer bugs, and stronger release gates.

Quality gates:

- Rust format check
- Clippy lint check
- Unit tests
- Integration tests
- API contract tests
- Schema compatibility tests
- Migration tests
- Load tests
- Security checks
- Dependency checks
- Regression tests

Example commands:

```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all
cargo audit
```

The goal is to prevent fragile AI-generated code from entering production without validation.

---

## Recommended Repository Structure

```text
open-runo/
├── README-English.md
├── README-Japan.md
├── Cargo.toml
├── crates/
│   ├── open-runo-router/
│   ├── open-runo-federation/
│   ├── open-runo-schema-registry/
│   ├── open-runo-ai-routing/
│   ├── open-runo-versionless-api/
│   ├── open-runo-db/
│   ├── open-runo-backup/
│   ├── open-runo-history/
│   ├── open-runo-observability/
│   └── open-runo-security/
├── docs/
│   ├── architecture.md
│   ├── api-spec.md
│   ├── federation.md
│   ├── ai-routing.md
│   ├── versionless-api.md
│   ├── database.md
│   ├── backup.md
│   ├── security.md
│   └── quality-gates.md
├── examples/
├── tests/
└── scripts/
```

---

## Technology Stack

Backend (Gateway):

- Rust
- Poem
- Tokio
- Serde
- SQLx

Desktop App (Tauri):

- Tauri 2.x (Rust backend + WebView frontend)
- TypeScript (type-safe UI & API client)
- HTML5 + CSS3 + Bootstrap 5 (pure TS SPA, no JS framework)
- Vite (build tooling)

Database (DUAL):

- PostgreSQL (primary DB)
- aruaru-db (custom-designed DB)
- SQLite for local development
- Redis-compatible cache

Federation:

- GraphQL
- gRPC
- OpenAPI compatibility layer

AI:

- OpenAI-compatible API
- Anthropic-compatible API
- Gemini-compatible API
- DeepSeek-compatible API
- Local LLM runtime integration

Observability:

- OpenTelemetry
- Prometheus
- Grafana

Infrastructure:

- Docker
- Kubernetes
- VPS
- Bare metal

---

## Why Rust + Poem?

Rust provides:

- Memory safety
- Thread safety
- High performance
- Low runtime overhead
- Strong type system
- Better reliability for infrastructure software

Poem provides:

- Modern Rust web framework design
- Flexible routing
- Middleware support
- Async performance
- Clean API structure

Together, Rust + Poem are suitable for a high-performance gateway that must be stable, secure, and maintainable.

---

## Development Roadmap

### Phase 1: Core Foundation

- Rust workspace setup
- Poem-based HTTP router
- Basic health check
- Configuration loader
- PostgreSQL connection
- Logging and tracing
- Quality gate setup

### Phase 2: Federation Core

- Schema registry
- Schema validation
- Federation composition
- Query planning
- Router execution model

### Phase 3: VersionlessAPI and DB Layer

- VersionlessAPI compatibility rules
- PostgreSQL integration
- aruaru-db interface
- Migration tracking
- Git-like schema history

### Phase 4: AI Native Layer

- AI provider registry
- AI routing rules
- Local LLM routing
- Cost and latency tracking
- Fallback system

### Phase 5: Production Platform

- Distributed backup
- Observability dashboard
- Security hardening
- Load testing
- Multi-node deployment
- Kubernetes support

---

## Project Status

open-runo is currently in the design and early development stage.

The first goal is to build a minimal but high-quality Rust + Poem gateway foundation, then gradually add federation, VersionlessAPI, AI routing, database history, distributed backup, and quality gates.

---

## License

License is planned as either:

- Apache License 2.0
- MIT License
- Dual MIT / Apache 2.0

Final license decision is TBD.

---

## Disclaimer

open-runo is an independent experimental project inspired by modern graph federation and API gateway architecture. It is not an official WunderGraph Cosmo project.
