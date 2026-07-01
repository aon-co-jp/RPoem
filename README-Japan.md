# OpenCosmo

## WunderGraph Cosmo に着想を得た Rust + Poem NewVersion Graph Federation Platform

OpenCosmo は、**Rust + Poem** で開発する、次世代の Graph Federation、API統合、AIネイティブ Gateway プラットフォームです。

WunderGraph Cosmo は現状 Go 言語を中心に実装されています。OpenCosmo は、その思想を参考にしつつ、Rust の安全性・高速性・低メモリ性を活かして、より堅牢で、AI時代に適した新しいアーキテクチャとして設計します。

OpenCosmo は、WEBアプリ、デスクトップアプリ、モバイルアプリ、AIエージェント、社内システム、エンタープライズ基盤を、BUGを少なく、品質ゲートを強く、長期運用しやすく開発するための基盤です。

---

## プロジェクトビジョン

OpenCosmo は、以下を統合した次世代インフラ基盤を目指します。

- Graph Federation Gateway
- API Gateway
- Schema Registry
- AI Gateway
- 分散DATABASE連携
- VersionlessAPI Platform
- Observability Platform
- Quality Gate System

単なるルーターではなく、AIネイティブな開発・運用プラットフォームを目指します。

---

## コア目標

- Rust Native な Graph Federation Gateway を構築する
- Poem を新しい Rust Web Framework として採用する
- 型安全性とコンパイル時検証で BUG を削減する
- 分散ルーティングと Schema Orchestration を実現する
- Cloud LLM と Local LLM の AI Native Routing に対応する
- PostgreSQL と aruaru-db の連携に対応する
- VersionlessAPI により長期互換性を高める
- 分散自動バックアップに対応する
- Git型 DATABASE / Schema 履歴を実現する
- デプロイ前の品質ゲートを強化する

---

## メインアーキテクチャ

```text
OpenCosmo
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

Gateway Router は OpenCosmo の高速入口です。

役割:

- HTTP Routing
- GraphQL Routing
- Federation Query Routing
- 認証 Middleware
- 認可 Middleware
- Rate Limit
- Load Balancing
- Intelligent Cache
- Request Validation
- Error Normalization
- Zero-downtime Routing Update

技術:

- Rust
- Poem
- Tokio
- Tower互換 Middleware 設計

---

## 2. Federation Engine

Federation Engine は複数の Backend Service を一つの統合 API として扱います。

対応対象:

- GraphQL Service
- PostgreSQL
- aruaru-db
- gRPC Service
- OpenAPI互換 Service
- Rust内部 Service
- AI Service

主な機能:

- Schema Composition
- Schema Validation
- Conflict Detection
- Query Planning
- Distributed Execution
- Federation Compatibility Check
- Breaking Change Detection

---

## 3. Schema Registry

Schema Registry は API Schema、Version、互換性、履歴を管理します。

機能:

- Schema登録
- Schema差分確認
- Breaking Change検出
- Schema承認フロー
- Git型Schema履歴
- Rollback対応
- 環境別Schema昇格

想定環境:

- local
- development
- staging
- production

---

## 4. VersionlessAPI Engine

OpenCosmo は API Version の乱立を防ぐために VersionlessAPI を採用します。

`/v1`、`/v2`、`/v3` のように API を増やし続けるのではなく、以下で進化させます。

- 後方互換Schema変更
- 互換Mapping
- Field単位のDeprecation
- 自動変換Rule
- Client Capability Detection
- Schema履歴管理
- 安全なMigration期間

目的は、API保守コストを下げ、不要なBreaking Changeを減らすことです。

---

## 5. AI Routing Engine

OpenCosmo は AI Native Platform として設計します。

AI Routing Engine は、用途に応じて最適なAI ProviderまたはLocal Modelへルーティングします。

対応Provider分類:

- OpenAI
- Anthropic Claude
- Google Gemini
- DeepSeek
- Local LLM
- Custom OpenAI-compatible API
- Self-hosted Inference Server

Routing Policy:

- Cost Optimization
- Latency Optimization
- Model Capability Matching
- Context Length Matching
- Fallback Routing
- Local-first Routing
- Privacy-first Routing
- Hardware-aware Routing

利用例:

- AI Code Generation
- AI Debugging
- AI Teacher Mode
- AI Agent Orchestration
- Documentation自動生成
- Test自動生成

---

## 6. Database Coordination Layer

OpenCosmo は単なるDATABASE Wrapperではありません。複数のDATABASE戦略を統合・調整するLayerを持ちます。

対応概念:

- PostgreSQL連携
- aruaru-db 独自DATABASE設計
- 分散DATABASE Architecture
- DATABASE Migration / Transformation
- Schema History
- Data History
- 分散自動Backup
- Git型DB変更追跡

計画中のDATABASE対象:

- PostgreSQL
- aruaru-db
- CockroachDB互換思想
- Local開発用SQLite
- Backup Archive用Object Storage

---

## 7. Distributed Automatic Backup

OpenCosmo は、Application Data、Schema Data、Configuration Data、Metadata を対象にした分散Backup戦略を持ちます。

Backup対象:

- Local Storage
- Remote VPS
- S3互換Object Storage
- 別OpenCosmo Node
- Git互換Archive Repository

機能:

- Scheduled Backup
- Incremental Backup
- Schema Backup
- Configuration Backup
- Encrypted Backup
- Integrity Check
- Restore Test
- Disaster Recovery Workflow

---

## 8. Git-like Database and Schema History

OpenCosmo は Schema、DATABASE変更、Configuration を Git のように履歴管理することを目指します。

機能:

- Commit風Change Record
- Diff View
- Rollback
- Branch風Environment分離
- Migration Review
- Change Approval
- Audit Log

これにより、本番環境での事故を減らし、BUG調査や原因特定を容易にします。

---

## 9. Observability System

OpenCosmo は最初から本番運用レベルの監視を重視します。

機能:

- Metrics
- Logs
- Traces
- Request Timeline
- Error Analytics
- Slow Query Detection
- AI Cost Tracking
- Federation Performance Analysis
- Database Performance Analysis

連携予定:

- OpenTelemetry
- Prometheus
- Grafana
- Loki互換Logging

---

## 10. Security Layer

Security は OpenCosmo の中核設計です。

機能:

- Authentication
- Authorization
- API Key Management
- Token Validation
- Secret Management
- Rate Limiting
- Request Validation
- Audit Logging
- Encrypted Backup
- Secure Configuration Management

---

## 11. Quality Gate Pipeline

OpenCosmo は、些細なミス削減、BUG削減、品質ゲート強化を重視します。

Quality Gate:

- Rust Format Check
- Clippy Lint Check
- Unit Test
- Integration Test
- API Contract Test
- Schema Compatibility Test
- Migration Test
- Load Test
- Security Check
- Dependency Check
- Regression Test

Example Commands:

```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all
cargo audit
```

AIが生成したCodeをそのまま本番へ入れず、検証を通してから反映することで、出戻りといたちごっこを減らします。

---

## 推奨Repository構成

```text
opencosmo/
├── README-English.md
├── README-Japan.md
├── Cargo.toml
├── crates/
│   ├── opencosmo-router/
│   ├── opencosmo-federation/
│   ├── opencosmo-schema-registry/
│   ├── opencosmo-ai-routing/
│   ├── opencosmo-versionless-api/
│   ├── opencosmo-db/
│   ├── opencosmo-backup/
│   ├── opencosmo-history/
│   ├── opencosmo-observability/
│   └── opencosmo-security/
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

## 技術Stack

Core:

- Rust
- Poem
- Tokio
- Serde
- SQLx

Database:

- PostgreSQL
- aruaru-db
- Local開発用SQLite
- Redis互換Cache

Federation:

- GraphQL
- gRPC
- OpenAPI Compatibility Layer

AI:

- OpenAI-compatible API
- Anthropic-compatible API
- Gemini-compatible API
- DeepSeek-compatible API
- Local LLM Runtime Integration

Observability:

- OpenTelemetry
- Prometheus
- Grafana

Infrastructure:

- Docker
- Kubernetes
- VPS
- Bare Metal

---

## Rust + Poem を採用する理由

Rustの利点:

- Memory Safety
- Thread Safety
- High Performance
- Low Runtime Overhead
- Strong Type System
- Infrastructure Software向けの高い信頼性

Poemの利点:

- Modern Rust Web Framework
- Flexible Routing
- Middleware Support
- Async Performance
- Clean API Structure

Rust + Poem は、高速・安全・保守しやすい Gateway を作るのに適しています。

---

## Development Roadmap

### Phase 1: Core Foundation

- Rust Workspace構築
- Poem-based HTTP Router
- Basic Health Check
- Configuration Loader
- PostgreSQL Connection
- Logging / Tracing
- Quality Gate Setup

### Phase 2: Federation Core

- Schema Registry
- Schema Validation
- Federation Composition
- Query Planning
- Router Execution Model

### Phase 3: VersionlessAPI and DB Layer

- VersionlessAPI Compatibility Rules
- PostgreSQL Integration
- aruaru-db Interface
- Migration Tracking
- Git-like Schema History

### Phase 4: AI Native Layer

- AI Provider Registry
- AI Routing Rules
- Local LLM Routing
- Cost / Latency Tracking
- Fallback System

### Phase 5: Production Platform

- Distributed Backup
- Observability Dashboard
- Security Hardening
- Load Testing
- Multi-node Deployment
- Kubernetes Support

---

## Project Status

OpenCosmo は現在、設計・初期開発段階です。

最初の目標は、Rust + Poem による最小構成かつ高品質な Gateway Foundation を作り、その後 Federation、VersionlessAPI、AI Routing、Database History、Distributed Backup、Quality Gate を順番に追加することです。

---

## License

License は以下を予定しています。

- Apache License 2.0
- MIT License
- Dual MIT / Apache 2.0

最終決定は TBD です。

---

## Disclaimer

OpenCosmo は、現代的な Graph Federation と API Gateway Architecture に着想を得た独立した実験的Projectです。WunderGraph Cosmo の公式Projectではありません。
