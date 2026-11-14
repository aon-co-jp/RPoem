# PORTING.md — RPoem お引越しファイル

> このファイル 1 枚で、**どのプロジェクトでも RPoem を導入・移設**できます。
> open-e-gov / OpenRedmine / OpenWordPress など新プロジェクトのリポジトリに
> このファイルをコピーして、上から順に進めてください。
>
> 対象バージョン: workspace 0.1.0（18 クレート / 335 テスト、--all-features で353）
> 最終更新: 2026-08-20(自動アップデート機構`self_update.rs`追加を反映)

## REST 全廃の状況(2026-11-01 時点、最新) — 管理操作は gRPC

**`/api/` 配下の REST はコード上すべて削除済み**。管理操作は **gRPC サービス `open_runo.admin.v1.Admin`**(定義は
`crates/open-runo-proto/proto/open_runo/admin/v1/admin.proto`)で、**同じ定義を gRPC-web・Connect でも受ける**。GraphQL は Federation の問い合わせだけ。
残る HTTP は `/health`・ACME・`/mcp`・SCIM(`/scim/v2/*`)・`/internal/db/mutate`・静的配信のみ。**このファイルの旧記述(`/api/…` や GraphQL `admin` を叩く例)は gRPC に読み替えること。**

- 起動: `cargo run -p open-runo-gateway`(管理画面・gRPC・GraphQL を同じポート 8080 で。`open-runo-router` 単体は管理サービスを持たない)。素のgRPCの別ポートは `OPEN_RUNO_GRPC_BIND_ADDR`(TLS1.3/mTLS 用)。
- 呼び方: `open-runo-cli`(素のgRPC、`--endpoint http://127.0.0.1:8080`)、管理画面(gRPC-web/Connect のハイブリッド、`RPC Inspector` で通信を確認)、
  `curl`(Connect JSON): `curl -X POST http://localhost:8080/open_runo.admin.v1.Admin/SelfIssueKey -H 'content-type: application/json' -d '{}'`。
- 管理画面の WASM は `www/pkg/` が git 管理外: `apps/desktop-wasm` で `cargo build --release --target wasm32-unknown-unknown` →
  `wasm-bindgen target/wasm32-unknown-unknown/release/open_runo_desktop_wasm.wasm --target web --out-dir www/pkg --no-typescript`(CLI のバージョンは Cargo.lock の wasm-bindgen と一致)。
  静的ファイルは ETag 再検証なので、作り直せば自動で配られる。
- 本番(`OPEN_RUNO_ENV=production`)は gRPC に TLS1.3 必須(+任意で mTLS)、GraphQL は登録済みクエリのみ・深さ/複雑さ上限・イントロスペクション停止・エイリアス/バッチ対策。設定の一覧と理由は `CLAUDE.md` の最新の節。
- 呼び出し側 `open-web-server` は、専用ブランチ `rest-to-graphql` で Connect(HTTP POST+JSON)に切替済み(未 push・未マージ)。**前面の open-web-server は HTTP/1.1 のみ**なので素のgRPC(HTTP/2)は通らない(Connect/gRPC-web は通る)。
- VPS: ゲートウェイは未稼働(`rpoem-web` は紹介ページ)。反映は `git pull` まで。起動手順と注意は `CLAUDE.md`。

## ブラウザ内AI実行(WASM+WebGPU)構想 — 技術検証済み・段階的導入計画(2026-08-25追記)

`experiments/wgpu-browser-spike/`に、`wgpu`(features=`webgpu`+`wgsl`)を
`wasm32-unknown-unknown --release`向けにコンパイルできることを確認した
最小スパイクを配置済み(`cargo check`成功、実ブラウザでの完全なE2E実行は
未検証)。ブラウザは既にW3C勧告候補の`WebGPU` APIを実装済みのため、
`wasi:webgpu`(非ブラウザ実行環境向けの別提案)の成熟を待つ必要はない。
段階的導入計画(第1段階: aruaru-llmのCPU推論をwasm32ビルド+RPoemの
`apps/desktop-wasm`でホスト+open-englishから読み込み、第2段階: WebGPU
経由のGPU推論)の詳細は`CLAUDE.md`の2026-08-25エントリを参照。

## 自動アップデート機構(2026-08-20新設、`crates/open-runo-router/src/self_update.rs`)

起動時にGitHub Releases API(`aon-co-jp/RPoem`)で最新タグを確認し、
現バージョンより新しければダウンロード→安全な自己置換(ヘルスチェック
成功時のみ切替、失敗時は`.bak`からロールバックし旧プロセスは継続稼働)
を行う。**既定オフ**——`OPEN_RUNO_ROUTER_SELF_UPDATE=true`を明示設定
した場合のみ動作する(VPS本番運用との予期しない競合を避けるため)。
Linux(x86_64/aarch64)・Windows(x86_64)のみ対応。詳細・未検証事項は
`self_update.rs`モジュールdoc、および本リポジトリの`CLAUDE.md`
2026-08-20エントリを参照。他プロジェクトへ移植する場合は
`Cargo.toml`の`reqwest`が非optional依存になっている点に注意
(GitHub Releases APIへの常時稼働HTTPクライアントとして必要)。

---

## 1. RPoem とは（30 秒版）

**RPoem** は、以下の3要素をすべて外部パッケージに依存せず
自前で一から開発・再現し、その連携をスムーズに行うことで WEB サイト/
WEB アプリ開発を効率的に行えるようにする**フレームワーク/ミドルウェア**
です(2026-07-11、ユーザーによる最終定義)。

1. **cosmo部分**（open-cosmo と共通のコア）: WunderGraph Cosmo 有料版
   （Launch/Scale/Enterprise）の機能を、Cosmo自体には依存せず
   Rust + tokio/hyper で自前再実装した OSS 版。**REST API不要**
   （VersionlessAPI/GraphQL Federation）・**契約不要**（商用ライセンス
   なしで同等機能）・**独自AI搭載のWeb高速化**（`CachePredictor`の
   自己学習型HTMLキャッシュ）・Tauri互換のフロントエンド体験、を含む。
2. **poem部分**（バックエンド、本リポジトリ固有）: Rust の Poem
   フレームワークの**全機能を完全互換で一から自作・再現**（`poem`
   パッケージへの直接依存なし。到達度は `docs/poem-parity.md` 参照）。
3. **tauri部分**（フロントエンド、本リポジトリ固有）: Tauri の**全機能を
   完全互換で一から自作・再現**（`tauri`パッケージへの直接依存なし、
   現状 Rust→WASM 実装。到達度は `docs/tauri-parity.md` 参照）。

この3要素（Tauri再現フロントエンド + open-cosmo/cosmoコア + Poem再現
バックエンド）がスムーズに連携し合うこと自体が価値であり、フロントエンド
開発・バックエンド開発・Web中心的な開発の連携を円滑にすることが本
フレームワークの目的です。「連携」とはいえ、各部分は外部フレームワーク
との統合ではなく、そのフレームワーク自体の完全な自前再実装である点に
注意してください。

以下、実装本体である Rust + tokio/hyper 製 **GraphQL Federation
プラットフォーム / Web フレームワーク**部分（cosmo部分）の詳細です。
さらに独自の自己学習 AI（外部 LLM 契約不要）を搭載します。

| 分類 | 提供機能 |
|------|----------|
| GraphQL | Federation 合成・破壊的変更検出・`POST /graphql`・Subscriptions (WS)・GraphiQL |
| Cosmo 有料版互換 | Persisted Queries (Trusted Documents)・厳密 RBAC・OIDC SSO・SCIM 2.0 (Users/Groups)・監査ログ・細粒度レートリミット・レスポンスキャッシュ・マルチグラフ namespace |
| 独自 AI（LLM 不要） | HTML ページキャッシュの自動判定/適応 TTL/先回り再生成、API キーの異常検知・自動隔離 |
| 自己運用 | 鍵の自動発行/失効（SCIM 連動）、AI 学習の自動永続化、両 DB 整合性の自動検証・自動修復、二か所自動バックアップ |
| お引越し | エンジン間変換（MySQL→PostgreSQL 等）、SQL/CSV エクスポート（Snowflake）、分散 DB 統合（Federated）、ワンコール復活 |

## 2. 持っていくもの（ファイル一覧）

```
open-runo/
├── Cargo.toml / Cargo.lock      ← workspace 定義（バージョン固定）
├── crates/                      ← 18 クレート（本体）
├── apps/desktop-wasm/            ← Rust→WebAssembly 管理アプリ（任意、open-runo-routerが自前配信）
├── docs/                        ← 設計・API 仕様・migration.md ほか
├── .github/workflows/ci.yml     ← fmt / clippy -D warnings / test
├── Dockerfile / docker-compose.yml / Makefile
└── PORTING.md                   ← 本ファイル
```

丸ごと移設する場合はフォルダごとコピーして `cargo test --workspace`
（256 テストが通れば移設成功）。以下はライブラリとして使う場合です。

## 3. 依存の書き方（新プロジェクトの Cargo.toml）

```toml
[dependencies]
# 同一マシンにある場合（path 依存）
open-runo-core     = { path = "../open-runo/crates/open-runo-core" }
open-runo-router   = { path = "../open-runo/crates/open-runo-router" }
open-runo-gateway  = { path = "../open-runo/crates/open-runo-gateway" }
open-runo-db       = { path = "../open-runo/crates/open-runo-db" }
open-runo-cache    = { path = "../open-runo/crates/open-runo-cache" }
open-runo-security = { path = "../open-runo/crates/open-runo-security" }

# GitHub 公開後は git 依存でも可
# open-runo-router = { git = "https://github.com/aon-co-jp/RPoem" }

tokio = { version = "1.40", features = ["full"] }
hyper = { version = "1.10", features = ["full"] }
# poem は不要(open-runo-routerは4.1のようにtokio/hyperで直接動く)。
# GraphQL Subscriptions(WebSocket)が必要な場合のみ追加:
# poem = { version = "3.1", features = ["sse"] }
```

必要なものだけ選べます（各クレートは独立してテスト可能）。
DB エンジンは feature で選択: `open-runo-db = { ..., features = ["dual"] }`
（`postgres` / `mysql` / `sqlite` / `aruaru` / `cockroach` / `yugabyte` /
`mongodb` / `redis` / `clickhouse` / 複合: `dual` `single-pg` `single-my`
`dev` `full` `cluster`）。

## 4. 組み込みレシピ

### 4.1 フルスタック（REST + GraphQL + 自己運用ぜんぶ）

`open-runo-gateway`の`main.rs`をそのまま流用するのが最速です(poem非依存、
tokio/hyperベース):

```rust
use open_runo_core::Config;
use open_runo_router::{build_hyper_app, hyper_compat, state::AppState};
use std::sync::Arc;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = Config::from_env()?;
    open_runo_observability::init_tracing(&config.log_level);

    let state = Arc::new(AppState::new()); // 本番: AppState::with_db(...)
    let (graphiql, graphql_post) =
        open_runo_gateway::graphql_hyper::graphql_handlers(Arc::clone(&state));

    let app = build_hyper_app(
        state,
        config.rate_limit_max_requests,
        config.rate_limit_window_secs as i64,
    )
    .route(hyper::Method::GET, "/graphql", graphiql)
    .route(hyper::Method::POST, "/graphql", graphql_post);

    let addr = config.bind_addr.parse()?;
    let (_, handle) = hyper_compat::serve(app, addr).await?;
    handle.await?;
    Ok(())
}
```

GraphQL Subscriptions（WebSocket）が必要な場合のみ、代わりに
`open_runo_gateway::graphql_route(state)`（poem版、`/graphql/ws`対応）を
使ってください — その場合`poem`への依存が復活します。

これだけで、認証（KeyGuardian 自動運用）・RBAC・OIDC・SCIM・監査ログ・
AI モデル自動永続化・整合性自動修復・定期バックアップが**全部背景で動きます**。

### 4.2 自分のページに AI HTML キャッシュだけ載せる（OpenWordPress 型）

```rust
use open_runo_router::middleware::html_cache::{
    HtmlCacheConfig, HtmlCacheMiddleware, HtmlPageCache,
};
use poem::{get, handler, EndpointExt, Route};
use std::sync::Arc;

#[handler]
async fn render_article(/* db から記事を取って HTML を返す */) -> String {
    /* 重いレンダリング */ String::new()
}

let cache = Arc::new(HtmlPageCache::new(HtmlCacheConfig::from_env()));

let app = Route::new()
    .at("/article/:id", get(render_article))
    .data(Arc::clone(&cache))          // 更新ハンドラから purge するため
    .with(HtmlCacheMiddleware(cache)); // ← これだけで AI キャッシュ有効
```

記事更新時は `cache.purge("/article/123").await;` を 1 行呼ぶだけ。
purge は AI の学習信号にもなり、更新の多いページは TTL が自動短縮されます。
個人化部分（ようこそ○○さん等）は `Cookie` 付きリクエストが自動バイパス
されるため、共通 HTML + JS Fetch の分離パターンでどうぞ。

### 4.3 単一 DB で始めて後から DUAL に育てる

```rust
use open_runo_db::dual::DualBackend;
use open_runo_router::state::AppState;

// 開発: SQLite 1 台でも DUAL と同じコードパス
let state = AppState::with_single_db(Arc::new(sqlite_backend));

// 本番: PostgreSQL + aruaru-db の二重化（自動整合性修復つき）
let dual = DualBackend::with_default_routing(postgres, aruaru);
let state = AppState::with_db(Arc::new(dual));
```

### 4.4 社内の分散 DB を 1 つに統合（Federated）

```rust
use open_runo_db::federated::FederatedBackend;

let fed = FederatedBackend::builder()
    .member("tokyo-pg", tokyo_postgres)
    .member("osaka-my", osaka_mysql)
    .route("orders", "osaka-my")
    .broadcast("schemas")
    .default_member("tokyo-pg")
    .build()?;
let state = AppState::with_db(Arc::new(fed));
```

段階的な片寄せは `open_runo_db::migrate::transfer_verified(src, dst, tables)`。

## 5. データのお引越し（既存環境から）

```bash
# 旧環境: 全 DATA + AI 学習記録をポータブル JSON へ（二か所に書き込み）
curl -X POST -H "x-api-key: $KEY" http://old:8080/api/backup/export

# 新環境: ファイル指定で取り込み、または最新を自動発見して一発復活
curl -X POST -H "x-api-key: $KEY" -d '{"path":"..."}' http://new:8080/api/backup/import
curl -X POST -H "x-api-key: $KEY" http://new:8080/api/backup/restore-latest
```

異種エンジンへの変換・Snowflake 取り込み・無停止統合は `docs/migration.md` 参照。

## 6. 環境変数 全一覧

| 変数 | 既定 | 説明 |
|------|------|------|
| `OPEN_RUNO_BIND_ADDR` ほか Config 系 | — | `open-runo-core::Config` 参照 |
| `OPEN_RUNO_JWT_SECRET` | 無効 | HS256 Bearer 認証 |
| `OPEN_RUNO_OIDC_ISSUER` / `_OIDC_JWKS_FILE` / `_OIDC_AUDIENCE` | 無効 | OIDC SSO (JWKS/RS256) |
| `OPEN_RUNO_RBAC` | off | `enforce` でルート単位 RBAC |
| `OPEN_RUNO_SCIM_TOKEN` | 無効 | IdP 用固定 Bearer（/scim/* 限定） |
| `OPEN_RUNO_KEY_ANOMALY_FACTOR` / `_KEY_WARMUP_REQUESTS` / `_KEY_COOLDOWN_SECS` | 20 / 50 / 300 | KeyGuardian 異常検知 |
| `OPEN_RUNO_PQ_MODE` | allow | disabled / allow (APQ) / enforce (Trusted Documents) |
| `OPEN_RUNO_CACHE` / `_CACHE_TTL_SECS` | off / 30 | GraphQL レスポンスキャッシュ |
| `OPEN_RUNO_HTML_CACHE` | off | AI HTML ページキャッシュ（`on` で有効） |
| `OPEN_RUNO_HTML_CACHE_TTL_SECS` / `_MAX_ENTRIES` / `_MIN_HITS` | 60 / 10000 / 2 | 同上の基本設定 |
| `OPEN_RUNO_HTML_CACHE_AI` | on | `off` で固定 min-hits 判定へ |
| `OPEN_RUNO_HTML_CACHE_REFRESH_RATIO` | 0.8 | 先回り再生成の閾値 |
| `OPEN_RUNO_CACHE_AI_MIN_TTL_SECS` / `_MAX_TTL_SECS` / `_DEFAULT_TTL_SECS` / `_MAX_TRACKED` / `_MIN_EXPECTED_HITS` | 5 / 3600 / 60 / 50000 / 1.0 | AI 予測器チューニング |
| `OPEN_RUNO_AI_PERSIST_SECS` | 300 | AI 学習の自動保存（0=off） |
| `OPEN_RUNO_INTEGRITY_SECS` | 3600 | 両 DB 整合性の自動検証・修復（0=off） |
| `OPEN_RUNO_BACKUP_DIR` | ./backups | 一次バックアップ先 |
| `OPEN_RUNO_BACKUP_MIRROR_DIR` | 無効 | 二次（Google Drive 同期フォルダ推奨） |
| `OPEN_RUNO_BACKUP_SECS` | 0 | 定期バックアップ間隔（0=手動） |

## 7. REST サーフェス早見表(**旧記述: 2026-11-01 に REST は削除済み。GraphQL に読み替えること**)

| Path | 用途 |
|------|------|
| `/health` `/healthz` | 認証不要ヘルスチェック |
| `/api/openapi.json` | REST APIのOpenAPI 3.0仕様(認証不要、Postman/Insomnia/Swagger UIへインポート可) |
| `/.well-known/acme-challenge/:token` | ACME HTTP-01チャレンジ応答(認証不要、CAが直接アクセスする) |
| `/api/keys/self-issue` | **認証不要**でAPIキーを自動発行(developer role、24時間有効)。人間がキーを意識しない設計の起点 |
| `/mcp` | MCP(Model Context Protocol)Streamable HTTP transport(認証不要、JSON-RPC 2.0)。`initialize`/`tools/list`/`tools/call`/`resources/list`/`resources/read`/`prompts/list`/`prompts/get`に対応。実ツール2種(`health_check`・`self_issue_api_key`)+実リソース2種(`openapi://spec`・`health://status`)+実プロンプト1種(`summarize_api`、同じ`openapi::spec()`を動的にレンダリング)、いずれも既存REST APIと同じ本番ロジックを共有 |
| `/api/session/login` `/logout` | `X-Api-Key`を追加のセッションCookie(HttpOnly+SameSite=Strict)+CSRFトークンへ交換(置き換えではなく追加の認証経路) |
| `/graphql` (+`/graphql/ws`) | Federation GraphQL / Subscriptions(GraphiQLは`GET /graphql`) |
| `/api/schemas*`（`?namespace=`） | Schema Registry（マルチグラフ対応） |
| `/api/schemas/upload` | `multipart/form-data`でSDLファイルを直接アップロードして登録(`/api/schemas`のJSON版と同じ登録処理を共有) |
| `/api/federation/*` | 合成・状態 |
| `/api/persisted-queries*` | Trusted Documents 登録・取得 |
| `/api/db/*` | DUAL DATABASE KV |
| `/api/db/:table/:key/at/:commit_id` | VersionLessAPI + Git-on-SQL ハイブリッドの読み出し側(2026-07-14、open-cosmoからミラー) — 指定コミット時点の値を取得(`aruaru-db`バックエンドのみ実対応、他バックエンドは501) |
| `/api/ai/route` | AI プロバイダ選択 |
| `/api/events` | SSE |
| `/api/ws-echo` | 汎用 WebSocket エコー（手書き RFC 6455、認証不要） |
| `/api/ws-events` | 汎用 WebSocket イベント配信（認証必須、SSE と同じブローカー） |
| `/api/feature-flags*` `/api/feature-flags/:name` `/api/feature-flags/:name/evaluate` | Feature Flags（upsert/list/get/delete/evaluate、決定的バケッティングによる canary ロールアウト） |
| `/scim/v2/Users` `/scim/v2/Groups` | SCIM 2.0（鍵の自動発行/失効つき） |
| `/api/cache/purge` `/purge-all` `/ai-stats` | HTML キャッシュ管理・AI 観測 |
| `/api/backup/export` `/import` `/restore-latest` | バックアップ・復活 |
| `/api/migrate/export-sql` `/export-csv` | エンジン変換エクスポート |
| `/api/integrity/check` | 両 DB 整合性チェック・自動修復 |
| `/api/analytics/requests-per-month` | 月間リクエスト数（運用メトリクスのみ、課金/レート制限には不使用） |
| `/api/analytics/operations` | オペレーション別（method+path）レイテンシ・エラー率集計 |

Federation合成（`POST /api/federation/compose`）は`sdl`フィールドで生の
GraphQL SDL（Federation v1の暗黙ディレクティブ方式・v2の`@link`方式
どちらも自動判別）を渡せます。従来の`{service_name, types: {...}}`
JSON直接指定も後方互換のためそのまま利用可能です。

認証: `X-Api-Key`（KeyGuardian 台帳が空なら開発モードで任意値可）/
JWT / OIDC Bearer / セッションCookie(`/api/session/login`発行、状態変更
リクエストは`X-CSRF-Token`ヘッダ必須)。RBAC 有効時、管理系は `admin`
ロール必須。

TLS を直接終端したい場合（リバースプロキシを使わない構成）は
`tls` Cargo feature（既定オフ）で `hyper_compat::tls::{load_tls_config,
serve_tls}` が使えます。証明書を自動取得したい場合は`acme` feature
（`tls`を暗黙有効化）で`acme::{AcmeClient, obtain_certificate_http01,
tls_alpn01::obtain_certificate, dns01::obtain_certificate}`（RFC 8555、
HTTP-01・TLS-ALPN-01・DNS-01の3チャレンジ型すべてに対応）が使えます。
DNS-01は`acme::dns01::DnsProvider` traitを実装すれば任意のDNSプロバイダに
対応可能（付属の`CloudflareDnsProvider`はbearerトークンのみ・署名不要な
最小実装例）。gRPCが必要な場合は`grpc::serve_grpc`（新規依存無し、
`grpc.health.v1.Health/Check`・`/Watch`ストリーミング・
`grpc.reflection.v1.ServerReflection`の`list_services`）を
`OPEN_RUNO_GRPC_BIND_ADDR`で有効化。ネイティブのシステムトレイ常駐アプリ・
OSネイティブ通知・Windows インストーラーが必要な場合は `apps/desktop-tray`
（`tauri`パッケージ非依存）をそのまま移植できます。

## 8. 動作確認

```bash
cd open-runo
cargo test --workspace     # 296 テスト(--all-features で305) + doctest が全部通れば OK
cargo run -p open-runo-gateway   # REST + GraphQL 統合バイナリ起動
```

## 9. 命名規約（お引越し先でも守ること）

- クレート/ディレクトリ: `open-runo-*`　- Rust パス: `open_runo_*`
- 環境変数: `OPEN_RUNO_*`　- 型名のみ CamelCase: `OpenRuno*`（Rust 言語制約）

## 10. 詳細ドキュメント

`docs/HANDOFF.md`（全開発履歴）/ `architecture.md` / `cosmo-parity.md` /
`migration.md` / `api-spec.md` / `database.md` / `security.md` / `federation.md`

## 11. 移植・拡張時の注意

他プロジェクトへ移植・拡張する際、依存クレートの現状（バージョン・
メンテナンス状況）や仕様の詳細で判断に迷う場合は、学習データからの
推測だけに頼らず、実際に検索（日本語・英語の両方の Google 検索、
および GitHub 上の実装状況調査）して裏付けを取ってから技術選定を
行うことを推奨する。

**パフォーマンス・並行処理**: 移植先でも、非同期（tokio マルチスレッド
ランタイム）を基本としつつ、必要な場面のみ同期処理を用いる方針を
踏襲すること。`#[tokio::main]` のランタイム flavor が
`current_thread` に固定されていないか、async 関数内でブロッキング
I/O・CPU 負荷処理を直接呼んでいないか（`tokio::task::spawn_blocking`
へ退避すべき）、CPU 律速な処理は `rayon` 等でのデータ並列化を検討する
価値があるか、を移植時にも確認するとよい。

**最先端追従の方針(2026-07-23追記)**: 移植時の日英Web検索で
CockroachDB/TiKV等の最先端の実運用システムが既に対応済みと判明した
設計ギャップが見つかった場合、「今のところは大丈夫」という報告に
留めず、確認を求めず自動でそのまま実装に着手すること(ユーザー指示、
正本は`open-raid-z/CLAUDE.md`・`PORTING.md`同日エントリ参照)。

**送信側だけあって受信側が無い、という欠落の見つけ方(2026-07-23、
UDP-IP冗長経路で実際に発見した教訓)**: 「Aが呼ぶエンドポイント/経路を
Bが実装しているはず」という設計は、**Bを実際にgrepして存在確認する
まで信用しないこと**。`open-web-server-ledger::Ledger`が
`POST /internal/db/mutate`をフォワードする設計だったが、RPoem側に
このエンドポイントが2026-07-23まで一つも存在しなかった
(`grep -rn "internal/db/mutate" crates/`で0件だったことで発覚)。
同様にUDP冗長経路も送信側のみ実装され受信側が存在しなかった。移植・
統合作業では「送信元のコードだけでなく、受信先に対応する実装が
本当に存在するか」を必ずgrepで裏取りすること。

**Apache/Tomcat互換の多言語アプリケーションサーバー対応(2026-07-23、
ユーザー指示、正本は`open-raid-z/CLAUDE.md`同名節参照)**: このリポジトリを
他プロジェクトへ移植・拡張する際、「open-web-serverをJavaのApacheのように、
RPoemをTomcatのように——Java/Ruby on Rails/PHP+Laravel/Python+FastAPI等
言語を問わず連携できる汎用性を高める」という目標を踏襲すること。移植先で
このリポジトリの`open-runo-appserver`(第二のTomcat骨格)を使う場合、
プレーンHTTPで応答する任意の言語・フレームワークのアプリケーションサーバー
(Rustに限らない)を`tenant_bridge`/`SharedDispatcher`経由でそのまま
指せる設計を維持すること——特定言語向けの専用実装を持ち込まない
(HTTPで統一する既存方針からブレないこと)。

## 12. RPoem ⇔ RCosmo の「Cosmo共通コア」重複整理(2026-07-23)

RCosmoは`docs/HANDOFF.md`の通りRPoemと共通のCosmoコア(Federation・
VersionlessAPI・SCIM・Security・Cache等)を持つ姉妹リポジトリ。今回、
実際に両リポジトリの`crates/`配下をファイル単位で`diff`した結果を記録する。

- **調査結果**: 共通20クレートのうち **18クレートが両リポジトリで
  byte-for-byte完全一致**(`open-runo-ai-routing`/`api-types`/
  `appserver`/`backup`/`cache`/`cli`/`core`/`feature-flags`/
  `federation`/`history`/`observability`/`persisted-queries`/
  `rustjson`/`schema-registry`/`scim`/`security`/`versionless-api`/
  `view`)。残る`open-runo-db`/`open-runo-gateway`/`open-runo-router`の
  3クレートは、RPoem固有の追加(`appserver_tenants.rs`・`udp_notice.rs`・
  gRPC/ACME/MCP拡張等)により意図的に分岐している——これらは
  「共通コア」ではなくRPoem固有スコープ(第二のTomcat・Poem完全再現)の
  範囲に属するため分岐は正常。
- **既存パターンの踏襲を確認**: このエコシステムの「別リポジトリの
  crateへ直接のCargo依存はしない、小さなモジュールとして直接コピーする」
  という既存方針(RustJSON移植等)通り、両リポジトリは同じディレクトリ
  ツリー(`F:\runo\RPoem`/`F:\runo\RCosmo`)に配置されており path依存も
  技術的には可能だが、CI環境でのpath依存問題を避けるため、**今回も
  コピー移植パターンを継続する判断とした**(ワークスペース統合は
  スコープが異なる別プロジェクトという既存方針に反するため不採用)。
- **新規: `scripts/sync-cosmo-core.sh`**(両リポジトリに同一ファイルを配置)。
  従来「手作業でファイルをコピーしてミラーする」運用だった同期作業を
  機械的に検証・実行できるようにした:
  - `scripts/sync-cosmo-core.sh check` — 共通コア18クレートの同期状態を
    一覧表示(diff -rq ベース、Cargo依存追加なし)。
  - `scripts/sync-cosmo-core.sh diff <crate>` — 指定クレートの差分を表示。
  - `scripts/sync-cosmo-core.sh push <crate>` / `pull <crate>` — 片方向へ
    `src/`をまるごとコピー(コピー後は`git diff`で確認し
    `cargo test`してからコミットすることを想定、自動コミットはしない)。
  - 実際に使用: 本セッションで発見した`open-runo-appserver`の
    Windows非互換テストバグ修正(後述)をこのスクリプトの`push`で
    RCosmo側へ同期し、`check`で0件drift(18/18 in sync)を確認済み。

## 13. GitHub `open-runo`(旧`open-cosmo`)からの「救済」調査(2026-07-24、結論: 移植なし)

ユーザーから「open-runo内にPoem+Cosmo+Tauri+WEB高速化のプログラムが
残っていれば、本来RPoemの物なので救済(移植)してほしい。ただし
open-runoがWunderGraph Cosmo有料版とWEB高速化中心の狭いプロジェクト
であれば、そのまま続けてよい」という条件分岐の指示を受けた。

`F:\runo\open-runo`(新規クローン)の中身を確認したところ、
`docs/poem-parity.md`・`docs/tauri-parity.md`・`apps/desktop-tray`・
`apps/desktop-wasm`・約20個の`open-runo-*`クレートが実在し、狭い
プロジェクトではなく広範な実装を持っていたため、11節・12節の
RPoem⇔RCosmo比較と同じ手順(`diff -rq`)で、今度はRPoem⇔open-runo間
を比較した。

- **`apps/desktop-tray`・`apps/desktop-wasm`(Tauri相当のデスクトップ
  アプリ体験)**: 両リポジトリで完全一致。差分は`target/`・
  `installer/dist`・`www/pkg`のようなビルド成果物のみで、ソース
  コードに差は無かった。ユーザーが最優先で確認を求めた項目だったが、
  RPoem側に既に同一のものが存在しており救済不要と判明。
- **`crates/`(共通20クレート)**: 17クレートが完全一致。差分のあった
  `open-runo-appserver`/`open-runo-db`/`open-runo-gateway`/
  `open-runo-router`/`open-runo-rustjson`の5クレートは、**すべて
  RPoem側がopen-runo側を上回る内容**(RPoemのみに存在する
  `appserver_tenants.rs`・`udp_notice.rs`・`router/tests/`、
  `open-runo-db`の`put_versioned`実装、行数もRPoem側が常に多い)。
  コメント中の実名・日付入り開発メモがopen-runo側では汎化・簡略化
  されている痕跡から、open-runoはRPoemから派生・エクスポートされた
  スナップショットだと判断した。
- **RPoemにのみ存在するクレート**: `open-runo-poem-compat`・
  `open-runo-poem-compat-macro`(Poem互換ファサード)——open-runo側には
  そもそも存在しない。

**結論**: open-runo側にRPoemより優れている・RPoemに無い実装は
見つからなかったため、**RPoemへの移植は0件**(コード変更なし)。
`open-runo`リポジトリ自体の扱い(削除・アーカイブ等)はユーザー指示
通りスコープ外としており、本セッションでは一切操作していない。
将来open-runo側で独自コミットが増えた場合は、本節と同じ
`diff -rq crates/<name>`手順で再判定すること。

## 15. `tenant_bridge`の実E2E検証(2026-08-04、初の実証)

`crates/open-runo-appserver/src/tenant_bridge.rs::dispatcher_from_tenants`
(`open-web-server::TenantRegistry`との橋渡し関数)は単体テストのみで
実証されておらず、実際にopen-web-server本体と接続してのE2E検証は
一度も行われていなかった。新規`crates/open-runo-appserver/examples/
e2e_stub_app.rs`(`tenant_bridge`で構築した`Dispatcher`を
`ThreadedProxyServer`に載せ、`X-Served-By: RPoem-appserver`固定応答を
返す最小upstreamの手前に立てる)を作り、open-web-server本体を実起動
→`POST /admin/tenants`で登録→`Host`ヘッダ指定リクエストが実際に
RPoem側まで転送されることを、5回連続で`200`+期待ヘッダにより実証した
(モック無し、実TCP2プロセス間通信)。移植先で同様のブリッジ機構を
検証する際は、この`examples/`ディレクトリのパターン(スタブ
アプリケーションサーバー+実バイナリ2つの同時起動)をそのまま使える。
詳細・初回リクエストの一時的な502(ウォームアップ猶予の要検討課題)は
[CLAUDE.md](CLAUDE.md)の2026-08-04 HANDOFF参照。

## 16. open-web-server⇔RPoem「4層4重」連携の再検証(2026-09-30、破損なし)

「15.」の実証(2026-08-04)から約2ヶ月経過した時点で、`open-web-server`を
Apache、RPoemをTomcatとして多段Webサーバー化する構成が現在も機能して
いるかをユーザーから問われ、`cargo test --workspace`をRPoem・
open-web-server双方で実行して再検証した。

- RPoem: 全クレート・全テスト成功(`ledger_fusion.rs`のクロスリポジトリ
  統合テストを含む)。
- open-web-server: 全クレート・全テスト成功(`tenant_router`・`proxy`の
  接続レース耐性テストを含む)。

自動テストの範囲では2026-08-04時点からのリグレッションは無いことを確認。
`examples/e2e_stub_app.rs`を使った実バイナリ2プロセス同時起動の手動E2E
手順自体は今回未実施(詳細は`CLAUDE.md`の2026-09-30 HANDOFF参照)。

## v1.3.0(2026-10-10): かすれ補正・字形照合・AI文脈補完の拡張 / v1.3.0: faded-text rescue, glyph matching, extended AI context completion

### 日本語
- 新設 **`crates/open-runo-glyph`**(MIT OR Apache-2.0): かすれた升目と候補の字形(fontdue で描画、明朝・ゴシック)を正規化相互相関で照合。升目の全位置を 1 回の行列積にする `windows()` / `GlyphIndex`(`matrix()` を `sgemm` の B に渡し、`reduce()` で最良位置・文字ごとの最大・上位を取る)と、CPU の参照実装 `rank_cpu()`。試験 3 件。
- **`crates/open-runo-pdf-ocr`**: `enhance.rs`(コントラスト自動補正 `normalize_contrast` = 常にオン、かすれ行の超解像+再 OCR `reocr_lines` = 設定 `reocr`)、`complete.rs`(走査 → 疑わしい箇所 → 候補 → 採点 → 決定 → 反映。複数の単語にまたがる置換は結合)、`visual.rs`(升目の切り出しと証拠の強さ)、`client.rs`(`scan` / `propose` / `fill` / `charset` / `glyph_rank`)、設定 `reocr*` / `fill*`(`fill_server_url` で補完だけ別サーバーにできる)。試験 20 件。
- **`crates/open-runo-sr`**: `upscale_crop()`(小さな切り抜きの拡大)を追加。Arm 向け NEON カーネルは v1.1.1。
- 測定結果・試験手順・未了は `pdf-r2l-rs/PORTING.md`(日英)に詳しく記録。補完は実験的・既定オフ。

### English
- New **`crates/open-runo-glyph`** (MIT OR Apache-2.0): matches a faded cell against candidate glyphs (rendered with fontdue, serif and sans) by normalized cross-correlation. `windows()` / `GlyphIndex` turn all positions into one matrix product (pass `matrix()` as B of `sgemm`, then `reduce()` takes the best position, the per-character maximum and the top list), plus the CPU reference `rank_cpu()`. 3 tests.
- **`crates/open-runo-pdf-ocr`**: `enhance.rs` (automatic contrast correction `normalize_contrast` = always on; super-resolution + re-OCR of faded lines `reocr_lines` = setting `reocr`), `complete.rs` (scan → suspect spots → candidates → scoring → decision → apply; replacements across words merge them), `visual.rs` (cell cutting and evidence strength), `client.rs` (`scan` / `propose` / `fill` / `charset` / `glyph_rank`), settings `reocr*` / `fill*` (`fill_server_url` can point only completion at another server). 20 tests.
- **`crates/open-runo-sr`**: added `upscale_crop()` (enlarge small crops). The Arm NEON kernel landed in v1.1.1.
- Measurements, test procedure and open items are recorded in `pdf-r2l-rs/PORTING.md` (JA/EN). Completion is experimental and off by default.

## JSON処理の使用方針: `parse_secure_sonic`(速い型+安全)と`sonic-rs`は使える所では全て使う(2026-10-14、ユーザー指示)
正本は[`RS-JSON`](https://github.com/aon-co-jp/RS-JSON)(`README.md`・`PORTING.md`)。

**方針**: JSONを読む所は、使える所では**全て** `rust-json`(RS-JSON)の `parse_secure_sonic`(`fast`機能、結果は`sonic_rs::Value`)を使う。
`sonic-rs` を直接使える所(型付き構造体への読み込みなど)も同様。使えない所は `parse_secure`(結果は`serde_json::Value`)を使う。
外部から来る信頼できない入力(HTTP本文・ネットワーク越しのファイル・アップロード)は、**必ずサイズ・深さ・重複キーの上限つき**で読む。

**実測(7 MB・4万件)**: `serde_json` 200 ms / `parse_secure` 217 ms / **`parse_secure_sonic` 83 ms(約2.4倍速い、防御込み)** / `sonic-rs`そのまま 33 ms(約6倍、防御なし)。
`serde_json::Value`に読む限りSIMDでもほぼ速くならない(188 ms)。**速さの源は、結果を連続した領域に詰め込む値の型**。

**使える所 / 使えない所**
- ✅ 読み取り中心で、JSONを大きく・頻繁に読む所(キャッシュJSON、設定、APIの応答、ログ、インデックス)。
- ✅ 外部からの入力の入口(速さより防御が目的。`parse_secure`でも可)。
- ⚠️ `serde_json::Value`を関数間で受け渡している既存コードは、型を`sonic_rs::Value`に直す書き換えが要る(`JsonValueTrait`/`JsonContainerTrait`のメソッド)。書き換えずに`serde_json::Value`へ読むと速くならない。
- ❌ WASM(`rust_json::light`、依存ゼロ)、PHP/JavaScript側のJSON処理(Rustの部品は使えない)。
- 注意: 依存先の`sonic-rs`は内部で`unsafe`を使う(自前コードに`unsafe`は無い)。小さなJSON(数KB)では体感差が出ない。速度目的の書き換えは、実測で効く所から。

**導入**: `Cargo.toml`に `rust-json = { path = "../RS-JSON", features = ["fast"] }`。使用例は`RS-JSON/README.md`。

**このリポジトリでの適用状況**: 未適用(方針のみ)。コードの切り替えは、JSONを大きく・頻繁に読む所から実測して順に行う。

## JSON handling policy: use `parse_secure_sonic` (fast type + secure) and `sonic-rs` everywhere they can be used (2026-10-14, user instruction)
The source of truth is [`RS-JSON`](https://github.com/aon-co-jp/RS-JSON) (`README.md`, `PORTING.md`).

**Policy**: wherever JSON is read, use `parse_secure_sonic` from `rust-json` (RS-JSON, the `fast` feature; the result is `sonic_rs::Value`) **everywhere it can be used**.
Use `sonic-rs` directly where possible too (e.g. reading into typed structs). Where it cannot be used, use `parse_secure` (result: `serde_json::Value`).
Untrusted input from outside (HTTP bodies, files over the network, uploads) **must always be read with size / depth / duplicate-key limits**.

**Measured (7 MB, 40,000 records)**: `serde_json` 200 ms / `parse_secure` 217 ms / **`parse_secure_sonic` 83 ms (about 2.4x faster, with the defences)** / raw `sonic-rs` 33 ms (about 6x, no defences).
Reading into `serde_json::Value` gives almost no speed-up even with SIMD (188 ms). **The speed comes from the value type that packs the result into contiguous memory.**

**Where it can / cannot be used**
- ✅ Read-heavy code that parses large or frequent JSON (cache JSON, config, API responses, logs, indexes).
- ✅ Entrances for outside input (the goal there is defence rather than speed; `parse_secure` is fine).
- ⚠️ Existing code that passes `serde_json::Value` between functions needs its type changed to `sonic_rs::Value` (methods from `JsonValueTrait` / `JsonContainerTrait`). Reading into `serde_json::Value` without that change is not faster.
- ❌ WASM (`rust_json::light`, zero dependencies) and the JSON handling on the PHP / JavaScript side (Rust components cannot be used there).
- Note: the `sonic-rs` dependency uses `unsafe` internally (our own code has none). For small JSON (a few KB) the difference is not noticeable; do speed-motivated rewrites where measurements show a benefit.

**Adoption**: in `Cargo.toml`, `rust-json = { path = "../RS-JSON", features = ["fast"] }`. Example in `RS-JSON/README.md`.

**Status in this repository**: Not yet applied (policy only). Code will be switched over place by place, starting where measurements show large or frequent JSON reads.

## 2026-10-28 依存クレートの一括メジャー更新 / Major dependency update (2026-10-28)
**日本語**: 自動保守で残っていたメジャー更新を全て適用。`cargo check/test/clippy --workspace --all-features --all-targets` が通る(clippyは警告のみ・既存分)。
利用側(aruaru-search・realdata.pro・open-english/server)も新RPoemで`cargo check --all-targets`成功。
- 上げた版: base64 0.23 / clickhouse 0.15 / deadpool-redis 0.23 / hmac 0.13 / jsonschema 0.60 / jsonwebtoken 11.1 / lopdf 0.46 / opentelemetry・_sdk・-otlp 0.33 / tracing-opentelemetry 0.34 / rcgen 0.14 / redis 1.8 / reqwest 0.13 / serde_html_form 0.4 / sha1・sha2 0.11 / sonic-rs 0.5 / sqlx 0.9 / surrealdb 3 / syn 3 / thiserror 2 / tokio-tungstenite 0.30 / toml 1.1。`poem`・`poem-derive`・`indexmap`の厳密ピン(=3.1.0/=2.2.6)を解除(lopdf 0.46がindexmap 2.14+を要求)、`rust-version`を1.75→1.88。
- 破壊的変更への対応:
  - **sqlx 0.9**: feature `runtime-tokio-rustls`は廃止→`runtime-tokio`+`tls-rustls-ring-webpki`。`raw_sql`は動的文字列を`sqlx::AssertSqlSafe(..)`で包む必要あり(本コードは`sql_escape`済みのリテラルのみ)。
  - **clickhouse 0.15**: `Client::insert::<Row>(table)`が非同期(`.await`)かつ型引数必須。
  - **jsonwebtoken 11**: 暗号バックエンド必須→`features = ["use_pem","rust_crypto"]`(純Rust)。無いと実行時panic。
  - **reqwest 0.13**: `rustls-tls`→`rustls`、`.query()`は`query` feature。さらに**aws-lc-rsが引き込まれrustlsのプロバイダが2つ有効になる**ため、暗黙の既定(`ServerConfig::builder()`)は実行時panic→`builder_with_provider(ring)`で明示(`hyper_compat::load_tls_config`・`acme`)。利用側が`ServerConfig::builder()`を直接使うと同じ罠。
  - **jsonschema 0.60**: `ValidationError::instance_path`はフィールド→メソッド。 **hmac 0.13**: `new_from_slice`は`KeyInit`トレイト。 **rcgen 0.14**: `CertifiedKey.key_pair`→`signing_key`。 **tokio-tungstenite 0.30**: `Message::Binary`は`Bytes`。
  - 副次: `edfs`機能の`decode_event`が`sonic_rs::Error`を返す型不一致(更新前から全機能ビルドで失敗)を`serde_json::from_str`で修正。
- 注意: appserver/gateway のテストは先に`cargo build -p open-runo-appserver --bin open-runo-dummy-http-server`が必要。
- 未対応: なし(`serde`/`tokio`等の同一メジャー内の更新は`cargo update`で追従済み)。
**English**: Applied all pending major bumps (list above); workspace build/test/clippy pass with all features and the three consumers still `cargo check`. Notable breaking changes: sqlx 0.9 (`runtime-tokio`+`tls-rustls-ring-webpki`, `AssertSqlSafe` for dynamic `raw_sql`), clickhouse 0.15 (async typed `insert::<Row>`), jsonwebtoken 11 (must pick `rust_crypto`/`aws_lc_rs`), reqwest 0.13 (`rustls` feature, `query` feature, and it pulls in aws-lc-rs so rustls has two providers — implicit `ServerConfig::builder()` now panics; pass ring explicitly), jsonschema/hmac/rcgen/tungstenite API tweaks. Pins on poem/indexmap removed and `rust-version` raised to 1.88.


多言語要約(10言語)は `docs/i18n/<言語コード>/PORTING.md`。

## API 方針: REST 全廃・gRPC / Connect へ (2026-11-04)

### 日本語
- **REST は全廃する。新しい API は REST で作らない。** ブラウザや `curl` から呼ぶ API は **Connect の単純呼び出し**(`POST /<パッケージ>.<Service>/<Method>`、`content-type: application/json`、本文は JSON)、サービス間は **gRPC / gRPC-web**(RPoem の管理API `open_runo.admin.v1.Admin` と同じ流儀)。ストリーミングは、必要になってから。
- **既存の REST は、触るときに順に移す。** 移したら旧 REST の経路は消す。移行が終わるまで残るが、**新規には追加しない**。
- **JSON は RS-JSON に統一**する(読み込みは `from_str_secure` / `parse_secure` でサイズ・深さ・重複キーの上限つき、書き出しは `rust_json::fast`)。ブラウザの WASM に `prost` は入れない(サイズが増えるため)。
- **例外(自分たちの API ではなく、相手の規格が決めているもの)**: ACME、OAuth/OIDC のコールバック、第三者(GitHub など)からの Webhook、ヘルスチェック、静的ファイル配信、OTLP など。これらは規格どおりの HTTP を受ける。
- 参考実装: `RPoem`(`open_runo.admin.v1.Admin`)、`open-easy-web`(`server/src/connect.rs`、`proto/open_easy_web/admin/v1/admin.proto`)。
- ※ このリポジトリのコードが既に方針どおりかどうかは、各リポジトリの現状による。この節は方針の記録であり、コードの変更を意味しない。

### English
- **REST is being abolished. New APIs are not built as REST.** APIs called from browsers or `curl` use **Connect unary calls** (`POST /<package>.<Service>/<Method>`, `content-type: application/json`, JSON body); service-to-service calls use **gRPC / gRPC-web** (the same style as RPoem's admin API `open_runo.admin.v1.Admin`). Streaming only when it is actually needed.
- **Existing REST endpoints are migrated one by one, when they are touched.** The old REST route is removed once migrated. Until migration is finished they remain, but **no new ones are added**.
- **JSON is unified on RS-JSON**: reading with `from_str_secure` / `parse_secure` (size, depth and duplicate-key limits), writing with `rust_json::fast`. `prost` is not put into browser WASM (it would grow the binary).
- **Exceptions (shapes dictated by someone else's standard, not our own API)**: ACME, OAuth/OIDC callbacks, webhooks from third parties (e.g. GitHub), health checks, static file serving, OTLP, and so on. These accept plain HTTP as the standard requires.
- Reference implementations: `RPoem` (`open_runo.admin.v1.Admin`) and `open-easy-web` (`server/src/connect.rs`, `proto/open_easy_web/admin/v1/admin.proto`).
- Note: whether the code in a given repository already follows this policy depends on that repository. This section records the policy; it does not mean the code has been changed.

### Deutsch (KI-Übersetzung, redaktionell überarbeitet; noch nicht von Muttersprachlern geprüft)
- **REST wird vollständig abgeschafft; neue APIs entstehen nicht mehr als REST.** APIs, die aus dem Browser oder per `curl` aufgerufen werden, verwenden **unäre Connect-Aufrufe** (`POST /<Paket>.<Service>/<Methode>`, `content-type: application/json`, JSON-Body). Zwischen Diensten kommen **gRPC bzw. gRPC-web** zum Einsatz, im selben Stil wie die Admin-API von RPoem (`open_runo.admin.v1.Admin`). Streaming wird erst ergänzt, wenn es tatsächlich gebraucht wird.
- **Bestehende REST-Endpunkte werden schrittweise umgestellt, und zwar immer dann, wenn man ohnehin an ihnen arbeitet.** Sobald ein Endpunkt umgestellt ist, wird die alte REST-Route entfernt. Bis zum Abschluss der Migration bleiben die übrigen bestehen, es werden aber **keine neuen mehr hinzugefügt**.
- **JSON wird einheitlich über RS-JSON verarbeitet**: Gelesen wird mit `from_str_secure` bzw. `parse_secure` (mit Obergrenzen für Größe, Verschachtelungstiefe und doppelte Schlüssel), geschrieben mit `rust_json::fast`. `prost` wird nicht in das Browser-WASM aufgenommen, da es die Datei vergrößern würde.
- **Ausnahmen (Schnittstellen, deren Form ein fremder Standard vorgibt und die nicht zu unserer eigenen API gehören)**: ACME, OAuth/OIDC-Callbacks, Webhooks von Drittanbietern (z. B. GitHub), Health-Checks, die Auslieferung statischer Dateien, OTLP und Ähnliches. Diese nehmen, wie vom jeweiligen Standard verlangt, gewöhnliches HTTP entgegen.
- Referenzimplementierungen: `RPoem` (`open_runo.admin.v1.Admin`) und `open-easy-web` (`server/src/connect.rs`, `proto/open_easy_web/admin/v1/admin.proto`).
- Hinweis: Ob der Code eines einzelnen Repositorys diese Richtlinie bereits erfüllt, hängt vom jeweiligen Repository ab. Dieser Abschnitt dokumentiert lediglich die Richtlinie; er besagt nicht, dass der Code schon angepasst wurde.

### Русский (перевод выполнен с помощью ИИ и отредактирован; носителями языка пока не проверен)
- **От REST мы полностью отказываемся; новые API больше не создаются в стиле REST.** API, к которым обращаются из браузера или через `curl`, используют **унарные вызовы Connect** (`POST /<пакет>.<Service>/<Method>`, `content-type: application/json`, тело запроса — JSON). Для взаимодействия между сервисами применяются **gRPC и gRPC-web** — в том же стиле, что и административный API RPoem (`open_runo.admin.v1.Admin`). Потоковая передача данных добавляется лишь тогда, когда в ней действительно возникает необходимость.
- **Существующие REST-эндпоинты переводятся поэтапно — по мере того, как с ними приходится работать.** После перевода старый REST-маршрут удаляется. Пока миграция не завершена, оставшиеся эндпоинты сохраняются, но **новые не добавляются**.
- **Работа с JSON унифицирована на основе RS-JSON**: чтение — через `from_str_secure` или `parse_secure` (с ограничениями на размер, глубину вложенности и повторяющиеся ключи), запись — через `rust_json::fast`. `prost` в браузерный WASM не включается, поскольку это увеличило бы размер файла.
- **Исключения (интерфейсы, форму которых определяет чужой стандарт, а не наш собственный API)**: ACME, обратные вызовы OAuth/OIDC, вебхуки сторонних сервисов (например, GitHub), проверки работоспособности, раздача статических файлов, OTLP и тому подобное. Такие интерфейсы принимают обычный HTTP, как того требует соответствующий стандарт.
- Эталонные реализации: `RPoem` (`open_runo.admin.v1.Admin`) и `open-easy-web` (`server/src/connect.rs`, `proto/open_easy_web/admin/v1/admin.proto`).
- Примечание: соответствует ли код конкретного репозитория этой политике, зависит от самого репозитория. Данный раздел лишь фиксирует политику и не означает, что код уже изменён.

## API 方針の補足: 用語の整理と使い分け (2026-11-04)

### 日本語
**1つに統一するものではなく、別の層の話です。** 「REST の代わり」になるのは Connect・gRPC・GraphQL だけで、WASM は画面を作る技術です。

| 用語 | 何か | REST の代わりか |
|---|---|---|
| REST | URL と HTTP メソッドで操作を表す API の作り方 | 全廃する側 |
| **Connect** | `POST /サービス名/メソッド名` に JSON を送る形式。`curl` でもブラウザの `fetch` でも呼べる | はい(**採用**) |
| **gRPC** | 同じ「サービス/メソッド」の考え方を、バイナリ(protobuf)と HTTP/2 で行う形式。速く、ストリーミングも得意 | はい(**サービス同士に採用**) |
| gRPC-web | gRPC のブラウザ向けの変形版 | はい(必要なら) |
| GraphQL | 欲しい項目を問い合わせる形式 | 一部だけ(**問い合わせ(読み取り)専用**) |
| WASM | Rust をブラウザの中で動かす仕組み | いいえ(画面を作る技術) |

- **ブラウザの画面や `curl` から呼ぶ API は Connect**(POST、JSON を RS-JSON で読み書き)。ブラウザの WASM は素の gRPC を話せず、Connect なら `fetch` のままで、`prost` を足さずに済む(画面のサイズも増えない)。
- **サービス同士の通信(サーバー間、重い処理、ストリーミング)は gRPC。** 多くは Connect で足りるので、ストリーミングが必要になった所だけ gRPC に上げる。
- **GraphQL は、まとめて問い合わせたいときの読み取り専用。** 書き込み(操作)には使わない。
- **WASM は画面を作る技術として、そのまま使う。** 画面から呼ぶ API が Connect になっただけ。
- 外部の規格が形を決めるもの(ACME、OAuth のコールバック、第三者の Webhook、ヘルスチェック、静的ファイル、OTLP)は、規格どおりの HTTP のまま。
- **迷ったら Connect。**

### English
**These are not alternatives to unify; they sit at different layers.** Only Connect, gRPC and GraphQL can replace REST; WASM is a way to build the UI.

| Term | What it is | Replaces REST? |
|---|---|---|
| REST | API style that expresses operations through URLs and HTTP methods | The thing being abolished |
| **Connect** | `POST /Service/Method` with a JSON body; callable from `curl` and from a browser `fetch` | Yes (**adopted**) |
| **gRPC** | The same service/method idea over binary (protobuf) and HTTP/2; fast, good at streaming | Yes (**for service-to-service**) |
| gRPC-web | A browser-friendly variant of gRPC | Yes (if needed) |
| GraphQL | A query language: ask for exactly the fields you want | Partly (**read-only queries**) |
| WASM | Runs Rust inside the browser | No (a UI technology) |

- **APIs called from browser screens or `curl` use Connect** (POST, JSON read and written with RS-JSON). Browser WASM cannot speak plain gRPC; with Connect it stays a `fetch`, and `prost` is not needed (the binary does not grow).
- **Service-to-service calls (server to server, heavy work, streaming) use gRPC.** Connect is enough in most places; move a call to gRPC only where streaming is actually needed.
- **GraphQL is read-only**, for queries that gather several pieces of data. It is not used for writes (operations).
- **WASM stays as the way to build the UI.** The API it calls simply became Connect.
- Interfaces whose shape an outside standard dictates (ACME, OAuth callbacks, third-party webhooks, health checks, static files, OTLP) stay plain HTTP.
- **When in doubt, use Connect.**

### Deutsch (KI-Übersetzung, sprachlich überarbeitet; noch nicht von Muttersprachlern geprüft)
**Es geht nicht darum, eine einzige Technik auszuwählen – die Begriffe liegen auf unterschiedlichen Ebenen.** Als Ersatz für REST kommen nur Connect, gRPC und GraphQL in Frage; WASM dagegen ist eine Technik, mit der die Benutzeroberfläche gebaut wird.

| Begriff | Was ist das? | Ersatz für REST? |
|---|---|---|
| REST | Ein API-Stil, bei dem Operationen über URLs und HTTP-Methoden ausgedrückt werden | wird abgeschafft |
| **Connect** | `POST /Dienst/Methode` mit JSON im Body; aufrufbar mit `curl` ebenso wie mit `fetch` im Browser | ja (**gewählt**) |
| **gRPC** | Dasselbe Prinzip aus Dienst und Methode, aber über binäres protobuf und HTTP/2; schnell und gut für Streaming geeignet | ja (**zwischen Diensten**) |
| gRPC-web | Eine für den Browser angepasste Variante von gRPC | ja (bei Bedarf) |
| GraphQL | Abfragesprache, mit der man genau die benötigten Felder anfordert | teilweise (**nur für lesende Abfragen**) |
| WASM | Führt Rust-Code direkt im Browser aus | nein (Technik für die Oberfläche) |

- **APIs, die von Browseroberflächen oder per `curl` aufgerufen werden, verwenden Connect** (POST; JSON wird mit RS-JSON gelesen und geschrieben). WASM im Browser kann kein reines gRPC sprechen. Mit Connect bleibt es bei einem gewöhnlichen `fetch`, und `prost` ist nicht nötig – die WASM-Datei wird dadurch nicht größer.
- **Für die Kommunikation zwischen Diensten (Server zu Server, rechenintensive Aufgaben, Streaming) wird gRPC eingesetzt.** In den meisten Fällen reicht Connect aus; auf gRPC wird nur dort umgestellt, wo Streaming tatsächlich gebraucht wird.
- **GraphQL dient ausschließlich lesenden Abfragen**, wenn mehrere Informationen gebündelt abgefragt werden sollen. Für Schreibzugriffe (Operationen) wird es nicht verwendet.
- **WASM bleibt die Technik für die Oberfläche.** Geändert hat sich lediglich, dass die aufgerufene API nun auf Connect beruht.
- Schnittstellen, deren Form durch einen externen Standard festgelegt ist (ACME, OAuth-Callbacks, Webhooks von Drittanbietern, Health-Checks, statische Dateien, OTLP), bleiben gewöhnliches HTTP.
- **Im Zweifel: Connect.**

### Русский (перевод выполнен с помощью ИИ и литературно отредактирован; носителями языка пока не проверен)
**Это не варианты, из которых нужно выбрать один, а технологии разных уровней.** Заменить REST могут только Connect, gRPC и GraphQL; WASM же — это способ создания пользовательского интерфейса.

| Термин | Что это | Заменяет REST? |
|---|---|---|
| REST | Стиль построения API, при котором операции выражаются через URL и HTTP-методы | отменяется |
| **Connect** | `POST /Сервис/Метод` с телом в формате JSON; вызывается как из `curl`, так и из браузерного `fetch` | да (**выбран**) |
| **gRPC** | Тот же принцип «сервис — метод», но поверх двоичного формата protobuf и HTTP/2; работает быстро и хорошо подходит для потоковой передачи | да (**для связи между сервисами**) |
| gRPC-web | Адаптированный для браузера вариант gRPC | да (при необходимости) |
| GraphQL | Язык запросов: клиент запрашивает ровно те поля, которые ему нужны | частично (**только запросы на чтение**) |
| WASM | Позволяет выполнять код на Rust прямо в браузере | нет (технология для интерфейса) |

- **API, которые вызываются из браузерных интерфейсов или через `curl`, строятся на Connect** (POST; JSON читается и записывается через RS-JSON). Браузерный WASM не умеет работать с «чистым» gRPC. С Connect достаточно обычного `fetch`, а `prost` не требуется — размер WASM-файла не увеличивается.
- **Для обмена между сервисами (сервер — сервер, ресурсоёмкие задачи, потоковая передача) применяется gRPC.** В большинстве случаев достаточно Connect; на gRPC переходят только там, где потоковая передача действительно необходима.
- **GraphQL используется только для чтения**, когда нужно получить сразу несколько связанных данных. Для записи (операций) он не применяется.
- **WASM по-прежнему остаётся технологией для интерфейса.** Изменилось лишь то, что вызываемый им API теперь построен на Connect.
- Интерфейсы, форма которых задана внешним стандартом (ACME, обратные вызовы OAuth, вебхуки сторонних сервисов, проверки работоспособности, статические файлы, OTLP), остаются обычным HTTP.
- **Если есть сомнения — выбирайте Connect.**

## 外から来る JSON の入口を RS-JSON の安全モード+sonic-rs に統一(2026-11-06)
方針(`F:\runo\CLAUDE.md`「JSON処理の使用方針」)どおり、信頼できない入力は **`open_runo_router::secure_json`**(`rust_json::parse_secure_sonic` でサイズ16 MiB・深さ64・重複キー拒否を検査してから `sonic_rs` で型へ)を通す。対象: HTTP 本文(`hyper_compat::read_json_body`=全ハンドラーと poem-compat の `Json::from_body`)、poem-compat の型付き本文・WebSocket メッセージ(`api.rs`)、GraphQL の要求本文(`graphql_hyper.rs`)、リモートのサブグラフの応答(`remote_subgraphs.rs`)、EDFS の Redis ペイロード(`edfs.rs`)。管理 API の Connect 入口は `open-runo-connect-server` で済み。自分で書いた設定文字列(`admin_sys::parse_value`)は `sonic_rs::from_str`。
- **効果**: 重複キーを含む本文は 400(従来は黙って後勝ち)。深さ64超・16 MiB 超も 400。小さな本文での追加コストは約1 µs(前後差はノイズ以内)で、速度のためではなく防御のための統一。
- 確認: `cargo test --workspace --locked` 全通過。実機の gateway で、GraphQL の重複キー本文=400、SCIM の重複キー本文=400、正常=200/201。
- **まだ `serde_json` のままの所(理由)**: JWT のヘッダー/クレーム(`open-runo-jwt`、署名検証済みの数百バイト)、ACME サーバー応答(`acme.rs`、信頼する認証局・数KB)、DB の値の妥当性確認(`open-runo-db/dual.rs`)、CLI の `--value`(利用者自身の入力)、各クレートの試験コード。
# PORTING.md — RPoem お引越しファイル

> このファイル 1 枚で、**どのプロジェクトでも RPoem を導入・移設**できます。
> open-e-gov / OpenRedmine / OpenWordPress など新プロジェクトのリポジトリに
> このファイルをコピーして、上から順に進めてください。
>
> 対象バージョン: workspace 0.1.0（18 クレート / 335 テスト、--all-features で353）
> 最終更新: 2026-08-20(自動アップデート機構`self_update.rs`追加を反映)

## REST 全廃の状況(2026-11-01 時点、最新) — 管理操作は gRPC

**`/api/` 配下の REST はコード上すべて削除済み**。管理操作は **gRPC サービス `open_runo.admin.v1.Admin`**(定義は
`crates/open-runo-proto/proto/open_runo/admin/v1/admin.proto`)で、**同じ定義を gRPC-web・Connect でも受ける**。GraphQL は Federation の問い合わせだけ。
残る HTTP は `/health`・ACME・`/mcp`・SCIM(`/scim/v2/*`)・`/internal/db/mutate`・静的配信のみ。**このファイルの旧記述(`/api/…` や GraphQL `admin` を叩く例)は gRPC に読み替えること。**

- 起動: `cargo run -p open-runo-gateway`(管理画面・gRPC・GraphQL を同じポート 8080 で。`open-runo-router` 単体は管理サービスを持たない)。素のgRPCの別ポートは `OPEN_RUNO_GRPC_BIND_ADDR`(TLS1.3/mTLS 用)。
- 呼び方: `open-runo-cli`(素のgRPC、`--endpoint http://127.0.0.1:8080`)、管理画面(gRPC-web/Connect のハイブリッド、`RPC Inspector` で通信を確認)、
  `curl`(Connect JSON): `curl -X POST http://localhost:8080/open_runo.admin.v1.Admin/SelfIssueKey -H 'content-type: application/json' -d '{}'`。
- 管理画面の WASM は `www/pkg/` が git 管理外: `apps/desktop-wasm` で `cargo build --release --target wasm32-unknown-unknown` →
  `wasm-bindgen target/wasm32-unknown-unknown/release/open_runo_desktop_wasm.wasm --target web --out-dir www/pkg --no-typescript`(CLI のバージョンは Cargo.lock の wasm-bindgen と一致)。
  静的ファイルは ETag 再検証なので、作り直せば自動で配られる。
- 本番(`OPEN_RUNO_ENV=production`)は gRPC に TLS1.3 必須(+任意で mTLS)、GraphQL は登録済みクエリのみ・深さ/複雑さ上限・イントロスペクション停止・エイリアス/バッチ対策。設定の一覧と理由は `CLAUDE.md` の最新の節。
- 呼び出し側 `open-web-server` は、専用ブランチ `rest-to-graphql` で Connect(HTTP POST+JSON)に切替済み(未 push・未マージ)。**前面の open-web-server は HTTP/1.1 のみ**なので素のgRPC(HTTP/2)は通らない(Connect/gRPC-web は通る)。
- VPS: ゲートウェイは未稼働(`rpoem-web` は紹介ページ)。反映は `git pull` まで。起動手順と注意は `CLAUDE.md`。

## ブラウザ内AI実行(WASM+WebGPU)構想 — 技術検証済み・段階的導入計画(2026-08-25追記)

`experiments/wgpu-browser-spike/`に、`wgpu`(features=`webgpu`+`wgsl`)を
`wasm32-unknown-unknown --release`向けにコンパイルできることを確認した
最小スパイクを配置済み(`cargo check`成功、実ブラウザでの完全なE2E実行は
未検証)。ブラウザは既にW3C勧告候補の`WebGPU` APIを実装済みのため、
`wasi:webgpu`(非ブラウザ実行環境向けの別提案)の成熟を待つ必要はない。
段階的導入計画(第1段階: aruaru-llmのCPU推論をwasm32ビルド+RPoemの
`apps/desktop-wasm`でホスト+open-englishから読み込み、第2段階: WebGPU
経由のGPU推論)の詳細は`CLAUDE.md`の2026-08-25エントリを参照。

## 自動アップデート機構(2026-08-20新設、`crates/open-runo-router/src/self_update.rs`)

起動時にGitHub Releases API(`aon-co-jp/RPoem`)で最新タグを確認し、
現バージョンより新しければダウンロード→安全な自己置換(ヘルスチェック
成功時のみ切替、失敗時は`.bak`からロールバックし旧プロセスは継続稼働)
を行う。**既定オフ**——`OPEN_RUNO_ROUTER_SELF_UPDATE=true`を明示設定
した場合のみ動作する(VPS本番運用との予期しない競合を避けるため)。
Linux(x86_64/aarch64)・Windows(x86_64)のみ対応。詳細・未検証事項は
`self_update.rs`モジュールdoc、および本リポジトリの`CLAUDE.md`
2026-08-20エントリを参照。他プロジェクトへ移植する場合は
`Cargo.toml`の`reqwest`が非optional依存になっている点に注意
(GitHub Releases APIへの常時稼働HTTPクライアントとして必要)。

---

## 1. RPoem とは（30 秒版）

**RPoem** は、以下の3要素をすべて外部パッケージに依存せず
自前で一から開発・再現し、その連携をスムーズに行うことで WEB サイト/
WEB アプリ開発を効率的に行えるようにする**フレームワーク/ミドルウェア**
です(2026-07-11、ユーザーによる最終定義)。

1. **cosmo部分**（open-cosmo と共通のコア）: WunderGraph Cosmo 有料版
   （Launch/Scale/Enterprise）の機能を、Cosmo自体には依存せず
   Rust + tokio/hyper で自前再実装した OSS 版。**REST API不要**
   （VersionlessAPI/GraphQL Federation）・**契約不要**（商用ライセンス
   なしで同等機能）・**独自AI搭載のWeb高速化**（`CachePredictor`の
   自己学習型HTMLキャッシュ）・Tauri互換のフロントエンド体験、を含む。
2. **poem部分**（バックエンド、本リポジトリ固有）: Rust の Poem
   フレームワークの**全機能を完全互換で一から自作・再現**（`poem`
   パッケージへの直接依存なし。到達度は `docs/poem-parity.md` 参照）。
3. **tauri部分**（フロントエンド、本リポジトリ固有）: Tauri の**全機能を
   完全互換で一から自作・再現**（`tauri`パッケージへの直接依存なし、
   現状 Rust→WASM 実装。到達度は `docs/tauri-parity.md` 参照）。

この3要素（Tauri再現フロントエンド + open-cosmo/cosmoコア + Poem再現
バックエンド）がスムーズに連携し合うこと自体が価値であり、フロントエンド
開発・バックエンド開発・Web中心的な開発の連携を円滑にすることが本
フレームワークの目的です。「連携」とはいえ、各部分は外部フレームワーク
との統合ではなく、そのフレームワーク自体の完全な自前再実装である点に
注意してください。

以下、実装本体である Rust + tokio/hyper 製 **GraphQL Federation
プラットフォーム / Web フレームワーク**部分（cosmo部分）の詳細です。
さらに独自の自己学習 AI（外部 LLM 契約不要）を搭載します。

| 分類 | 提供機能 |
|------|----------|
| GraphQL | Federation 合成・破壊的変更検出・`POST /graphql`・Subscriptions (WS)・GraphiQL |
| Cosmo 有料版互換 | Persisted Queries (Trusted Documents)・厳密 RBAC・OIDC SSO・SCIM 2.0 (Users/Groups)・監査ログ・細粒度レートリミット・レスポンスキャッシュ・マルチグラフ namespace |
| 独自 AI（LLM 不要） | HTML ページキャッシュの自動判定/適応 TTL/先回り再生成、API キーの異常検知・自動隔離 |
| 自己運用 | 鍵の自動発行/失効（SCIM 連動）、AI 学習の自動永続化、両 DB 整合性の自動検証・自動修復、二か所自動バックアップ |
| お引越し | エンジン間変換（MySQL→PostgreSQL 等）、SQL/CSV エクスポート（Snowflake）、分散 DB 統合（Federated）、ワンコール復活 |

## 2. 持っていくもの（ファイル一覧）

```
open-runo/
├── Cargo.toml / Cargo.lock      ← workspace 定義（バージョン固定）
├── crates/                      ← 18 クレート（本体）
├── apps/desktop-wasm/            ← Rust→WebAssembly 管理アプリ（任意、open-runo-routerが自前配信）
├── docs/                        ← 設計・API 仕様・migration.md ほか
├── .github/workflows/ci.yml     ← fmt / clippy -D warnings / test
├── Dockerfile / docker-compose.yml / Makefile
└── PORTING.md                   ← 本ファイル
```

丸ごと移設する場合はフォルダごとコピーして `cargo test --workspace`
（256 テストが通れば移設成功）。以下はライブラリとして使う場合です。

## 3. 依存の書き方（新プロジェクトの Cargo.toml）

```toml
[dependencies]
# 同一マシンにある場合（path 依存）
open-runo-core     = { path = "../open-runo/crates/open-runo-core" }
open-runo-router   = { path = "../open-runo/crates/open-runo-router" }
open-runo-gateway  = { path = "../open-runo/crates/open-runo-gateway" }
open-runo-db       = { path = "../open-runo/crates/open-runo-db" }
open-runo-cache    = { path = "../open-runo/crates/open-runo-cache" }
open-runo-security = { path = "../open-runo/crates/open-runo-security" }

# GitHub 公開後は git 依存でも可
# open-runo-router = { git = "https://github.com/aon-co-jp/RPoem" }

tokio = { version = "1.40", features = ["full"] }
hyper = { version = "1.10", features = ["full"] }
# poem は不要(open-runo-routerは4.1のようにtokio/hyperで直接動く)。
# GraphQL Subscriptions(WebSocket)が必要な場合のみ追加:
# poem = { version = "3.1", features = ["sse"] }
```

必要なものだけ選べます（各クレートは独立してテスト可能）。
DB エンジンは feature で選択: `open-runo-db = { ..., features = ["dual"] }`
（`postgres` / `mysql` / `sqlite` / `aruaru` / `cockroach` / `yugabyte` /
`mongodb` / `redis` / `clickhouse` / 複合: `dual` `single-pg` `single-my`
`dev` `full` `cluster`）。

## 4. 組み込みレシピ

### 4.1 フルスタック（REST + GraphQL + 自己運用ぜんぶ）

`open-runo-gateway`の`main.rs`をそのまま流用するのが最速です(poem非依存、
tokio/hyperベース):

```rust
use open_runo_core::Config;
use open_runo_router::{build_hyper_app, hyper_compat, state::AppState};
use std::sync::Arc;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = Config::from_env()?;
    open_runo_observability::init_tracing(&config.log_level);

    let state = Arc::new(AppState::new()); // 本番: AppState::with_db(...)
    let (graphiql, graphql_post) =
        open_runo_gateway::graphql_hyper::graphql_handlers(Arc::clone(&state));

    let app = build_hyper_app(
        state,
        config.rate_limit_max_requests,
        config.rate_limit_window_secs as i64,
    )
    .route(hyper::Method::GET, "/graphql", graphiql)
    .route(hyper::Method::POST, "/graphql", graphql_post);

    let addr = config.bind_addr.parse()?;
    let (_, handle) = hyper_compat::serve(app, addr).await?;
    handle.await?;
    Ok(())
}
```

GraphQL Subscriptions（WebSocket）が必要な場合のみ、代わりに
`open_runo_gateway::graphql_route(state)`（poem版、`/graphql/ws`対応）を
使ってください — その場合`poem`への依存が復活します。

これだけで、認証（KeyGuardian 自動運用）・RBAC・OIDC・SCIM・監査ログ・
AI モデル自動永続化・整合性自動修復・定期バックアップが**全部背景で動きます**。

### 4.2 自分のページに AI HTML キャッシュだけ載せる（OpenWordPress 型）

```rust
use open_runo_router::middleware::html_cache::{
    HtmlCacheConfig, HtmlCacheMiddleware, HtmlPageCache,
};
use poem::{get, handler, EndpointExt, Route};
use std::sync::Arc;

#[handler]
async fn render_article(/* db から記事を取って HTML を返す */) -> String {
    /* 重いレンダリング */ String::new()
}

let cache = Arc::new(HtmlPageCache::new(HtmlCacheConfig::from_env()));

let app = Route::new()
    .at("/article/:id", get(render_article))
    .data(Arc::clone(&cache))          // 更新ハンドラから purge するため
    .with(HtmlCacheMiddleware(cache)); // ← これだけで AI キャッシュ有効
```

記事更新時は `cache.purge("/article/123").await;` を 1 行呼ぶだけ。
purge は AI の学習信号にもなり、更新の多いページは TTL が自動短縮されます。
個人化部分（ようこそ○○さん等）は `Cookie` 付きリクエストが自動バイパス
されるため、共通 HTML + JS Fetch の分離パターンでどうぞ。

### 4.3 単一 DB で始めて後から DUAL に育てる

```rust
use open_runo_db::dual::DualBackend;
use open_runo_router::state::AppState;

// 開発: SQLite 1 台でも DUAL と同じコードパス
let state = AppState::with_single_db(Arc::new(sqlite_backend));

// 本番: PostgreSQL + aruaru-db の二重化（自動整合性修復つき）
let dual = DualBackend::with_default_routing(postgres, aruaru);
let state = AppState::with_db(Arc::new(dual));
```

### 4.4 社内の分散 DB を 1 つに統合（Federated）

```rust
use open_runo_db::federated::FederatedBackend;

let fed = FederatedBackend::builder()
    .member("tokyo-pg", tokyo_postgres)
    .member("osaka-my", osaka_mysql)
    .route("orders", "osaka-my")
    .broadcast("schemas")
    .default_member("tokyo-pg")
    .build()?;
let state = AppState::with_db(Arc::new(fed));
```

段階的な片寄せは `open_runo_db::migrate::transfer_verified(src, dst, tables)`。

## 5. データのお引越し（既存環境から）

```bash
# 旧環境: 全 DATA + AI 学習記録をポータブル JSON へ（二か所に書き込み）
curl -X POST -H "x-api-key: $KEY" http://old:8080/api/backup/export

# 新環境: ファイル指定で取り込み、または最新を自動発見して一発復活
curl -X POST -H "x-api-key: $KEY" -d '{"path":"..."}' http://new:8080/api/backup/import
curl -X POST -H "x-api-key: $KEY" http://new:8080/api/backup/restore-latest
```

異種エンジンへの変換・Snowflake 取り込み・無停止統合は `docs/migration.md` 参照。

## 6. 環境変数 全一覧

| 変数 | 既定 | 説明 |
|------|------|------|
| `OPEN_RUNO_BIND_ADDR` ほか Config 系 | — | `open-runo-core::Config` 参照 |
| `OPEN_RUNO_JWT_SECRET` | 無効 | HS256 Bearer 認証 |
| `OPEN_RUNO_OIDC_ISSUER` / `_OIDC_JWKS_FILE` / `_OIDC_AUDIENCE` | 無効 | OIDC SSO (JWKS/RS256) |
| `OPEN_RUNO_RBAC` | off | `enforce` でルート単位 RBAC |
| `OPEN_RUNO_SCIM_TOKEN` | 無効 | IdP 用固定 Bearer（/scim/* 限定） |
| `OPEN_RUNO_KEY_ANOMALY_FACTOR` / `_KEY_WARMUP_REQUESTS` / `_KEY_COOLDOWN_SECS` | 20 / 50 / 300 | KeyGuardian 異常検知 |
| `OPEN_RUNO_PQ_MODE` | allow | disabled / allow (APQ) / enforce (Trusted Documents) |
| `OPEN_RUNO_CACHE` / `_CACHE_TTL_SECS` | off / 30 | GraphQL レスポンスキャッシュ |
| `OPEN_RUNO_HTML_CACHE` | off | AI HTML ページキャッシュ（`on` で有効） |
| `OPEN_RUNO_HTML_CACHE_TTL_SECS` / `_MAX_ENTRIES` / `_MIN_HITS` | 60 / 10000 / 2 | 同上の基本設定 |
| `OPEN_RUNO_HTML_CACHE_AI` | on | `off` で固定 min-hits 判定へ |
| `OPEN_RUNO_HTML_CACHE_REFRESH_RATIO` | 0.8 | 先回り再生成の閾値 |
| `OPEN_RUNO_CACHE_AI_MIN_TTL_SECS` / `_MAX_TTL_SECS` / `_DEFAULT_TTL_SECS` / `_MAX_TRACKED` / `_MIN_EXPECTED_HITS` | 5 / 3600 / 60 / 50000 / 1.0 | AI 予測器チューニング |
| `OPEN_RUNO_AI_PERSIST_SECS` | 300 | AI 学習の自動保存（0=off） |
| `OPEN_RUNO_INTEGRITY_SECS` | 3600 | 両 DB 整合性の自動検証・修復（0=off） |
| `OPEN_RUNO_BACKUP_DIR` | ./backups | 一次バックアップ先 |
| `OPEN_RUNO_BACKUP_MIRROR_DIR` | 無効 | 二次（Google Drive 同期フォルダ推奨） |
| `OPEN_RUNO_BACKUP_SECS` | 0 | 定期バックアップ間隔（0=手動） |

## 7. REST サーフェス早見表(**旧記述: 2026-11-01 に REST は削除済み。GraphQL に読み替えること**)

| Path | 用途 |
|------|------|
| `/health` `/healthz` | 認証不要ヘルスチェック |
| `/api/openapi.json` | REST APIのOpenAPI 3.0仕様(認証不要、Postman/Insomnia/Swagger UIへインポート可) |
| `/.well-known/acme-challenge/:token` | ACME HTTP-01チャレンジ応答(認証不要、CAが直接アクセスする) |
| `/api/keys/self-issue` | **認証不要**でAPIキーを自動発行(developer role、24時間有効)。人間がキーを意識しない設計の起点 |
| `/mcp` | MCP(Model Context Protocol)Streamable HTTP transport(認証不要、JSON-RPC 2.0)。`initialize`/`tools/list`/`tools/call`/`resources/list`/`resources/read`/`prompts/list`/`prompts/get`に対応。実ツール2種(`health_check`・`self_issue_api_key`)+実リソース2種(`openapi://spec`・`health://status`)+実プロンプト1種(`summarize_api`、同じ`openapi::spec()`を動的にレンダリング)、いずれも既存REST APIと同じ本番ロジックを共有 |
| `/api/session/login` `/logout` | `X-Api-Key`を追加のセッションCookie(HttpOnly+SameSite=Strict)+CSRFトークンへ交換(置き換えではなく追加の認証経路) |
| `/graphql` (+`/graphql/ws`) | Federation GraphQL / Subscriptions(GraphiQLは`GET /graphql`) |
| `/api/schemas*`（`?namespace=`） | Schema Registry（マルチグラフ対応） |
| `/api/schemas/upload` | `multipart/form-data`でSDLファイルを直接アップロードして登録(`/api/schemas`のJSON版と同じ登録処理を共有) |
| `/api/federation/*` | 合成・状態 |
| `/api/persisted-queries*` | Trusted Documents 登録・取得 |
| `/api/db/*` | DUAL DATABASE KV |
| `/api/db/:table/:key/at/:commit_id` | VersionLessAPI + Git-on-SQL ハイブリッドの読み出し側(2026-07-14、open-cosmoからミラー) — 指定コミット時点の値を取得(`aruaru-db`バックエンドのみ実対応、他バックエンドは501) |
| `/api/ai/route` | AI プロバイダ選択 |
| `/api/events` | SSE |
| `/api/ws-echo` | 汎用 WebSocket エコー（手書き RFC 6455、認証不要） |
| `/api/ws-events` | 汎用 WebSocket イベント配信（認証必須、SSE と同じブローカー） |
| `/api/feature-flags*` `/api/feature-flags/:name` `/api/feature-flags/:name/evaluate` | Feature Flags（upsert/list/get/delete/evaluate、決定的バケッティングによる canary ロールアウト） |
| `/scim/v2/Users` `/scim/v2/Groups` | SCIM 2.0（鍵の自動発行/失効つき） |
| `/api/cache/purge` `/purge-all` `/ai-stats` | HTML キャッシュ管理・AI 観測 |
| `/api/backup/export` `/import` `/restore-latest` | バックアップ・復活 |
| `/api/migrate/export-sql` `/export-csv` | エンジン変換エクスポート |
| `/api/integrity/check` | 両 DB 整合性チェック・自動修復 |
| `/api/analytics/requests-per-month` | 月間リクエスト数（運用メトリクスのみ、課金/レート制限には不使用） |
| `/api/analytics/operations` | オペレーション別（method+path）レイテンシ・エラー率集計 |

Federation合成（`POST /api/federation/compose`）は`sdl`フィールドで生の
GraphQL SDL（Federation v1の暗黙ディレクティブ方式・v2の`@link`方式
どちらも自動判別）を渡せます。従来の`{service_name, types: {...}}`
JSON直接指定も後方互換のためそのまま利用可能です。

認証: `X-Api-Key`（KeyGuardian 台帳が空なら開発モードで任意値可）/
JWT / OIDC Bearer / セッションCookie(`/api/session/login`発行、状態変更
リクエストは`X-CSRF-Token`ヘッダ必須)。RBAC 有効時、管理系は `admin`
ロール必須。

TLS を直接終端したい場合（リバースプロキシを使わない構成）は
`tls` Cargo feature（既定オフ）で `hyper_compat::tls::{load_tls_config,
serve_tls}` が使えます。証明書を自動取得したい場合は`acme` feature
（`tls`を暗黙有効化）で`acme::{AcmeClient, obtain_certificate_http01,
tls_alpn01::obtain_certificate, dns01::obtain_certificate}`（RFC 8555、
HTTP-01・TLS-ALPN-01・DNS-01の3チャレンジ型すべてに対応）が使えます。
DNS-01は`acme::dns01::DnsProvider` traitを実装すれば任意のDNSプロバイダに
対応可能（付属の`CloudflareDnsProvider`はbearerトークンのみ・署名不要な
最小実装例）。gRPCが必要な場合は`grpc::serve_grpc`（新規依存無し、
`grpc.health.v1.Health/Check`・`/Watch`ストリーミング・
`grpc.reflection.v1.ServerReflection`の`list_services`）を
`OPEN_RUNO_GRPC_BIND_ADDR`で有効化。ネイティブのシステムトレイ常駐アプリ・
OSネイティブ通知・Windows インストーラーが必要な場合は `apps/desktop-tray`
（`tauri`パッケージ非依存）をそのまま移植できます。

## 8. 動作確認

```bash
cd open-runo
cargo test --workspace     # 296 テスト(--all-features で305) + doctest が全部通れば OK
cargo run -p open-runo-gateway   # REST + GraphQL 統合バイナリ起動
```

## 9. 命名規約（お引越し先でも守ること）

- クレート/ディレクトリ: `open-runo-*`　- Rust パス: `open_runo_*`
- 環境変数: `OPEN_RUNO_*`　- 型名のみ CamelCase: `OpenRuno*`（Rust 言語制約）

## 10. 詳細ドキュメント

`docs/HANDOFF.md`（全開発履歴）/ `architecture.md` / `cosmo-parity.md` /
`migration.md` / `api-spec.md` / `database.md` / `security.md` / `federation.md`

## 11. 移植・拡張時の注意

他プロジェクトへ移植・拡張する際、依存クレートの現状（バージョン・
メンテナンス状況）や仕様の詳細で判断に迷う場合は、学習データからの
推測だけに頼らず、実際に検索（日本語・英語の両方の Google 検索、
および GitHub 上の実装状況調査）して裏付けを取ってから技術選定を
行うことを推奨する。

**パフォーマンス・並行処理**: 移植先でも、非同期（tokio マルチスレッド
ランタイム）を基本としつつ、必要な場面のみ同期処理を用いる方針を
踏襲すること。`#[tokio::main]` のランタイム flavor が
`current_thread` に固定されていないか、async 関数内でブロッキング
I/O・CPU 負荷処理を直接呼んでいないか（`tokio::task::spawn_blocking`
へ退避すべき）、CPU 律速な処理は `rayon` 等でのデータ並列化を検討する
価値があるか、を移植時にも確認するとよい。

**最先端追従の方針(2026-07-23追記)**: 移植時の日英Web検索で
CockroachDB/TiKV等の最先端の実運用システムが既に対応済みと判明した
設計ギャップが見つかった場合、「今のところは大丈夫」という報告に
留めず、確認を求めず自動でそのまま実装に着手すること(ユーザー指示、
正本は`open-raid-z/CLAUDE.md`・`PORTING.md`同日エントリ参照)。

**送信側だけあって受信側が無い、という欠落の見つけ方(2026-07-23、
UDP-IP冗長経路で実際に発見した教訓)**: 「Aが呼ぶエンドポイント/経路を
Bが実装しているはず」という設計は、**Bを実際にgrepして存在確認する
まで信用しないこと**。`open-web-server-ledger::Ledger`が
`POST /internal/db/mutate`をフォワードする設計だったが、RPoem側に
このエンドポイントが2026-07-23まで一つも存在しなかった
(`grep -rn "internal/db/mutate" crates/`で0件だったことで発覚)。
同様にUDP冗長経路も送信側のみ実装され受信側が存在しなかった。移植・
統合作業では「送信元のコードだけでなく、受信先に対応する実装が
本当に存在するか」を必ずgrepで裏取りすること。

**Apache/Tomcat互換の多言語アプリケーションサーバー対応(2026-07-23、
ユーザー指示、正本は`open-raid-z/CLAUDE.md`同名節参照)**: このリポジトリを
他プロジェクトへ移植・拡張する際、「open-web-serverをJavaのApacheのように、
RPoemをTomcatのように——Java/Ruby on Rails/PHP+Laravel/Python+FastAPI等
言語を問わず連携できる汎用性を高める」という目標を踏襲すること。移植先で
このリポジトリの`open-runo-appserver`(第二のTomcat骨格)を使う場合、
プレーンHTTPで応答する任意の言語・フレームワークのアプリケーションサーバー
(Rustに限らない)を`tenant_bridge`/`SharedDispatcher`経由でそのまま
指せる設計を維持すること——特定言語向けの専用実装を持ち込まない
(HTTPで統一する既存方針からブレないこと)。

## 12. RPoem ⇔ RCosmo の「Cosmo共通コア」重複整理(2026-07-23)

RCosmoは`docs/HANDOFF.md`の通りRPoemと共通のCosmoコア(Federation・
VersionlessAPI・SCIM・Security・Cache等)を持つ姉妹リポジトリ。今回、
実際に両リポジトリの`crates/`配下をファイル単位で`diff`した結果を記録する。

- **調査結果**: 共通20クレートのうち **18クレートが両リポジトリで
  byte-for-byte完全一致**(`open-runo-ai-routing`/`api-types`/
  `appserver`/`backup`/`cache`/`cli`/`core`/`feature-flags`/
  `federation`/`history`/`observability`/`persisted-queries`/
  `rustjson`/`schema-registry`/`scim`/`security`/`versionless-api`/
  `view`)。残る`open-runo-db`/`open-runo-gateway`/`open-runo-router`の
  3クレートは、RPoem固有の追加(`appserver_tenants.rs`・`udp_notice.rs`・
  gRPC/ACME/MCP拡張等)により意図的に分岐している——これらは
  「共通コア」ではなくRPoem固有スコープ(第二のTomcat・Poem完全再現)の
  範囲に属するため分岐は正常。
- **既存パターンの踏襲を確認**: このエコシステムの「別リポジトリの
  crateへ直接のCargo依存はしない、小さなモジュールとして直接コピーする」
  という既存方針(RustJSON移植等)通り、両リポジトリは同じディレクトリ
  ツリー(`F:\runo\RPoem`/`F:\runo\RCosmo`)に配置されており path依存も
  技術的には可能だが、CI環境でのpath依存問題を避けるため、**今回も
  コピー移植パターンを継続する判断とした**(ワークスペース統合は
  スコープが異なる別プロジェクトという既存方針に反するため不採用)。
- **新規: `scripts/sync-cosmo-core.sh`**(両リポジトリに同一ファイルを配置)。
  従来「手作業でファイルをコピーしてミラーする」運用だった同期作業を
  機械的に検証・実行できるようにした:
  - `scripts/sync-cosmo-core.sh check` — 共通コア18クレートの同期状態を
    一覧表示(diff -rq ベース、Cargo依存追加なし)。
  - `scripts/sync-cosmo-core.sh diff <crate>` — 指定クレートの差分を表示。
  - `scripts/sync-cosmo-core.sh push <crate>` / `pull <crate>` — 片方向へ
    `src/`をまるごとコピー(コピー後は`git diff`で確認し
    `cargo test`してからコミットすることを想定、自動コミットはしない)。
  - 実際に使用: 本セッションで発見した`open-runo-appserver`の
    Windows非互換テストバグ修正(後述)をこのスクリプトの`push`で
    RCosmo側へ同期し、`check`で0件drift(18/18 in sync)を確認済み。

## 13. GitHub `open-runo`(旧`open-cosmo`)からの「救済」調査(2026-07-24、結論: 移植なし)

ユーザーから「open-runo内にPoem+Cosmo+Tauri+WEB高速化のプログラムが
残っていれば、本来RPoemの物なので救済(移植)してほしい。ただし
open-runoがWunderGraph Cosmo有料版とWEB高速化中心の狭いプロジェクト
であれば、そのまま続けてよい」という条件分岐の指示を受けた。

`F:\runo\open-runo`(新規クローン)の中身を確認したところ、
`docs/poem-parity.md`・`docs/tauri-parity.md`・`apps/desktop-tray`・
`apps/desktop-wasm`・約20個の`open-runo-*`クレートが実在し、狭い
プロジェクトではなく広範な実装を持っていたため、11節・12節の
RPoem⇔RCosmo比較と同じ手順(`diff -rq`)で、今度はRPoem⇔open-runo間
を比較した。

- **`apps/desktop-tray`・`apps/desktop-wasm`(Tauri相当のデスクトップ
  アプリ体験)**: 両リポジトリで完全一致。差分は`target/`・
  `installer/dist`・`www/pkg`のようなビルド成果物のみで、ソース
  コードに差は無かった。ユーザーが最優先で確認を求めた項目だったが、
  RPoem側に既に同一のものが存在しており救済不要と判明。
- **`crates/`(共通20クレート)**: 17クレートが完全一致。差分のあった
  `open-runo-appserver`/`open-runo-db`/`open-runo-gateway`/
  `open-runo-router`/`open-runo-rustjson`の5クレートは、**すべて
  RPoem側がopen-runo側を上回る内容**(RPoemのみに存在する
  `appserver_tenants.rs`・`udp_notice.rs`・`router/tests/`、
  `open-runo-db`の`put_versioned`実装、行数もRPoem側が常に多い)。
  コメント中の実名・日付入り開発メモがopen-runo側では汎化・簡略化
  されている痕跡から、open-runoはRPoemから派生・エクスポートされた
  スナップショットだと判断した。
- **RPoemにのみ存在するクレート**: `open-runo-poem-compat`・
  `open-runo-poem-compat-macro`(Poem互換ファサード)——open-runo側には
  そもそも存在しない。

**結論**: open-runo側にRPoemより優れている・RPoemに無い実装は
見つからなかったため、**RPoemへの移植は0件**(コード変更なし)。
`open-runo`リポジトリ自体の扱い(削除・アーカイブ等)はユーザー指示
通りスコープ外としており、本セッションでは一切操作していない。
将来open-runo側で独自コミットが増えた場合は、本節と同じ
`diff -rq crates/<name>`手順で再判定すること。

## 15. `tenant_bridge`の実E2E検証(2026-08-04、初の実証)

`crates/open-runo-appserver/src/tenant_bridge.rs::dispatcher_from_tenants`
(`open-web-server::TenantRegistry`との橋渡し関数)は単体テストのみで
実証されておらず、実際にopen-web-server本体と接続してのE2E検証は
一度も行われていなかった。新規`crates/open-runo-appserver/examples/
e2e_stub_app.rs`(`tenant_bridge`で構築した`Dispatcher`を
`ThreadedProxyServer`に載せ、`X-Served-By: RPoem-appserver`固定応答を
返す最小upstreamの手前に立てる)を作り、open-web-server本体を実起動
→`POST /admin/tenants`で登録→`Host`ヘッダ指定リクエストが実際に
RPoem側まで転送されることを、5回連続で`200`+期待ヘッダにより実証した
(モック無し、実TCP2プロセス間通信)。移植先で同様のブリッジ機構を
検証する際は、この`examples/`ディレクトリのパターン(スタブ
アプリケーションサーバー+実バイナリ2つの同時起動)をそのまま使える。
詳細・初回リクエストの一時的な502(ウォームアップ猶予の要検討課題)は
[CLAUDE.md](CLAUDE.md)の2026-08-04 HANDOFF参照。

## 16. open-web-server⇔RPoem「4層4重」連携の再検証(2026-09-30、破損なし)

「15.」の実証(2026-08-04)から約2ヶ月経過した時点で、`open-web-server`を
Apache、RPoemをTomcatとして多段Webサーバー化する構成が現在も機能して
いるかをユーザーから問われ、`cargo test --workspace`をRPoem・
open-web-server双方で実行して再検証した。

- RPoem: 全クレート・全テスト成功(`ledger_fusion.rs`のクロスリポジトリ
  統合テストを含む)。
- open-web-server: 全クレート・全テスト成功(`tenant_router`・`proxy`の
  接続レース耐性テストを含む)。

自動テストの範囲では2026-08-04時点からのリグレッションは無いことを確認。
`examples/e2e_stub_app.rs`を使った実バイナリ2プロセス同時起動の手動E2E
手順自体は今回未実施(詳細は`CLAUDE.md`の2026-09-30 HANDOFF参照)。

## v1.3.0(2026-10-10): かすれ補正・字形照合・AI文脈補完の拡張 / v1.3.0: faded-text rescue, glyph matching, extended AI context completion

### 日本語
- 新設 **`crates/open-runo-glyph`**(MIT OR Apache-2.0): かすれた升目と候補の字形(fontdue で描画、明朝・ゴシック)を正規化相互相関で照合。升目の全位置を 1 回の行列積にする `windows()` / `GlyphIndex`(`matrix()` を `sgemm` の B に渡し、`reduce()` で最良位置・文字ごとの最大・上位を取る)と、CPU の参照実装 `rank_cpu()`。試験 3 件。
- **`crates/open-runo-pdf-ocr`**: `enhance.rs`(コントラスト自動補正 `normalize_contrast` = 常にオン、かすれ行の超解像+再 OCR `reocr_lines` = 設定 `reocr`)、`complete.rs`(走査 → 疑わしい箇所 → 候補 → 採点 → 決定 → 反映。複数の単語にまたがる置換は結合)、`visual.rs`(升目の切り出しと証拠の強さ)、`client.rs`(`scan` / `propose` / `fill` / `charset` / `glyph_rank`)、設定 `reocr*` / `fill*`(`fill_server_url` で補完だけ別サーバーにできる)。試験 20 件。
- **`crates/open-runo-sr`**: `upscale_crop()`(小さな切り抜きの拡大)を追加。Arm 向け NEON カーネルは v1.1.1。
- 測定結果・試験手順・未了は `pdf-r2l-rs/PORTING.md`(日英)に詳しく記録。補完は実験的・既定オフ。

### English
- New **`crates/open-runo-glyph`** (MIT OR Apache-2.0): matches a faded cell against candidate glyphs (rendered with fontdue, serif and sans) by normalized cross-correlation. `windows()` / `GlyphIndex` turn all positions into one matrix product (pass `matrix()` as B of `sgemm`, then `reduce()` takes the best position, the per-character maximum and the top list), plus the CPU reference `rank_cpu()`. 3 tests.
- **`crates/open-runo-pdf-ocr`**: `enhance.rs` (automatic contrast correction `normalize_contrast` = always on; super-resolution + re-OCR of faded lines `reocr_lines` = setting `reocr`), `complete.rs` (scan → suspect spots → candidates → scoring → decision → apply; replacements across words merge them), `visual.rs` (cell cutting and evidence strength), `client.rs` (`scan` / `propose` / `fill` / `charset` / `glyph_rank`), settings `reocr*` / `fill*` (`fill_server_url` can point only completion at another server). 20 tests.
- **`crates/open-runo-sr`**: added `upscale_crop()` (enlarge small crops). The Arm NEON kernel landed in v1.1.1.
- Measurements, test procedure and open items are recorded in `pdf-r2l-rs/PORTING.md` (JA/EN). Completion is experimental and off by default.

## JSON処理の使用方針: `parse_secure_sonic`(速い型+安全)と`sonic-rs`は使える所では全て使う(2026-10-14、ユーザー指示)
正本は[`RS-JSON`](https://github.com/aon-co-jp/RS-JSON)(`README.md`・`PORTING.md`)。

**方針**: JSONを読む所は、使える所では**全て** `rust-json`(RS-JSON)の `parse_secure_sonic`(`fast`機能、結果は`sonic_rs::Value`)を使う。
`sonic-rs` を直接使える所(型付き構造体への読み込みなど)も同様。使えない所は `parse_secure`(結果は`serde_json::Value`)を使う。
外部から来る信頼できない入力(HTTP本文・ネットワーク越しのファイル・アップロード)は、**必ずサイズ・深さ・重複キーの上限つき**で読む。

**実測(7 MB・4万件)**: `serde_json` 200 ms / `parse_secure` 217 ms / **`parse_secure_sonic` 83 ms(約2.4倍速い、防御込み)** / `sonic-rs`そのまま 33 ms(約6倍、防御なし)。
`serde_json::Value`に読む限りSIMDでもほぼ速くならない(188 ms)。**速さの源は、結果を連続した領域に詰め込む値の型**。

**使える所 / 使えない所**
- ✅ 読み取り中心で、JSONを大きく・頻繁に読む所(キャッシュJSON、設定、APIの応答、ログ、インデックス)。
- ✅ 外部からの入力の入口(速さより防御が目的。`parse_secure`でも可)。
- ⚠️ `serde_json::Value`を関数間で受け渡している既存コードは、型を`sonic_rs::Value`に直す書き換えが要る(`JsonValueTrait`/`JsonContainerTrait`のメソッド)。書き換えずに`serde_json::Value`へ読むと速くならない。
- ❌ WASM(`rust_json::light`、依存ゼロ)、PHP/JavaScript側のJSON処理(Rustの部品は使えない)。
- 注意: 依存先の`sonic-rs`は内部で`unsafe`を使う(自前コードに`unsafe`は無い)。小さなJSON(数KB)では体感差が出ない。速度目的の書き換えは、実測で効く所から。

**導入**: `Cargo.toml`に `rust-json = { path = "../RS-JSON", features = ["fast"] }`。使用例は`RS-JSON/README.md`。

**このリポジトリでの適用状況**: 未適用(方針のみ)。コードの切り替えは、JSONを大きく・頻繁に読む所から実測して順に行う。

## JSON handling policy: use `parse_secure_sonic` (fast type + secure) and `sonic-rs` everywhere they can be used (2026-10-14, user instruction)
The source of truth is [`RS-JSON`](https://github.com/aon-co-jp/RS-JSON) (`README.md`, `PORTING.md`).

**Policy**: wherever JSON is read, use `parse_secure_sonic` from `rust-json` (RS-JSON, the `fast` feature; the result is `sonic_rs::Value`) **everywhere it can be used**.
Use `sonic-rs` directly where possible too (e.g. reading into typed structs). Where it cannot be used, use `parse_secure` (result: `serde_json::Value`).
Untrusted input from outside (HTTP bodies, files over the network, uploads) **must always be read with size / depth / duplicate-key limits**.

**Measured (7 MB, 40,000 records)**: `serde_json` 200 ms / `parse_secure` 217 ms / **`parse_secure_sonic` 83 ms (about 2.4x faster, with the defences)** / raw `sonic-rs` 33 ms (about 6x, no defences).
Reading into `serde_json::Value` gives almost no speed-up even with SIMD (188 ms). **The speed comes from the value type that packs the result into contiguous memory.**

**Where it can / cannot be used**
- ✅ Read-heavy code that parses large or frequent JSON (cache JSON, config, API responses, logs, indexes).
- ✅ Entrances for outside input (the goal there is defence rather than speed; `parse_secure` is fine).
- ⚠️ Existing code that passes `serde_json::Value` between functions needs its type changed to `sonic_rs::Value` (methods from `JsonValueTrait` / `JsonContainerTrait`). Reading into `serde_json::Value` without that change is not faster.
- ❌ WASM (`rust_json::light`, zero dependencies) and the JSON handling on the PHP / JavaScript side (Rust components cannot be used there).
- Note: the `sonic-rs` dependency uses `unsafe` internally (our own code has none). For small JSON (a few KB) the difference is not noticeable; do speed-motivated rewrites where measurements show a benefit.

**Adoption**: in `Cargo.toml`, `rust-json = { path = "../RS-JSON", features = ["fast"] }`. Example in `RS-JSON/README.md`.

**Status in this repository**: Not yet applied (policy only). Code will be switched over place by place, starting where measurements show large or frequent JSON reads.

## 2026-10-28 依存クレートの一括メジャー更新 / Major dependency update (2026-10-28)
**日本語**: 自動保守で残っていたメジャー更新を全て適用。`cargo check/test/clippy --workspace --all-features --all-targets` が通る(clippyは警告のみ・既存分)。
利用側(aruaru-search・realdata.pro・open-english/server)も新RPoemで`cargo check --all-targets`成功。
- 上げた版: base64 0.23 / clickhouse 0.15 / deadpool-redis 0.23 / hmac 0.13 / jsonschema 0.60 / jsonwebtoken 11.1 / lopdf 0.46 / opentelemetry・_sdk・-otlp 0.33 / tracing-opentelemetry 0.34 / rcgen 0.14 / redis 1.8 / reqwest 0.13 / serde_html_form 0.4 / sha1・sha2 0.11 / sonic-rs 0.5 / sqlx 0.9 / surrealdb 3 / syn 3 / thiserror 2 / tokio-tungstenite 0.30 / toml 1.1。`poem`・`poem-derive`・`indexmap`の厳密ピン(=3.1.0/=2.2.6)を解除(lopdf 0.46がindexmap 2.14+を要求)、`rust-version`を1.75→1.88。
- 破壊的変更への対応:
  - **sqlx 0.9**: feature `runtime-tokio-rustls`は廃止→`runtime-tokio`+`tls-rustls-ring-webpki`。`raw_sql`は動的文字列を`sqlx::AssertSqlSafe(..)`で包む必要あり(本コードは`sql_escape`済みのリテラルのみ)。
  - **clickhouse 0.15**: `Client::insert::<Row>(table)`が非同期(`.await`)かつ型引数必須。
  - **jsonwebtoken 11**: 暗号バックエンド必須→`features = ["use_pem","rust_crypto"]`(純Rust)。無いと実行時panic。
  - **reqwest 0.13**: `rustls-tls`→`rustls`、`.query()`は`query` feature。さらに**aws-lc-rsが引き込まれrustlsのプロバイダが2つ有効になる**ため、暗黙の既定(`ServerConfig::builder()`)は実行時panic→`builder_with_provider(ring)`で明示(`hyper_compat::load_tls_config`・`acme`)。利用側が`ServerConfig::builder()`を直接使うと同じ罠。
  - **jsonschema 0.60**: `ValidationError::instance_path`はフィールド→メソッド。 **hmac 0.13**: `new_from_slice`は`KeyInit`トレイト。 **rcgen 0.14**: `CertifiedKey.key_pair`→`signing_key`。 **tokio-tungstenite 0.30**: `Message::Binary`は`Bytes`。
  - 副次: `edfs`機能の`decode_event`が`sonic_rs::Error`を返す型不一致(更新前から全機能ビルドで失敗)を`serde_json::from_str`で修正。
- 注意: appserver/gateway のテストは先に`cargo build -p open-runo-appserver --bin open-runo-dummy-http-server`が必要。
- 未対応: なし(`serde`/`tokio`等の同一メジャー内の更新は`cargo update`で追従済み)。
**English**: Applied all pending major bumps (list above); workspace build/test/clippy pass with all features and the three consumers still `cargo check`. Notable breaking changes: sqlx 0.9 (`runtime-tokio`+`tls-rustls-ring-webpki`, `AssertSqlSafe` for dynamic `raw_sql`), clickhouse 0.15 (async typed `insert::<Row>`), jsonwebtoken 11 (must pick `rust_crypto`/`aws_lc_rs`), reqwest 0.13 (`rustls` feature, `query` feature, and it pulls in aws-lc-rs so rustls has two providers — implicit `ServerConfig::builder()` now panics; pass ring explicitly), jsonschema/hmac/rcgen/tungstenite API tweaks. Pins on poem/indexmap removed and `rust-version` raised to 1.88.


多言語要約(10言語)は `docs/i18n/<言語コード>/PORTING.md`。

## API 方針: REST 全廃・gRPC / Connect へ (2026-11-04)

### 日本語
- **REST は全廃する。新しい API は REST で作らない。** ブラウザや `curl` から呼ぶ API は **Connect の単純呼び出し**(`POST /<パッケージ>.<Service>/<Method>`、`content-type: application/json`、本文は JSON)、サービス間は **gRPC / gRPC-web**(RPoem の管理API `open_runo.admin.v1.Admin` と同じ流儀)。ストリーミングは、必要になってから。
- **既存の REST は、触るときに順に移す。** 移したら旧 REST の経路は消す。移行が終わるまで残るが、**新規には追加しない**。
- **JSON は RS-JSON に統一**する(読み込みは `from_str_secure` / `parse_secure` でサイズ・深さ・重複キーの上限つき、書き出しは `rust_json::fast`)。ブラウザの WASM に `prost` は入れない(サイズが増えるため)。
- **例外(自分たちの API ではなく、相手の規格が決めているもの)**: ACME、OAuth/OIDC のコールバック、第三者(GitHub など)からの Webhook、ヘルスチェック、静的ファイル配信、OTLP など。これらは規格どおりの HTTP を受ける。
- 参考実装: `RPoem`(`open_runo.admin.v1.Admin`)、`open-easy-web`(`server/src/connect.rs`、`proto/open_easy_web/admin/v1/admin.proto`)。
- ※ このリポジトリのコードが既に方針どおりかどうかは、各リポジトリの現状による。この節は方針の記録であり、コードの変更を意味しない。

### English
- **REST is being abolished. New APIs are not built as REST.** APIs called from browsers or `curl` use **Connect unary calls** (`POST /<package>.<Service>/<Method>`, `content-type: application/json`, JSON body); service-to-service calls use **gRPC / gRPC-web** (the same style as RPoem's admin API `open_runo.admin.v1.Admin`). Streaming only when it is actually needed.
- **Existing REST endpoints are migrated one by one, when they are touched.** The old REST route is removed once migrated. Until migration is finished they remain, but **no new ones are added**.
- **JSON is unified on RS-JSON**: reading with `from_str_secure` / `parse_secure` (size, depth and duplicate-key limits), writing with `rust_json::fast`. `prost` is not put into browser WASM (it would grow the binary).
- **Exceptions (shapes dictated by someone else's standard, not our own API)**: ACME, OAuth/OIDC callbacks, webhooks from third parties (e.g. GitHub), health checks, static file serving, OTLP, and so on. These accept plain HTTP as the standard requires.
- Reference implementations: `RPoem` (`open_runo.admin.v1.Admin`) and `open-easy-web` (`server/src/connect.rs`, `proto/open_easy_web/admin/v1/admin.proto`).
- Note: whether the code in a given repository already follows this policy depends on that repository. This section records the policy; it does not mean the code has been changed.

### Deutsch (KI-Übersetzung, redaktionell überarbeitet; noch nicht von Muttersprachlern geprüft)
- **REST wird vollständig abgeschafft; neue APIs entstehen nicht mehr als REST.** APIs, die aus dem Browser oder per `curl` aufgerufen werden, verwenden **unäre Connect-Aufrufe** (`POST /<Paket>.<Service>/<Methode>`, `content-type: application/json`, JSON-Body). Zwischen Diensten kommen **gRPC bzw. gRPC-web** zum Einsatz, im selben Stil wie die Admin-API von RPoem (`open_runo.admin.v1.Admin`). Streaming wird erst ergänzt, wenn es tatsächlich gebraucht wird.
- **Bestehende REST-Endpunkte werden schrittweise umgestellt, und zwar immer dann, wenn man ohnehin an ihnen arbeitet.** Sobald ein Endpunkt umgestellt ist, wird die alte REST-Route entfernt. Bis zum Abschluss der Migration bleiben die übrigen bestehen, es werden aber **keine neuen mehr hinzugefügt**.
- **JSON wird einheitlich über RS-JSON verarbeitet**: Gelesen wird mit `from_str_secure` bzw. `parse_secure` (mit Obergrenzen für Größe, Verschachtelungstiefe und doppelte Schlüssel), geschrieben mit `rust_json::fast`. `prost` wird nicht in das Browser-WASM aufgenommen, da es die Datei vergrößern würde.
- **Ausnahmen (Schnittstellen, deren Form ein fremder Standard vorgibt und die nicht zu unserer eigenen API gehören)**: ACME, OAuth/OIDC-Callbacks, Webhooks von Drittanbietern (z. B. GitHub), Health-Checks, die Auslieferung statischer Dateien, OTLP und Ähnliches. Diese nehmen, wie vom jeweiligen Standard verlangt, gewöhnliches HTTP entgegen.
- Referenzimplementierungen: `RPoem` (`open_runo.admin.v1.Admin`) und `open-easy-web` (`server/src/connect.rs`, `proto/open_easy_web/admin/v1/admin.proto`).
- Hinweis: Ob der Code eines einzelnen Repositorys diese Richtlinie bereits erfüllt, hängt vom jeweiligen Repository ab. Dieser Abschnitt dokumentiert lediglich die Richtlinie; er besagt nicht, dass der Code schon angepasst wurde.

### Русский (перевод выполнен с помощью ИИ и отредактирован; носителями языка пока не проверен)
- **От REST мы полностью отказываемся; новые API больше не создаются в стиле REST.** API, к которым обращаются из браузера или через `curl`, используют **унарные вызовы Connect** (`POST /<пакет>.<Service>/<Method>`, `content-type: application/json`, тело запроса — JSON). Для взаимодействия между сервисами применяются **gRPC и gRPC-web** — в том же стиле, что и административный API RPoem (`open_runo.admin.v1.Admin`). Потоковая передача данных добавляется лишь тогда, когда в ней действительно возникает необходимость.
- **Существующие REST-эндпоинты переводятся поэтапно — по мере того, как с ними приходится работать.** После перевода старый REST-маршрут удаляется. Пока миграция не завершена, оставшиеся эндпоинты сохраняются, но **новые не добавляются**.
- **Работа с JSON унифицирована на основе RS-JSON**: чтение — через `from_str_secure` или `parse_secure` (с ограничениями на размер, глубину вложенности и повторяющиеся ключи), запись — через `rust_json::fast`. `prost` в браузерный WASM не включается, поскольку это увеличило бы размер файла.
- **Исключения (интерфейсы, форму которых определяет чужой стандарт, а не наш собственный API)**: ACME, обратные вызовы OAuth/OIDC, вебхуки сторонних сервисов (например, GitHub), проверки работоспособности, раздача статических файлов, OTLP и тому подобное. Такие интерфейсы принимают обычный HTTP, как того требует соответствующий стандарт.
- Эталонные реализации: `RPoem` (`open_runo.admin.v1.Admin`) и `open-easy-web` (`server/src/connect.rs`, `proto/open_easy_web/admin/v1/admin.proto`).
- Примечание: соответствует ли код конкретного репозитория этой политике, зависит от самого репозитория. Данный раздел лишь фиксирует политику и не означает, что код уже изменён.

## API 方針の補足: 用語の整理と使い分け (2026-11-04)

### 日本語
**1つに統一するものではなく、別の層の話です。** 「REST の代わり」になるのは Connect・gRPC・GraphQL だけで、WASM は画面を作る技術です。

| 用語 | 何か | REST の代わりか |
|---|---|---|
| REST | URL と HTTP メソッドで操作を表す API の作り方 | 全廃する側 |
| **Connect** | `POST /サービス名/メソッド名` に JSON を送る形式。`curl` でもブラウザの `fetch` でも呼べる | はい(**採用**) |
| **gRPC** | 同じ「サービス/メソッド」の考え方を、バイナリ(protobuf)と HTTP/2 で行う形式。速く、ストリーミングも得意 | はい(**サービス同士に採用**) |
| gRPC-web | gRPC のブラウザ向けの変形版 | はい(必要なら) |
| GraphQL | 欲しい項目を問い合わせる形式 | 一部だけ(**問い合わせ(読み取り)専用**) |
| WASM | Rust をブラウザの中で動かす仕組み | いいえ(画面を作る技術) |

- **ブラウザの画面や `curl` から呼ぶ API は Connect**(POST、JSON を RS-JSON で読み書き)。ブラウザの WASM は素の gRPC を話せず、Connect なら `fetch` のままで、`prost` を足さずに済む(画面のサイズも増えない)。
- **サービス同士の通信(サーバー間、重い処理、ストリーミング)は gRPC。** 多くは Connect で足りるので、ストリーミングが必要になった所だけ gRPC に上げる。
- **GraphQL は、まとめて問い合わせたいときの読み取り専用。** 書き込み(操作)には使わない。
- **WASM は画面を作る技術として、そのまま使う。** 画面から呼ぶ API が Connect になっただけ。
- 外部の規格が形を決めるもの(ACME、OAuth のコールバック、第三者の Webhook、ヘルスチェック、静的ファイル、OTLP)は、規格どおりの HTTP のまま。
- **迷ったら Connect。**

### English
**These are not alternatives to unify; they sit at different layers.** Only Connect, gRPC and GraphQL can replace REST; WASM is a way to build the UI.

| Term | What it is | Replaces REST? |
|---|---|---|
| REST | API style that expresses operations through URLs and HTTP methods | The thing being abolished |
| **Connect** | `POST /Service/Method` with a JSON body; callable from `curl` and from a browser `fetch` | Yes (**adopted**) |
| **gRPC** | The same service/method idea over binary (protobuf) and HTTP/2; fast, good at streaming | Yes (**for service-to-service**) |
| gRPC-web | A browser-friendly variant of gRPC | Yes (if needed) |
| GraphQL | A query language: ask for exactly the fields you want | Partly (**read-only queries**) |
| WASM | Runs Rust inside the browser | No (a UI technology) |

- **APIs called from browser screens or `curl` use Connect** (POST, JSON read and written with RS-JSON). Browser WASM cannot speak plain gRPC; with Connect it stays a `fetch`, and `prost` is not needed (the binary does not grow).
- **Service-to-service calls (server to server, heavy work, streaming) use gRPC.** Connect is enough in most places; move a call to gRPC only where streaming is actually needed.
- **GraphQL is read-only**, for queries that gather several pieces of data. It is not used for writes (operations).
- **WASM stays as the way to build the UI.** The API it calls simply became Connect.
- Interfaces whose shape an outside standard dictates (ACME, OAuth callbacks, third-party webhooks, health checks, static files, OTLP) stay plain HTTP.
- **When in doubt, use Connect.**

### Deutsch (KI-Übersetzung, sprachlich überarbeitet; noch nicht von Muttersprachlern geprüft)
**Es geht nicht darum, eine einzige Technik auszuwählen – die Begriffe liegen auf unterschiedlichen Ebenen.** Als Ersatz für REST kommen nur Connect, gRPC und GraphQL in Frage; WASM dagegen ist eine Technik, mit der die Benutzeroberfläche gebaut wird.

| Begriff | Was ist das? | Ersatz für REST? |
|---|---|---|
| REST | Ein API-Stil, bei dem Operationen über URLs und HTTP-Methoden ausgedrückt werden | wird abgeschafft |
| **Connect** | `POST /Dienst/Methode` mit JSON im Body; aufrufbar mit `curl` ebenso wie mit `fetch` im Browser | ja (**gewählt**) |
| **gRPC** | Dasselbe Prinzip aus Dienst und Methode, aber über binäres protobuf und HTTP/2; schnell und gut für Streaming geeignet | ja (**zwischen Diensten**) |
| gRPC-web | Eine für den Browser angepasste Variante von gRPC | ja (bei Bedarf) |
| GraphQL | Abfragesprache, mit der man genau die benötigten Felder anfordert | teilweise (**nur für lesende Abfragen**) |
| WASM | Führt Rust-Code direkt im Browser aus | nein (Technik für die Oberfläche) |

- **APIs, die von Browseroberflächen oder per `curl` aufgerufen werden, verwenden Connect** (POST; JSON wird mit RS-JSON gelesen und geschrieben). WASM im Browser kann kein reines gRPC sprechen. Mit Connect bleibt es bei einem gewöhnlichen `fetch`, und `prost` ist nicht nötig – die WASM-Datei wird dadurch nicht größer.
- **Für die Kommunikation zwischen Diensten (Server zu Server, rechenintensive Aufgaben, Streaming) wird gRPC eingesetzt.** In den meisten Fällen reicht Connect aus; auf gRPC wird nur dort umgestellt, wo Streaming tatsächlich gebraucht wird.
- **GraphQL dient ausschließlich lesenden Abfragen**, wenn mehrere Informationen gebündelt abgefragt werden sollen. Für Schreibzugriffe (Operationen) wird es nicht verwendet.
- **WASM bleibt die Technik für die Oberfläche.** Geändert hat sich lediglich, dass die aufgerufene API nun auf Connect beruht.
- Schnittstellen, deren Form durch einen externen Standard festgelegt ist (ACME, OAuth-Callbacks, Webhooks von Drittanbietern, Health-Checks, statische Dateien, OTLP), bleiben gewöhnliches HTTP.
- **Im Zweifel: Connect.**

### Русский (перевод выполнен с помощью ИИ и литературно отредактирован; носителями языка пока не проверен)
**Это не варианты, из которых нужно выбрать один, а технологии разных уровней.** Заменить REST могут только Connect, gRPC и GraphQL; WASM же — это способ создания пользовательского интерфейса.

| Термин | Что это | Заменяет REST? |
|---|---|---|
| REST | Стиль построения API, при котором операции выражаются через URL и HTTP-методы | отменяется |
| **Connect** | `POST /Сервис/Метод` с телом в формате JSON; вызывается как из `curl`, так и из браузерного `fetch` | да (**выбран**) |
| **gRPC** | Тот же принцип «сервис — метод», но поверх двоичного формата protobuf и HTTP/2; работает быстро и хорошо подходит для потоковой передачи | да (**для связи между сервисами**) |
| gRPC-web | Адаптированный для браузера вариант gRPC | да (при необходимости) |
| GraphQL | Язык запросов: клиент запрашивает ровно те поля, которые ему нужны | частично (**только запросы на чтение**) |
| WASM | Позволяет выполнять код на Rust прямо в браузере | нет (технология для интерфейса) |

- **API, которые вызываются из браузерных интерфейсов или через `curl`, строятся на Connect** (POST; JSON читается и записывается через RS-JSON). Браузерный WASM не умеет работать с «чистым» gRPC. С Connect достаточно обычного `fetch`, а `prost` не требуется — размер WASM-файла не увеличивается.
- **Для обмена между сервисами (сервер — сервер, ресурсоёмкие задачи, потоковая передача) применяется gRPC.** В большинстве случаев достаточно Connect; на gRPC переходят только там, где потоковая передача действительно необходима.
- **GraphQL используется только для чтения**, когда нужно получить сразу несколько связанных данных. Для записи (операций) он не применяется.
- **WASM по-прежнему остаётся технологией для интерфейса.** Изменилось лишь то, что вызываемый им API теперь построен на Connect.
- Интерфейсы, форма которых задана внешним стандартом (ACME, обратные вызовы OAuth, вебхуки сторонних сервисов, проверки работоспособности, статические файлы, OTLP), остаются обычным HTTP.
- **Если есть сомнения — выбирайте Connect.**

## サーバーストリーミング(2026-11-07)

**対応範囲**: サーバーストリーミング(1回の要求 → 複数の応答)。クライアント/双方向ストリーミングは fetch の制約で不可(必要なら WebSocket を別途)。

| 方式 | Content-Type | 1件の形 | 終端 |
|---|---|---|---|
| Connect(JSON) | `application/connect+json` | 5バイトのエンベロープ(フラグ+長さ)+JSON | フラグ`0x02`の終端フレーム(`{}`=正常、`{"error":{...}}`=失敗) |
| Connect(proto) | `application/connect+proto` | 同上+proto | 同上 |
| gRPC-web[-text] | `application/grpc-web[-text]` | エンベロープ+proto | フラグ`0x80`のトレーラーフレーム(`grpc-status`) |
| gRPC | `application/grpc` | エンベロープ+proto | HTTP/2トレーラー |

- 実装は`open-runo-connect-server`(`Reply::Stream` = 失敗しないストリーム、`Reply::TryStream` = 途中で`Err(Status)`を返せるストリーム)。
  管理APIでは`WatchEvents`(スキーマ登録・履歴件数のイベント)が使う。
- **エラーは必ず終端フレームで返す**: ストリーム開始前の失敗(認証など)も、Connect/gRPC-webはHTTP 200+終端フレームの`error`。途中の失敗(`Err`)も同じ形で閉じる。
- **上限**(`StreamLimits`、`handle_with`で変更、`handle`は既定値): 1本あたり最大100万件(超過=`RESOURCE_EXHAUSTED`)・最長1時間(超過=`DEADLINE_EXCEEDED`)。超過しても終端フレームで閉じる。
  同時ストリーム数はHTTP/2の1接続あたり128(hyperの設定)。接続数の制限は前面側の責務。
- **切断**: クライアントが切ると応答ボディが捨てられ、元のストリームとその資源も解放される(試験済み)。
- **使い分け**: 一回で済む操作=単項。サーバーから**変化を押し出したい**(イベント通知・長い処理の進捗・トークン逐次生成)ときだけストリーム。ポーリング・SSE・WebSocketの置き換え先。
- 試験: `open-runo-connect-server`(途中エラー3方式・上限2種・切断解放)、`open-runo-router`の`connect_json_watch_events_over_real_http`(実TCP、認証なし=終端エラー、鍵あり=イベント受信、切断後も健全)と`real_http2_server_streaming_delivers_events`(実HTTP/2)。
- `curl`の例: `curl -N -X POST $URL/open_runo.admin.v1.Admin/WatchEvents -H 'content-type: application/connect+json' -H "x-api-key: $KEY" --data-binary $'\x00\x00\x00\x00\x02{}'`(先頭5バイトがエンベロープ)。

## 2026-11-08 appserver系RESTをConnect/gRPCへ(REST全廃の続き)
- `/admin/appserver-processes`(+`/stop-all`・`/:host`・`/:host/stop`・`/:host/stop-graceful`・`/:host/restart`)と `/admin/appserver-tenants`(+`/:host`)の**計10本を削除**し、`open_runo.admin.v1.Admin` のRPC `AppServerRegisterProcess`/`ListProcesses`/`ProcessStatus`/`StopProcess`/`StopProcessGraceful`/`RestartProcess`/`StopAll`/`UpsertTenant`/`ListTenants`/`RemoveTenant` に移した(gRPC・gRPC-web・Connect共通。要求・応答は `google.protobuf.Struct`=旧RESTと同じJSON形、認証は他のAdminと同じ `x-api-key`/セッション)。
- 実装: ルーターに拡張フック `grpc_admin::AdminExt`(`AdminGrpc::with_ext`)を追加し、ゲートウェイの `appserver_admin.rs`(`AppServerAdmin`)が `SupervisedTenantRegistry` を操作する。旧 `appserver_processes.rs`/`appserver_tenants.rs` は削除。
- 呼び出し元: `open-easy-web`(`server/src/appserver_registration.rs`、PoemCosmoTauri登録)を `AppServerUpsertTenant` に変更。それ以外に旧パスの呼び出しは無い(`open-web-server/CLAUDE.md` の記述は過去の記録)。
- 例: `curl -X POST -H 'x-api-key: KEY' -H 'content-type: application/json' -d '{"host":"a.example.jp","backend_addr":"127.0.0.1:9001"}' http://HOST/open_runo.admin.v1.Admin/AppServerUpsertTenant`
- 試験: WSLで `cargo test -p open-runo-router --lib`(180件成功)・`-p open-runo-gateway --lib appserver_admin`(3件成功)。実プロセスを起動する旧E2E(ダミーHTTPサーバー起動)は、レジストリ自体のテスト(`open-runo-appserver`)で担保しており、Connect経由の実起動の通しは未実施。
- 本番(VPS)へは未反映(RPoemゲートウェイはVPSで未稼働)。

## 2026-11-08 VPS反映の見送り
- `781c824` の `Admin/DbMutate` 等は、VPSにRPoem/open-cosmoゲートウェイの稼働が無い(systemd unit・プロセスとも無し、`rpoem-web` は静的紹介ページのみ)ため反映対象なし。呼び出し元 open-web-server `Ledger`(`OPEN_RUNO_API_KEY`)と同時に、ゲートウェイを稼働させる時点で反映する。

## 2026-11-09 VPSでゲートウェイを常駐化
- `open-runo-gateway`(`9c7a127`時点、WSL Ubuntu-24.04でリリースビルド、glibc 2.39)を `/opt/open-runo-gateway/` に配置し、systemdユニット `open-runo-gateway.service`(enable済み、`127.0.0.1:8140` のみ、`OPEN_RUNO_ENV=production`、状態は `/var/lib/open-runo-gateway`)で起動。
- 実測: active、`/healthz`=200、`Admin/DbMutate` 鍵なし=401、旧 `/internal/db/mutate`=404。鍵つきの200は2026-11-08のdevビルド実測のみで、本番ユニットでは未実施。
- **未了**: open-web-server の `OPEN_RUNO_ENDPOINT`・`OPEN_RUNO_API_KEY` は未設定(現状 Ledger は本番で使われていない)。設定すると前面サーバーの再起動が要るため見送り。鍵の発行・配線は別途。

## 2026-11-09 ゲートウェイの鍵を本物にした / Service key made real
- **発見**: 鍵の登録簿が空だと `check_api_key` は何でも通す(開発モード)。常駐直後のゲートウェイは `x-api-key: zzz` でも認証を通った=`Admin/DbMutate` の「x-api-key必須」は実質空だった。
- **修正**: `KeyGuardian::seed_service_key`(平文は保存せずSHA-256のみ、32文字未満は拒否、期限なし)と、ゲートウェイ起動時の環境変数 `OPEN_RUNO_SERVICE_API_KEY` での登録を追加。テスト `seeded_service_key_ends_permissive_mode` を追加(keyring 6件成功)。
- **VPS**: 鍵は `/etc/open-runo-gateway.env`(600)、ユニットは `EnvironmentFile` で読む。実測: 鍵なし401・誤った鍵401・正しい鍵で書き込み200(`committed:true`)。旧バイナリは `.before-key-20261109`。
- **未了(接続を見送った理由)**: 正しい鍵で書き込んでも、ゲートウェイの DB が InMemory のため `db_commit_id` が `null` で、open-web-server の `Ledger::forward_once` は「db_commit_id が無い」を失敗として扱う。つなぐには、ゲートウェイを aruaru-db バックエンド(コミットID付き)で動かす設計と、`OPEN_RUNO_ENDPOINT`/`OPEN_RUNO_API_KEY` の設定+前面サーバー再起動が要る。現状は接続しても全書き込みが失敗するため、前面サーバーは再起動していない。

## 2026-11-09 ゲートウェイのaruaru-dbバックエンド(作業途中の記録)
- `OPEN_RUNO_DB_URL`(aruaru-dbのpgwire)を指定するとゲートウェイが永続DBで動く(未設定はメモリ上)。`open-runo-gateway` に `open-runo-db`(feature `aruaru`)を追加。
- VPSに専用の `open-runo-aruaru-db.service`(pg 15434・gql 14002、`ARUARU_USERS` は `/etc/open-runo-aruaru-db.env`)を新設済み。ゲートウェイの `/etc/open-runo-gateway.env` には**今は `OPEN_RUNO_DB_URL` を入れていない**(旧メモリ版で稼働中、ロールバック済み)。
- **原因を特定**: aruaru-db のSQLパーサー(`aruaru-query/src/parser.rs` の `parse_insert`)が値を単純に `,` で割り `)` を探すため、JSONの値(`,` や `)` を含む)で「4 columns but 8 values」になる。引用符を考慮した分割へ直す修正と単体テストを作成(WSLの `~/aruaru-db-src`、未コミット/未push、aruaru-query の parser テスト17件成功)。
- **2026-11-14 結果(完了)**: 修正版 `aruaru-server` を `/opt/open-runo-aruaru-db/aruaru-server` に入れ(専用インスタンスのみ。本番 `aruaru-server`(5433)は無変更)、ゲートウェイの `/etc/open-runo-gateway.env` に `OPEN_RUNO_DB_URL` を設定して再起動。実測: 誤キー401、正キーの書き込み200で `db_commit_id` が非null、`api_keys` が永続化されゲートウェイ再起動後も鍵が有効、JSONの値(`,` `)` `'` を含む)が欠落なく保存されることを読み戻しで確認。3サービスとも稼働。
- **未了**: (1) aruaru-db の修正はブランチ `fix/insert-quoted-values`(`1590750`)のみで main 未マージ・本番 aruaru-server には未適用。(2) open-web-server は未接続(`OPEN_RUNO_ENDPOINT=http://127.0.0.1:8140` と `OPEN_RUNO_API_KEY` を設定して前面を再起動する必要があり、全サイトに影響するため要承認)。
