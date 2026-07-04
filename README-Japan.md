# open-runo

## Rust + Poem で WunderGraph Cosmo を超える — Graph Federation Platform

open-runo は、**Rust + Poem** で開発する、次世代の Graph Federation・API統合・AI ネイティブ Gateway プラットフォームです。

---

## なぜ open-runo を作るのか

### REST API が引き起こす問題

現代のマイクロサービス開発では、REST API の乱立が深刻な問題を生みます。

- **BFF（Backend For Frontend）地獄** — 画面変更のたびに専用 API を作り直す無限ループ
- **オーバーフェッチ / N+1 問題** — 必要なデータを取るために不要なデータまで取得、または何度もリクエストが必要
- **API ドキュメントの形骸化** — Swagger は実装と乖離し、誰も信用しなくなる
- **エンドポイントの乱立** — 認証・セキュリティ・Rate Limit の管理が崩壊する
- **バージョン爆発** — `/v1` `/v2` `/v3` が永遠に積み重なる

### WunderGraph Cosmo（Go製）が解決しようとしていること

WunderGraph Cosmo は **GraphQL Federation** でこれらの問題を解決する優れたプラットフォームです。複数の REST API / gRPC / GraphQL サービスを1つの GraphQL エンドポイントに統合し、BFF 開発を不要にします。

**しかし、Go 製であるがゆえの限界があります：**

| 課題 | 詳細 |
|------|------|
| GC（ガベージコレクション）| 本番環境で突発的なレイテンシスパイクが発生する |
| メモリ安全性 | コンパイル時に保証されず、実行時バグのリスクが残る |
| AI Native 機能なし | LLM ルーティング・AI Gateway の概念が存在しない |
| VersionlessAPI なし | バージョン乱立の根本問題は未解決 |
| DUAL DATABASE なし | 分散 DB 戦略・aruaru-db 統合ができない |
| Desktop App なし | 管理 UI は Web のみ |

### REST API vs Cosmo vs open-runo — 機能差分表

| 課題・機能 | REST API のみ | Cosmo (Go) | **open-runo (Rust)** |
|---|:---:|:---:|:---:|
| BFF 開発が必要 | ❌ 毎回必要 | ✅ 不要 | ✅ 不要 |
| オーバーフェッチ / N+1 | ❌ 発生する | ✅ 解消 | ✅ 解消 |
| API ドキュメント形骸化 | ❌ Swagger がずれる | ✅ Schema 管理 | ✅ Schema + Git履歴 |
| エンドポイント乱立 | ❌ バラバラ | ✅ 1エンドポイント | ✅ 1エンドポイント |
| 認証・Security 一元管理 | ❌ 個別実装 | ✅ Gateway で一元化 | ✅ Gateway で一元化 |
| バージョン爆発 (/v1 /v2…) | ❌ 永遠に積み重なる | ⚠️ 根本解決なし | ✅ **VersionlessAPI** |
| GC レイテンシスパイク | ⚠️ 実装依存 | ❌ Go GC が発生 | ✅ **GC なし** |
| コンパイル時メモリ安全 | ⚠️ 言語依存 | ❌ 実行時リスクあり | ✅ **Rust が静的保証** |
| AI Native LLM ルーティング | ❌ なし | ❌ なし | ✅ **自動選択** |
| DUAL DATABASE (PG + aruaru-db) | ❌ なし | ❌ なし | ✅ **対応** |
| Git型 Schema / DB 変更履歴 | ❌ なし | ❌ なし | ✅ **Commit / Rollback** |
| 分散自動バックアップ | ❌ なし | ❌ なし | ✅ **S3 / VPS / Peer** |
| Tauri Desktop App | ❌ なし | ❌ なし | ✅ **TS + Bootstrap 5** |

詳細は [`docs/why-open-runo.md`](docs/why-open-runo.md) を参照してください。

---

## プロジェクトビジョン

open-runo は、以下を統合した次世代インフラ基盤を目指します。

- Graph Federation Gateway（Cosmo 同等・Rust 高速）
- API Gateway（認証・Rate Limit・Security 一元管理）
- Schema Registry（Git型履歴・ステージ昇格）
- AI Gateway（Cloud LLM + Local LLM 自動ルーティング）
- 分散 DUAL DATABASE 連携（PostgreSQL + aruaru-db）
- VersionlessAPI Platform（バージョンなし長期互換運用）
- Observability Platform（OpenTelemetry / Prometheus / Grafana）
- Quality Gate System（Rust コンパイル時検証 + CI ゲート）
- Tauri Desktop App（TypeScript + HTML5 + Bootstrap 5）

単なるルーターではなく、**REST API 問題を根本解決する AI ネイティブ開発・運用プラットフォーム**を目指します。

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

Gateway Router は open-runo の高速入口です。

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

open-runo は API Version の乱立を防ぐために VersionlessAPI を採用します。

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

open-runo は AI Native Platform として設計します。

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

open-runo は単なるDATABASE Wrapperではありません。複数のDATABASE戦略を統合・調整するLayerを持ちます。

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

open-runo は、Application Data、Schema Data、Configuration Data、Metadata を対象にした分散Backup戦略を持ちます。

Backup対象:

- Local Storage
- Remote VPS
- S3互換Object Storage
- 別open-runo Node
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

open-runo は Schema、DATABASE変更、Configuration を Git のように履歴管理することを目指します。

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

open-runo は最初から本番運用レベルの監視を重視します。

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

Security は open-runo の中核設計です。

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

open-runo は、些細なミス削減、BUG削減、品質ゲート強化を重視します。

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

## 技術Stack

Backend (Gateway):

- Rust
- Poem
- Tokio
- Serde
- SQLx

Desktop App (Tauri):

- Tauri 2.x（Rust バックエンド + WebView フロントエンド）
- TypeScript（型安全 UI・API クライアント）
- HTML5 + CSS3 + Bootstrap 5（フレームワーク不使用の Pure TS SPA）
- Vite（ビルドツール）

Database (DUAL):

- PostgreSQL（主DB）
- aruaru-db（独自設計 DB）
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

open-runo は現在、設計・初期開発段階です。

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

open-runo は、現代的な Graph Federation と API Gateway Architecture に着想を得た独立した実験的Projectです。WunderGraph Cosmo の公式Projectではありません。
