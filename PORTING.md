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

## 2026-11-15 実プロセスE2E再確認(open-web-server → RPoem appserver)
`open-web-server`(Connect版)から`e2e_stub_app`(RPoem)へHostルーティングで3回連続 HTTP 200 + `X-Served-By: RPoem-appserver` を確認。
注意: スタブが登録するテナントは`e2e-rpoem.test`のみ。`domains.toml`のhostを別名(例 shop.test)にするとHost不一致でスタブが接続を閉じ502になる(不具合ではなく試験設定ミス)。`domains.toml`には`db_uri`が必須。
*Re-verified: 3/3 requests returned 200 with X-Served-By; a 502 appears only if the Host does not match the stub's tenant `e2e-rpoem.test`.*

## 2026-11-15 別アカウントでの再開用メモ(REST全廃のVPS反映・ゲートウェイDB化)

### 日本語
- **到達点**: VPSで RPoemゲートウェイ(`open-runo-gateway.service`、127.0.0.1:8140)が稼働。鍵は `OPEN_RUNO_SERVICE_API_KEY`(`/etc/open-runo-gateway.env`、値は書かない)、永続化は専用 aruaru-db(`open-runo-aruaru-db.service`、pg 15434、`/opt/open-runo-aruaru-db/aruaru-server`=修正版)。有効な鍵の書き込みは200で `db_commit_id` が入り、`api_keys` はゲートウェイ再起動後も有効。本番 `aruaru-server`(5433)は無変更。open-web-server は `OPEN_RUNO_API_KEY` を設定済みで接続(2026-11-14)。open-raid-z の Connect 版バイナリもVPS反映済み。
- **コード**: RPoem `keyring.rs` に `seed_service_key`、ゲートウェイ `main.rs` に `OPEN_RUNO_SERVICE_API_KEY` / `OPEN_RUNO_DB_URL` の読み込み。aruaru-db の `parse_insert` 修正(`,` `)` `'` を含む値)はブランチ `fix/insert-quoted-values`(`1590750`)のみで **main 未マージ・本番 aruaru-server 未適用**。
- **未了**: (1) 上記 aruaru-db 修正のレビュー→main マージ→本番反映(本番は承認後)。(2) Android 実機での変換確認(別セッション rest-android 担当)。(3) 手元の `F:\open-runo-gateway.new`・`F:\aruaru-server.new` は削除(安全チェックで自動削除不可、手動で)。(4) open-cosmo 本体の本番起動は判断待ち。
- **落とし穴**: ビルドは VPS ではなく WSL `Ubuntu-24.04`(glibc 2.39)で行い、PowerShell から `wsl` を呼ぶ(Git Bash は `/mnt/...` を書き換える)。差し替えは バックアップ→入替→検証→自動ロールバック。systemd の起動回数制限に当たったら `systemctl reset-failed`。秘密の値は画面・コミットに出さない。書き換え済みリポジトリで `git pull` しない。
- **再開用メッセージ(コピーして使う)**: 「`F:\RPoem` の `PORTING.md` 末尾『2026-11-15 別アカウントでの再開用メモ』を読み、(1) aruaru-db の `fix/insert-quoted-values`(WSL `~/aruaru-db-src`)をレビューして main へマージするか判断、(2) ゲートウェイ・専用DB・open-web-server の稼働を `ssh conoha` で読み取り確認、(3) 未了項目を順に進めて。対象リポジトリ: RPoem・aruaru-db・open-web-server・open-raid-z・open-easy-web。本番の変更は事前に確認し、結果は日英で PORTING.md に記録してpush。」

### English
- **Status**: On the VPS the RPoem gateway (`open-runo-gateway.service`, 127.0.0.1:8140) is live. Auth uses `OPEN_RUNO_SERVICE_API_KEY` (`/etc/open-runo-gateway.env`; never write the value), persistence is a dedicated aruaru-db (`open-runo-aruaru-db.service`, pg 15434, fixed binary at `/opt/open-runo-aruaru-db/aruaru-server`). A write with a valid key returns 200 with a `db_commit_id`; `api_keys` survive a gateway restart. Production `aruaru-server` (5433) is untouched. open-web-server has `OPEN_RUNO_API_KEY` set and is connected (2026-11-14). The Connect build of open-raid-z is deployed too.
- **Code**: `seed_service_key` in RPoem `keyring.rs`; the gateway `main.rs` reads `OPEN_RUNO_SERVICE_API_KEY` / `OPEN_RUNO_DB_URL`. The aruaru-db `parse_insert` fix (values containing `,` `)` `'`) exists only on branch `fix/insert-quoted-values` (`1590750`): **not merged to main, not applied to production aruaru-server**.
- **Open**: (1) review/merge that aruaru-db fix and deploy to production (production only after approval). (2) Real-device Android conversion check (owned by the rest-android session). (3) Delete local `F:\open-runo-gateway.new` and `F:\aruaru-server.new` by hand (auto-delete was blocked). (4) Running open-cosmo itself in production awaits a decision.
- **Pitfalls**: build in WSL `Ubuntu-24.04` (glibc 2.39), not on the VPS, calling `wsl` from PowerShell (Git Bash rewrites `/mnt/...`). Swap = backup → replace → verify → auto-rollback. After a systemd start-limit failure run `systemctl reset-failed`. Never print secrets. Never `git pull` in rewritten repositories.
- **Resume message (copy and paste)**: "Read the end of `F:\RPoem\PORTING.md` ('2026-11-15 resume notes'). (1) Review the aruaru-db branch `fix/insert-quoted-values` (WSL `~/aruaru-db-src`) and decide on merging to main; (2) check by read-only `ssh conoha` that the gateway, the dedicated DB and open-web-server are healthy; (3) work through the open items in order. Repositories: RPoem, aruaru-db, open-web-server, open-raid-z, open-easy-web. Confirm before any production change and record results in PORTING.md in Japanese and English, then push."

## 2026-11-15 RPoem「分身の術」ホスティング(別アカウントでの再開用) / Bunshin hosting (resume on another account)

### 日本語
RPoem「分身の術」ホスティング(1VPSに1回入れ、全サイトを分身で非同期・マルチスレッド収容)の仕様・到達点・未了・**再開用メッセージ**は、正本の `open-easy-web/CLAUDE.md` と `open-easy-web/PORTING.md` 末尾の同名節、および `open-easy-web/docs/RPOEM_BUNSHIN_HOSTING_SPEC.md` にある。このリポジトリの該当部分: `crates/open-runo-appserver/src/fleet.rs`(`SiteFleet`、feature `fleet`、コミット `2b50b3e`、push済み)と `tests/fleet.rs`。

### English
The spec, status, open items and the **resume message** for RPoem "bunshin" hosting (install once per VPS, host every site with shadow-clone workers, async and multi-threaded) live in the same-named section at the end of `open-easy-web/CLAUDE.md` and `open-easy-web/PORTING.md`, and in `open-easy-web/docs/RPOEM_BUNSHIN_HOSTING_SPEC.md`. This repository's part: `crates/open-runo-appserver/src/fleet.rs` (`SiteFleet`, feature `fleet`, commit `2b50b3e`, pushed) and `tests/fleet.rs`.

## 2026-11-17 追記: 実プロセス通し確認の完了・再開用メッセージ更新(別アカウントでの再開用) / Real-process E2E done, resume message updated (resume on another account)

### 日本語
実プロセス通し確認(起動・転送・503上限・crash復帰)は完了。詳細・限界・次にやること・**最新の再開用メッセージ**は、正本の `open-easy-web/CLAUDE.md` と `open-easy-web/PORTING.md` 末尾の同名節にある。このリポジトリの該当部分: `open-runo-appserver` の `SiteFleet`(`fleet.rs`)は、上記の通し確認で実プロセスの起動・crash復帰を確認済み。未了は hyper 転送(`proxy.rs`)と段階3。

### English
The real-process E2E (start, forward, 503 limit, crash recovery) is done. Details, limits, next steps and the **latest resume message** are in the same-named section at the end of `open-easy-web/CLAUDE.md` and `open-easy-web/PORTING.md`. This repository's part: `SiteFleet` (`fleet.rs`) in `open-runo-appserver` was exercised by the E2E above (real process start, crash recovery). Open: hyper forwarding (`proxy.rs`) and stage 3.

## 2026-11-17 別アカウントでの再開用メモ(open-web-server⇔RPoem 実プロセスE2E・aruaru-vpn) / Resume memo for another account

### 日本語
- **到達点**: (1) open-web-server(Connect版)→RPoem `e2e_stub_app` の実プロセスE2Eを再確認(Host=`e2e-rpoem.test`、3/3で200+`X-Served-By: RPoem-appserver`)。502は`domains.toml`のhostがスタブのテナントと不一致だった試験設定ミスで、不具合ではない(`db_uri`も必須)。(2) aruaru-vpn: 証明書クローン・Mux・Chrome風フィンガープリント(RPoemの共有クレート`open-runo-tls-fingerprint`)まで実装・push済み。
- **未了**: aruaru-vpn固定key_shareのプライバシー面のトレードオフ、翻訳のネイティブ検証(人手)、`PORTING.md` §20ロードマップ。RPoemの未追跡`RPoem/`・`target-crossui/`は別作業なのでコミットしない。
- **注意**: cargoはPATHに`%USERPROFILE%\.cargo\bin`が必要。`cargo fmt --all`禁止(`-p <crate>`)。書き換え済みリポジトリで`git pull`しない。VPSは`/root/open-web-server`・`/root/RPoem`を`git pull --ff-only`(`/root/mirror-cache`は存在しない)。
- **再開用メッセージ(そのまま貼り付け)**: 「再開するリポジトリは `F:\aruaru-vpn`・`F:\RPoem`・`F:\open-web-server`・`F:\runo`。各`PORTING.md`末尾『2026-11-17 別アカウントでの再開用メモ』を読み、未了(aruaru-vpnのプライバシー面・翻訳検証・§20)から着手してください。open-web-server→RPoemのE2Eは`RPoem/crates/open-runo-appserver/examples/e2e_stub_app.rs`をHost=`e2e-rpoem.test`で再現できます。」

### English
- **Status**: (1) The real-process E2E open-web-server (Connect) → RPoem `e2e_stub_app` was re-verified (Host `e2e-rpoem.test`, 3/3 returned 200 + `X-Served-By: RPoem-appserver`). The earlier 502 was a test-setup mistake (host in `domains.toml` did not match the stub's tenant; `db_uri` is required), not a bug. (2) aruaru-vpn: cert cloning, Mux and the Chrome-like fingerprint (shared RPoem crate `open-runo-tls-fingerprint`) are implemented and pushed.
- **Open**: privacy trade-off of aruaru-vpn's fixed key_share, native-speaker translation review (human), `PORTING.md` §20 roadmap. Do not commit RPoem's untracked `RPoem/` and `target-crossui/` (other work).
- **Pitfalls**: cargo needs `%USERPROFILE%\.cargo\bin` on PATH; never `cargo fmt --all` (use `-p <crate>`); never `git pull` in rewritten repos. On the VPS, `git pull --ff-only` in `/root/open-web-server` and `/root/RPoem` (`/root/mirror-cache` does not exist).
- **Resume message (paste as-is)**: "Resume in `F:\aruaru-vpn`, `F:\RPoem`, `F:\open-web-server`, `F:\runo`. Read the section '2026-11-17 別アカウントでの再開用メモ' at the end of each `PORTING.md`, and start with the open items (aruaru-vpn privacy trade-off, translation review, §20). The open-web-server→RPoem E2E can be reproduced with `RPoem/crates/open-runo-appserver/examples/e2e_stub_app.rs` using Host `e2e-rpoem.test`."

## 2026-11-17 /internal/db/mutate を Admin/DbMutate へ移行(REST全廃の仕上げ) / Moved /internal/db/mutate to Admin/DbMutate

**日本語**: 旧 `POST /internal/db/mutate`(`open-web-server-ledger::Ledger` が呼ぶ内部経路)を削除し、`open_runo.admin.v1.Admin/DbMutate`(Connect/gRPC、`x-api-key` またはセッション認証が必須)へ移した(`781c824`)。本体は `handlers_hyper::db_mutate_core`(冪等性は `__mutation_receipts`)。**`admin.proto` に `rpc DbMutate(google.protobuf.Struct) returns (google.protobuf.Struct)` が無いと「unknown method」(501)になる**。旧経路は認証なしだったので、`Ledger` を動かす側は環境変数 `OPEN_RUNO_API_KEY` が必須。テスト: router 181・`tests/ledger_fusion.rs`(本物の `Ledger` から新経路へ書き込み)・proto・gateway 成功。VPSで一時起動して、旧=404、新=鍵なし401・不正本文400・正常200を実測。残るRESTは SCIM(RFC 7644)・`/health`・ACME・`/mcp`・`/graphql`系・`/demo` の例外のみ。`openapi.rs` は `/mcp` が使うため削除不可で、旧REST記述の整理は未了。再開用メッセージは `runo/PORTING.md` 末尾「2026-11-17 別アカウントでの再開用メモ(open-cosmo・RPoem・aruaru-db …)」。

**English**: The old `POST /internal/db/mutate` (the internal route called by `open-web-server-ledger::Ledger`) was removed and replaced by `open_runo.admin.v1.Admin/DbMutate` (Connect/gRPC; `x-api-key` or session auth required) (`781c824`). The logic is `handlers_hyper::db_mutate_core` (idempotency via `__mutation_receipts`). **Without `rpc DbMutate(google.protobuf.Struct) returns (google.protobuf.Struct)` in `admin.proto` the call fails with "unknown method" (501)**. The old route needed no auth, so whatever runs a `Ledger` must set `OPEN_RUNO_API_KEY`. Tests: router 181, `tests/ledger_fusion.rs` (the real `Ledger` writing through the new route), proto and gateway pass. A temporary run on the VPS showed old = 404 and new = 401 without a key / 400 for a bad body / 200 for a valid one. The only REST left is the exceptions: SCIM (RFC 7644), `/health`, ACME, `/mcp`, the `/graphql` family and `/demo`. `openapi.rs` cannot be deleted (`/mcp` uses it); tidying its old REST descriptions is not done. Resume message: end of `runo/PORTING.md` ("2026-11-17 別アカウントでの再開用メモ(open-cosmo・RPoem・aruaru-db …)").

## 2026-11-18 OCRフォント配信のConnect化(別アカウントでの再開用)

**日本語**: `GET /v1/ocr/fonts/:name` を廃止し、`POST /aruaru_llm.v1.Llm/OcrFont`(JSON `{name}` → `{name, content_type, sha256, data_base64}`)へ移した。呼び出し側は RPoem の `open-runo-pdf-ocr`(`client.rs`の`font()`、sha256を照合)の1か所のみ。手元で新200・不明名404・旧404を実測、RPoem `open-runo-pdf-ocr` は22件+スモーク1件成功。
- ブランチ(push済み、**mainへ未マージ**): aruaru-llm `ocr-font-connect`(`0de391b`系、`94a9eab`)・RPoem `ocr-font-connect`(`345149a`)。
- **本番は未反映**。VPSの`/root/repository/aruaru-llm`は`c4c94d0`(`28007df`の翻訳ワーカー化より前)なので、`origin/main`をそのまま入れずに、`c4c94d0`へフォント修正だけを載せた`ocr-font-deploy`(`b42fc8c`、ローカルworktree `F:\_work\llm-deploy`、未push)をWSLでリリースビルド中だった(`F:\_workb4.sh`、出力`F:\_workb4.out`)。
- 未了: (1)ビルド成果物をVPSの`/root/aruaru-llm/target/release/aruaru-llm`へバックアップ付きで差し替え(旧は`*.before-*`で残す)、再起動後に`/healthz`・`OcrFont`200・旧404を確認、(2)aruaru-llmとRPoemの`main`へマージ、(3)`version-control`の`exceptions.toml`に入れた`/v1/ocr/fonts/:name`の例外(`879f619`)を戻す、(4)古い版のPDF変換クライアントはフォント取得が失敗する旨の告知。
- 注意: VPSではビルドしない(WSLでビルド→差し替え)。秘密は表示しない。REST新設は不可(Connectのみ)。

**English**: `GET /v1/ocr/fonts/:name` was removed and replaced by `POST /aruaru_llm.v1.Llm/OcrFont` (JSON `{name}` -> `{name, content_type, sha256, data_base64}`). The only caller is RPoem's `open-runo-pdf-ocr` (`font()` in `client.rs`, which verifies sha256). Verified locally: new 200, unknown name 404, old route 404; `open-runo-pdf-ocr` passes 22 tests + 1 smoke test.
- Branches (pushed, **not merged to main**): aruaru-llm `ocr-font-connect` (on `0de391b`, `94a9eab`), RPoem `ocr-font-connect` (`345149a`).
- **Production is not updated.** The VPS clone is at `c4c94d0` (before the `28007df` translate-worker change), so do not deploy `origin/main` as is; a branch `ocr-font-deploy` (`b42fc8c`, local worktree `F:\_work\llm-deploy`, unpushed) carries only the font fix on top of `c4c94d0` and was being release-built in WSL (`F:\_workb4.sh`, output `F:\_workb4.out`).
- Open: (1) swap the build into `/root/aruaru-llm/target/release/aruaru-llm` on the VPS with a backup (keep the old one as `*.before-*`), restart, check `/healthz`, `OcrFont` 200 and old route 404; (2) merge to `main` in aruaru-llm and RPoem; (3) revert the exception entry for `/v1/ocr/fonts/:name` added to `version-control`'s `exceptions.toml` (`879f619`); (4) tell users that old PDF-converter clients can no longer fetch fonts.
- Notes: never build on the VPS (WSL build, then swap); never print secrets; no new REST (Connect only).

**2026-11-18 追記(完了)**: aruaru-llm は `ocr-font-deploy`(`b42fc8c`)のビルドで本番へ差し替え済み(旧バイナリ `/root/aruaru-llm/target/release/aruaru-llm.before-font-20261118`)。本番で `OcrFont` 200・旧経路404・`Runtime` 200・3サービスactiveを実測。上の「本番は未反映」「mainへ未マージ」は解消済み(両リポジトリとも`main`へ取り込み、`version-control`の例外追記も戻した)。残りは古い版のPDF変換クライアントへの告知のみ。
**Update (done)**: aruaru-llm was swapped on the VPS with the `ocr-font-deploy` build (`b42fc8c`; old binary `aruaru-llm.before-font-20261118`). Verified in production: `OcrFont` 200, old route 404, `Runtime` 200, 3 services active. The 'production not updated' / 'not merged' notes above are resolved (both repos merged to `main`, the `version-control` exception entry reverted). Only the notice to old PDF-converter clients remains.

## 2026-11-19 追記: 4姉妹プロジェクト(`open-cosmo`・`RPoem`・`Reaxt`・`RPoRea`)を同時に開発 / The four sister projects are developed together

### 日本語
これまでの「3姉妹」(`Reaxt`・`RPoRea`・`open-cosmo`)に、土台である **`RPoem`** を加えた **4姉妹**を、常に同時に開発します(ユーザー指示)。役割は次のとおりです。

- **`RPoem`**: 土台(アプリケーションサーバー、Tomcat 相当。管理 API は Connect / gRPC)。
- **`Reaxt`**: Rust で書く画面とサーバーアクション。Cosmo 向けのサブグラフ入口 `POST /_reaxt/graphql` を持つ。
- **`RPoRea`**: `RPoem` と `Reaxt` を合わせた土台。Rust 以外からは C ABI(`rporea-ffi`)で使う。
- **`open-cosmo`**: 複数のサブグラフをまとめるゲートウェイ(WunderGraph Cosmo 相当)。

**決まり**: どれかの接点(Cosmo のサブグラフ形式・manifest・Connect・`x-reaxt`・管理 API)を変えたら、4つの README・CLAUDE・PORTING・POLICY を同じ日に、日本語とネイティブに近い英語で更新する。最新の到達点: ゲートウェイの複数サブグラフ分割、`@key` 結合(入れ子の段にも対応)、Reaxt の `input:` リテラル対応(いずれも push 済み)。未了: `@requires` / `@provides`、変更系の後の再取得、読み取りフィールドを持つ実際の Reaxt サンプルでの通し確認。

### English
The former "three sisters" (`Reaxt`, `RPoRea`, `open-cosmo`) now include their foundation, **`RPoem`**, making **four sister projects** that are always developed together (user instruction). Their roles:

- **`RPoem`**: the foundation (an application server, comparable to Tomcat; its admin API uses Connect / gRPC).
- **`Reaxt`**: UI and server actions written in Rust, with a Cosmo-compatible subgraph entry, `POST /_reaxt/graphql`.
- **`RPoRea`**: the base that joins `RPoem` and `Reaxt`; callers other than Rust use its C ABI (`rporea-ffi`).
- **`open-cosmo`**: the gateway that combines several subgraphs (comparable to WunderGraph Cosmo).

**Rule**: when an interface between them changes (the Cosmo subgraph format, the manifest, Connect, `x-reaxt`, the admin API), update the README, CLAUDE, PORTING and POLICY of all four on the same day, in Japanese and natural English. Latest state: the gateway splits queries across subgraphs and supports `@key` joins (including nested levels), and Reaxt accepts `input:` literals (all pushed). Open: `@requires` / `@provides`, re-fetching after a mutation, and a real run with a Reaxt example that has read fields.

## 2026-11-19 追記(続き3): 実サーバー2台での「読み取りの分割」を確認、Reaxt に読み取り用アクションを追加(4姉妹共通) / Read-query split verified on two real servers; read actions added to Reaxt (shared by the four sisters)

### 日本語
- **Reaxt の規約**: メソッド名が `Get` / `List` / `Read` で始まるアクション(例: `GetCount`、`GetVisits`)は、Cosmo 向けのサブグラフで GraphQL の **Query** として公開されます。それ以外は従来どおり **Mutation** です。変更用のアクションが1つも無いときは、SDL に `Mutation` 型を出しません。`reaxt_ready` は引き続き全サブグラフで共有されます。
- **実機での確認**: カウンター(`GetCount`)とグリーター(`GetVisits`)の2台を `open-cosmo` のゲートウェイ越しに動かし、`query { c: demo_v1_Counter_GetCount g: demo_v1_Greeter_GetVisits }` が、2台に分けて実行され、`{"data":{"c":2,"g":1}}` と1つの結果にまとまることを確認しました。2台にまたがる変更系は、従来どおり「読み取り専用の問い合わせだけが分割できる」というメッセージで拒否されます。`input:` のリテラルも実機で通りました。
- **試験**: Reaxt の `reaxt-actions` は11件に成功。
- **未了**: `@requires` / `@provides`、変更系の後の再取得。

### English
- **Reaxt convention**: actions whose method name starts with `Get`, `List` or `Read` (for example `GetCount`, `GetVisits`) are exposed by the Cosmo subgraph as GraphQL **Query** fields; all other actions remain **Mutation** fields. When there are no mutating actions, the SDL has no `Mutation` type. `reaxt_ready` is still shared by every subgraph.
- **Verified on a real machine**: with the counter (`GetCount`) and greeter (`GetVisits`) servers running behind the `open-cosmo` gateway, `query { c: demo_v1_Counter_GetCount g: demo_v1_Greeter_GetVisits }` was split across the two servers and merged into one answer, `{"data":{"c":2,"g":1}}`. A mutation spanning both is still rejected, now with the message that only read-only queries can be split. `input:` literals also worked on the real servers.
- **Tests**: `reaxt-actions` passes 11 tests.
- **Open**: `@requires` / `@provides`, and re-fetching after a mutation.

## 2026-11-19 追記(続き4): `@external`・`@provides`・`@requires` に対応(4姉妹共通) / `@external`, `@provides` and `@requires` supported (shared by the four sisters)

### 日本語
`open-cosmo` のゲートウェイの `@key` 結合が、Apollo Federation の残りの主なディレクティブにも対応しました。

- **`@external`**: サブグラフAが `@external` と宣言したフィールドは、Aでは解決できないものとして扱い、実際に持つサブグラフBへの結合(`_entities`)の対象にします。
- **`@provides`**: 親のフィールドが `@provides(fields: "...")` で供給すると宣言したフィールドだけは、`@external` でもAに任せ、結合しません(余計な往復を減らします)。
- **`@requires`**: サブグラフBのフィールドが `@requires(fields: "a b")` で他のフィールドを必要とするとき、(1) そのフィールドが選ばれたときだけ、`a` と `b` をサブグラフAの問い合わせに足して先に取得し、(2) エンティティ表現(representation)に鍵と一緒に入れて B の `_entities` へ送ります。クライアントが選んでいない `a` と `b` は、最後に結果から取り除きます。Aが必須フィールドを供給できないときは、結合せずに従来どおり転送します。
- **設計の根拠(調査)**: Apollo の [Subgraph 仕様](https://www.apollographql.com/docs/graphos/reference/federation/subgraph-spec/)(`_entities` は表現と同じ順で返す、`@requires` のフィールドは表現に含める)と、[apollographql/router](https://github.com/apollographql/router) の計画器(必須フィールドは、要求したフィールドが選ばれたときだけ取得する修正、`FetchNode` の `requires`)を参考にしました。コードは流用せず、Rust で一から実装しています。
- **試験**: ゲートウェイ47件・federation 15件に成功(擬似サブグラフ)。
- **未了**: 必須フィールドが「さらに別のサブグラフ」にある場合(3者以上の連鎖)、`@skip` / `@include` つきの条件、変更系の後の再取得。

### English
The `@key` join in the `open-cosmo` gateway now also handles the other main Apollo Federation directives.

- **`@external`**: a field that subgraph A marks `@external` is treated as not resolvable by A, and is joined (through `_entities`) from the subgraph B that really owns it.
- **`@provides`**: a field that the parent declares as supplied by `@provides(fields: "...")` stays with A even though it is `@external`, so no extra round trip is made.
- **`@requires`**: when a field of subgraph B uses `@requires(fields: "a b")`, the gateway (1) adds `a` and `b` to the query sent to subgraph A, only when that field is selected, and (2) sends them in the entity representation together with the key to B's `_entities`. Fields the client did not select are removed from the final result. If A cannot supply a required field, the query is forwarded as before without a join.
- **Design sources (research)**: the Apollo [Subgraph specification](https://www.apollographql.com/docs/graphos/reference/federation/subgraph-spec/) (`_entities` returns entities in the order of the representations; `@requires` fields belong in the representation) and the planner in [apollographql/router](https://github.com/apollographql/router) (required fields are fetched only when the requiring field is selected; `FetchNode` has `requires`). No code was copied; this is a fresh Rust implementation.
- **Tests**: gateway 47 and federation 15 pass (fake subgraphs).
- **Open**: required fields that live in yet another subgraph (chains of three or more), `@skip` / `@include` conditions, and re-fetching after a mutation.

## 2026-11-19 追記(続き5): 変更系(mutation)が返したエンティティの結合に対応(4姉妹共通) / Joins on entities returned by a mutation (shared by the four sisters)

### 日本語
`open-cosmo` のゲートウェイが、**変更系(mutation)が返したエンティティ**にも `@key` 結合を行うようになりました。たとえば `mutation { createUser { name reviews } }` で、`createUser` は users サブグラフが実行し、`reviews`(別のサブグラフにしかないフィールド)は、返ってきた `id` を使って reviews サブグラフの `_entities`(読み取り専用)から足します。

- **変更は1回だけ**: mutation は元のサブグラフで1回だけ実行し、結合のための追加の呼び出しは読み取り専用の `_entities` だけです(二重実行しません)。試験で、users 側が `mutation` を1回だけ、reviews 側が `_entities` だけを受けたことを確認しました。
- **従来どおり拒否**: 変更系が複数のサブグラフにまたがる問い合わせは、拒否します(原子的にできないため)。
- **試験**: ゲートウェイ48件に成功(擬似サブグラフ)。
- **未了**: 必須フィールドが別のサブグラフにある3者以上の連鎖、`@skip` / `@include` つきの条件。

### English
The `open-cosmo` gateway now also performs `@key` joins on **entities returned by a mutation**. For example, in `mutation { createUser { name reviews } }` the users subgraph runs `createUser`, and `reviews` (a field only another subgraph defines) is added from the reviews subgraph's read-only `_entities`, using the returned `id`.

- **The mutation runs once**: it is executed once at its own subgraph, and the only extra calls are read-only `_entities` calls (nothing runs twice). The test confirms that the users side saw exactly one `mutation` and the reviews side saw only `_entities`.
- **Still rejected**: mutations that span several subgraphs (they cannot be made atomic).
- **Tests**: the gateway passes 48 tests (fake subgraphs).
- **Open**: chains of three or more subgraphs where a required field lives in yet another subgraph, and `@skip` / `@include` conditions.

## 2026-11-19 追記(続き6): `@skip` / `@include` と、3者以上の連鎖に対応(4姉妹共通) / `@skip` / `@include` and chains of three or more subgraphs (shared by the four sisters)

### 日本語
`open-cosmo` のゲートウェイの結合が、次の2つに対応しました。

- **`@skip(if:)` / `@include(if:)`**: 条件は、リテラルか、問い合わせの時点で分かっている変数の値で、ゲートウェイが**計画時に**判定します。除外されたフィールドは問い合わせから外し、そのフィールドのためだけに必要だった別のサブグラフへの呼び出しも行いません。これ以外のディレクティブや、値の分からない変数は、従来どおり結合せずに転送します。
- **3者以上の連鎖**: `@requires` の必須フィールドが、最初のサブグラフでも結合先でもない**さらに別のサブグラフ**にあるとき、その別のサブグラフへの結合を先に行い、得た値を結合先の表現(representation)に入れます(例: catalog → pricing で `price` を取得 → shipping の `shippingEstimate` に渡す)。結合には順位を付けて、必要な順に実行します(最大4段)。クライアントが選んでいない必須フィールドは、最後に結果から取り除きます。どのサブグラフも供給できないときは、結合せずに転送します。
- **試験**: ゲートウェイ51件に成功(擬似サブグラフ)。
- **未了**: 実際のサーバー(Reaxt 以外のサブグラフ)での `@key` / `@requires` の通し確認、`@skip` / `@include` をフラグメント内で使う場合(フラグメントは分割・結合の対象外のまま)。

### English
The join in the `open-cosmo` gateway now handles two more cases.

- **`@skip(if:)` / `@include(if:)`**: the condition is a literal or a variable whose value is already known, and the gateway decides it **at planning time**. An excluded field is dropped from the query, and a call to another subgraph that was needed only for that field is not made. Other directives, and variables with no known value, are forwarded as before, without a join.
- **Chains of three or more subgraphs**: when a `@requires` field lives in **yet another subgraph** (neither the first one nor the one being joined), the gateway first joins that third subgraph and puts the value into the representation sent onward (for example: catalog, then pricing supplies `price`, then shipping computes `shippingEstimate`). Joins get a rank and run in dependency order (up to four levels). Required fields the client did not select are removed from the final result. If no subgraph can supply a field, the query is forwarded without a join.
- **Tests**: the gateway passes 51 tests (fake subgraphs).
- **Open**: a run of `@key` / `@requires` against real servers other than Reaxt, and `@skip` / `@include` inside fragments (fragments are still outside the split and join planner).

## 2026-11-19 追記(続き7): 本物のフェデレーション実装2台で `@key` 結合を確認し、ルート型の名前の不具合を修正(4姉妹共通) / `@key` join verified against two real federation servers; root-type-name bug fixed (shared by the four sisters)

### 日本語
擬似サブグラフだけでなく、**独立した本物の GraphQL サーバー**でも確認しました。`async-graphql` のフェデレーション機能(`_service { sdl }` と `_entities` を自前で実装)で users と reviews の2台を立て、`open-cosmo` のゲートウェイ越しに `{ me { name reviews } }` を送ると、`{"name":"user-7","reviews":["review-of-7"]}` と正しく結合されました(試験はゲートウェイ52件に成功)。

- **見つかった不具合と修正**: 実際のサーバーは、ルート型の名前が `Query` とは限りません(今回は `QueryA`)。ゲートウェイは `Query` / `Mutation` 固定で探していたため、結合が働かず、元の問い合わせがそのまま転送されて失敗しました。内省(introspection)でルート型の名前を取得し、内部では `Query` / `Mutation` に正規化するようにして直しました。擬似サブグラフだけの試験では見つからなかった不具合です。
- **未了**: `@requires` と `@provides` を本物のサーバーで確認する試験(今回は `@key` のみ)、Reaxt 自身のエンティティ(`@key`)対応、フラグメント内のディレクティブ。

### English
We verified the join not only with fake subgraphs but also with **independent, real GraphQL servers**. Two servers (users and reviews) were built with `async-graphql`'s own federation support (its own `_service { sdl }` and `_entities`); sending `{ me { name reviews } }` through the `open-cosmo` gateway returned the correctly joined `{"name":"user-7","reviews":["review-of-7"]}` (the gateway passes 52 tests).

- **Bug found and fixed**: a real server's root type is not always called `Query` (here it was `QueryA`). The gateway looked for `Query` / `Mutation` by name, so no join happened and the original query was forwarded unchanged, which failed. The gateway now reads the root type names through introspection and normalizes them to `Query` / `Mutation` internally. The fake-subgraph tests alone could not have found this.
- **Open**: tests of `@requires` and `@provides` against real servers (this run covered `@key` only), entity (`@key`) support in Reaxt itself, and directives inside fragments.

## 2026-11-19 追記(続き8): 本物の SDL での `@external` / `@requires` / `@provides` の読み取りを確認、Reaxt のエンティティ対応は見送り(4姉妹共通) / Directives read from real SDL; Reaxt entities deliberately not added (shared by the four sisters)

### 日本語
- **確認**: `async-graphql` が実際に生成する SDL(`#[graphql(external)]`・`#[graphql(requires = "price")]`・`#[graphql(provides = "price")]` つき)を、ゲートウェイのモデルが正しく読むことを試験で確認しました(`@key`・`@external`・`@requires`・`@provides` の4種。ゲートウェイ53件に成功)。手書きの SDL だけでなく、実際の実装が出力する書式でも読めます。ただし、`@requires` と `@provides` を使った**結合の実行**を本物のサーバー2台で通すところまでは、まだ行っていません(実行の確認は擬似サブグラフでの試験のみ)。
- **判断(見送り)**: Reaxt 自身を `@key` のエンティティにすることは**しません**。Reaxt のアクションは `JSON` スカラーを返し、GraphQL の型(フィールドの構造)を持たないためです。型を持たせるには Reaxt のアクション定義に型情報を足す大きな設計変更が必要で、現状の用途(アクションの呼び出しと読み取り)には不要です。必要になったら、`Wire` 型から GraphQL の型を生成する形を検討します。
- **未了**: `@requires` / `@provides` を使った結合を、本物のサーバー2台で実行する試験、フラグメント内のディレクティブ。

### English
- **Verified**: the gateway model correctly reads the SDL that `async-graphql` really generates (with `#[graphql(external)]`, `#[graphql(requires = "price")]` and `#[graphql(provides = "price")]`): `@key`, `@external`, `@requires` and `@provides` (the gateway passes 53 tests). It reads the format produced by a real implementation, not only hand-written SDL. However, running a **join that uses `@requires` and `@provides`** against two real servers has not been done yet (that execution is covered only by fake-subgraph tests).
- **Decision (not added)**: Reaxt itself will **not** become a `@key` entity. Reaxt actions return the `JSON` scalar and have no GraphQL type (no field structure). Giving them one would need a large design change in the action definitions, and the current use (calling actions and reading values) does not need it. If it is ever needed, we will consider generating GraphQL types from the `Wire` types.
- **Open**: running joins that use `@requires` / `@provides` against two real servers, and directives inside fragments.


## 2026-11-19 いまの状況と、これから作る機能(4姉妹: open-cosmo・RPoem・Reaxt・RPoRea)

### 日本語(やさしい説明)
**4つは、いつも一緒に作ります。** `RPoem`は土台、`open-cosmo`は入口(玄関)、`Reaxt`は画面とサーバーの仕事を書く道具、`RPoRea`はRPoemとReaxtをくっつけたものです。1つを変えたら、ほかの3つへの影響も確かめ、4つの README・CLAUDE・PORTING・POLICY を同じ日に直します。

**もうできていること**
- `Reaxt`のサーバーが、Cosmo(複数のサーバーをまとめる仕組み)に参加できる(`POST /_reaxt/graphql`)。
- 入口(ゲートウェイ)が、1つの質問を複数のサーバーに分けて聞き、答えをまとめる。
- `@key`で、別々のサーバーが持つ同じ人・物の情報をつなぐ(入れ子や、4段までの連鎖もOK)。
- `@requires`と`@provides`(必要な項目を先に取る・持っている項目をそのまま使う)。
- `@skip`と`@include`(項目を出す・出さないの切り替え)。型条件のないフラグメントの中でも動く。
- `Reaxt`のアクションが型(項目の形)を持てる。`@key`のエンティティとして公開できる。
- 本物のサーバー2つを入口につないで、1つの質問で結合できることを確認した。

**これから作る機能(完成予定)**
1. 結合の計画(`@key`)でも、フラグメントを使えるようにする。
2. `... on 型`(型条件つきのフラグメント)に、安全な範囲で対応する。
3. 変更(書き込み)のあとに、最新の値を取り直す。
4. `Reaxt`の`Option`(あるかもしれない値)や、もっと複雑な型への対応。
5. `open-cosmo`のルーターへ、Reaxtのサーバーをもっと広く実際につなぐ試験。
6. `RPoRea`をRust以外(Pythonなど)から使う入口(C ABI)の、Mojoでの確認、Mac/Android/iPhoneへの広がり。

**進め方のきまり**: 終わっていない仕事は、**確認を取らず、自動で次々に進めます**。作ったら試験をして、本物のサーバーで確かめてから「できた」と書きます。秘密(パスワードなど)は書きません。新しいRESTは作りません。

### English (simple words, easy to read)
**We always build these four together.** `RPoem` is the base. `open-cosmo` is the front door. `Reaxt` is the tool for writing screens and server jobs. `RPoRea` joins RPoem and Reaxt. If we change one, we check the other three too, and we update the README, CLAUDE, PORTING and POLICY of all four on the same day.

**What already works**
- A `Reaxt` server can join Cosmo (a way to combine many servers) at `POST /_reaxt/graphql`.
- The front door (gateway) splits one question across many servers and puts the answers together.
- `@key` joins the same person or thing that lives in different servers (nested joins and chains up to four steps work).
- `@requires` and `@provides` (fetch needed fields first; reuse fields a server already has).
- `@skip` and `@include` (show or hide a field). They also work inside fragments that have no type condition.
- `Reaxt` actions can have types (the shape of the fields) and can be shown as `@key` entities.
- We ran two real servers behind the gateway and joined them with one question.

**What we plan to build (the finish line)**
1. Let the join planner (`@key`) use fragments too.
2. Support `... on Type` (fragments with a type condition) in a safe way.
3. Re-read the newest values after a write.
4. Support `Option` (a value that may be missing) and richer types in `Reaxt`.
5. Wider real tests that connect Reaxt servers to the `open-cosmo` router.
6. Use `RPoRea` from other languages (the C ABI): check it with Mojo, then Mac, Android and iPhone.

**How we work**: we keep going on unfinished jobs **without asking for confirmation**. We test, check on real servers, and only then say "done". We never write secrets (such as passwords). We do not add new REST APIs.


## 2026-11-19 多言語版 / Translations (8 languages; 日本語・English are above / AI翻訳、ネイティブ確認前)

### 简体中文
**这四个项目始终一起开发。** `RPoem`是地基,`open-cosmo`是大门,`Reaxt`是编写界面和服务器任务的工具,`RPoRea`把RPoem和Reaxt合在一起。改动其中一个,就要检查另外三个,并在同一天更新四个项目的README、CLAUDE、PORTING、POLICY。
- **已经完成**:Reaxt服务器可加入Cosmo(`POST /_reaxt/graphql`);网关把一个问题分给多个服务器再合并答案;`@key`连接不同服务器里的同一个人或物(可嵌套、最多4步);`@requires`、`@provides`、`@skip`、`@include`(也可在无类型条件的片段里使用);Reaxt动作可以有类型并作为`@key`实体公开;用两个真实服务器验证了一次提问合并结果。
- **计划完成**:① 连接计划支持片段(已完成);② 安全支持`... on 类型`;③ 写入后重新读取最新值;④ Reaxt支持`Option`和更复杂的类型;⑤ 更广泛的真实连接测试;⑥ 从其他语言(C ABI)使用RPoRea,先用Mojo验证,再到Mac、Android、iPhone。
- **做事规则**:未完成的工作**无需确认,自动继续**;先测试、在真实服务器上确认后才说"完成";不写入密码等机密;不新增REST。

### 繁體中文
**這四個專案永遠一起開發。** `RPoem`是地基,`open-cosmo`是大門,`Reaxt`是撰寫畫面與伺服器工作的工具,`RPoRea`把RPoem與Reaxt結合在一起。改動其中一個,就要檢查另外三個,並在同一天更新四個專案的README、CLAUDE、PORTING、POLICY。
- **已經完成**:Reaxt伺服器可加入Cosmo(`POST /_reaxt/graphql`);閘道把一個問題分給多個伺服器再合併答案;`@key`連接不同伺服器裡的同一個人或物(可巢狀、最多4步);`@requires`、`@provides`、`@skip`、`@include`(也可用於沒有型別條件的片段);Reaxt動作可以有型別並作為`@key`實體公開;以兩個真實伺服器驗證了一次提問合併結果。
- **預計完成**:① 連接計畫支援片段(已完成);② 安全支援`... on 型別`;③ 寫入後重新讀取最新值;④ Reaxt支援`Option`與更複雜的型別;⑤ 更廣泛的真實連線測試;⑥ 從其他語言(C ABI)使用RPoRea,先以Mojo驗證,再到Mac、Android、iPhone。
- **做事規則**:未完成的工作**不需確認,自動繼續**;先測試、在真實伺服器上確認後才說「完成」;不寫入密碼等機密;不新增REST。

### 한국어
**이 네 프로젝트는 항상 함께 만듭니다.** `RPoem`은 바탕, `open-cosmo`는 현관문, `Reaxt`는 화면과 서버 작업을 쓰는 도구, `RPoRea`는 RPoem과 Reaxt를 합친 것입니다. 하나를 바꾸면 나머지 셋도 확인하고, 같은 날 네 프로젝트의 README, CLAUDE, PORTING, POLICY를 고칩니다.
- **이미 된 것**: Reaxt 서버가 Cosmo에 참여(`POST /_reaxt/graphql`), 게이트웨이가 질문 하나를 여러 서버에 나눠 묻고 답을 합침, `@key`로 서로 다른 서버의 같은 사람·물건을 연결(중첩, 최대 4단계), `@requires`·`@provides`·`@skip`·`@include`(타입 조건 없는 프래그먼트 안에서도 동작), Reaxt 액션에 타입을 주고 `@key` 엔티티로 공개, 실제 서버 2대로 한 번의 질문 결합 확인.
- **만들 예정**: ① 결합 계획에서 프래그먼트 지원(완료), ② `... on 타입`의 안전한 지원, ③ 쓰기 뒤 최신 값 다시 읽기, ④ Reaxt의 `Option`과 더 복잡한 타입, ⑤ 더 넓은 실제 연결 시험, ⑥ 다른 언어(C ABI)에서 RPoRea 사용(Mojo 확인 후 Mac·Android·iPhone).
- **작업 규칙**: 끝나지 않은 일은 **확인 없이 자동으로 계속**합니다. 시험하고 실제 서버에서 확인한 뒤에만 "완료"라고 씁니다. 비밀번호 같은 비밀은 쓰지 않고, 새 REST는 만들지 않습니다.

### Deutsch
**Diese vier Projekte entwickeln wir immer zusammen.** `RPoem` ist das Fundament, `open-cosmo` die Eingangstür, `Reaxt` das Werkzeug für Oberflächen und Server-Aufgaben, `RPoRea` verbindet RPoem und Reaxt. Ändern wir eines, prüfen wir die anderen drei und aktualisieren README, CLAUDE, PORTING und POLICY aller vier am selben Tag.
- **Schon fertig**: Reaxt-Server können Cosmo beitreten (`POST /_reaxt/graphql`); das Gateway verteilt eine Frage auf mehrere Server und fügt die Antworten zusammen; `@key` verbindet dieselbe Person oder Sache aus verschiedenen Servern (verschachtelt, bis zu 4 Schritte); `@requires`, `@provides`, `@skip`, `@include` (auch in Fragmenten ohne Typbedingung); Reaxt-Aktionen haben Typen und können als `@key`-Entitäten angeboten werden; mit zwei echten Servern wurde eine Frage erfolgreich zusammengeführt.
- **Geplant**: ① Fragmente im Verbindungsplan (erledigt); ② `... on Typ` sicher unterstützen; ③ nach dem Schreiben neue Werte lesen; ④ `Option` und komplexere Typen in Reaxt; ⑤ breitere Tests mit echten Verbindungen; ⑥ RPoRea aus anderen Sprachen (C ABI) nutzen, erst Mojo, dann Mac, Android, iPhone.
- **Arbeitsregel**: Unfertiges machen wir **ohne Rückfrage automatisch weiter**. Wir testen, prüfen auf echten Servern und schreiben erst dann „fertig“. Keine Geheimnisse (z. B. Passwörter) in Dateien, keine neuen REST-APIs.

### Français
**Nous développons toujours ces quatre projets ensemble.** `RPoem` est la base, `open-cosmo` la porte d'entrée, `Reaxt` l'outil pour écrire écrans et tâches serveur, `RPoRea` réunit RPoem et Reaxt. Si l'un change, nous vérifions les trois autres et mettons à jour README, CLAUDE, PORTING et POLICY des quatre le même jour.
- **Déjà fait** : un serveur Reaxt peut rejoindre Cosmo (`POST /_reaxt/graphql`) ; la passerelle répartit une question sur plusieurs serveurs et assemble les réponses ; `@key` relie la même personne ou chose située dans des serveurs différents (imbriqué, jusqu'à 4 étapes) ; `@requires`, `@provides`, `@skip`, `@include` (aussi dans les fragments sans condition de type) ; les actions Reaxt ont des types et deviennent des entités `@key` ; une question a été assemblée avec deux vrais serveurs.
- **Prévu** : ① fragments dans le plan de jointure (fait) ; ② `... on Type` de façon sûre ; ③ relire les valeurs après une écriture ; ④ `Option` et types plus riches dans Reaxt ; ⑤ tests de connexion réelle plus larges ; ⑥ utiliser RPoRea depuis d'autres langages (C ABI) : Mojo d'abord, puis Mac, Android, iPhone.
- **Règle de travail** : ce qui n'est pas fini continue **automatiquement, sans demander**. Nous testons, vérifions sur de vrais serveurs, puis seulement écrivons « terminé ». Pas de secrets (mots de passe…) dans les fichiers, pas de nouvelle API REST.

### Español
**Siempre desarrollamos estos cuatro proyectos juntos.** `RPoem` es la base, `open-cosmo` la puerta de entrada, `Reaxt` la herramienta para escribir pantallas y tareas del servidor, y `RPoRea` une RPoem y Reaxt. Si cambiamos uno, revisamos los otros tres y actualizamos README, CLAUDE, PORTING y POLICY de los cuatro el mismo día.
- **Ya funciona**: un servidor Reaxt puede unirse a Cosmo (`POST /_reaxt/graphql`); la pasarela reparte una pregunta entre varios servidores y junta las respuestas; `@key` une la misma persona o cosa que está en servidores distintos (anidado, hasta 4 pasos); `@requires`, `@provides`, `@skip`, `@include` (también en fragmentos sin condición de tipo); las acciones de Reaxt tienen tipos y se publican como entidades `@key`; se juntó una pregunta con dos servidores reales.
- **Planeado**: ① fragmentos en el plan de unión (hecho); ② `... on Tipo` de forma segura; ③ volver a leer los valores tras escribir; ④ `Option` y tipos más ricos en Reaxt; ⑤ pruebas de conexión real más amplias; ⑥ usar RPoRea desde otros lenguajes (C ABI): primero Mojo, luego Mac, Android e iPhone.
- **Regla de trabajo**: lo que falta se sigue **automáticamente, sin pedir confirmación**. Probamos, verificamos en servidores reales y solo entonces escribimos «terminado». Sin secretos (contraseñas) en los archivos y sin nuevas APIs REST.

### Português
**Sempre desenvolvemos estes quatro projetos juntos.** `RPoem` é a base, `open-cosmo` é a porta de entrada, `Reaxt` é a ferramenta para escrever telas e tarefas do servidor, e `RPoRea` une RPoem e Reaxt. Se mudamos um, verificamos os outros três e atualizamos README, CLAUDE, PORTING e POLICY dos quatro no mesmo dia.
- **Já funciona**: um servidor Reaxt pode entrar no Cosmo (`POST /_reaxt/graphql`); o gateway divide uma pergunta entre vários servidores e junta as respostas; `@key` liga a mesma pessoa ou coisa que está em servidores diferentes (aninhado, até 4 passos); `@requires`, `@provides`, `@skip`, `@include` (também em fragmentos sem condição de tipo); as ações do Reaxt têm tipos e viram entidades `@key`; uma pergunta foi juntada com dois servidores reais.
- **Planejado**: ① fragmentos no plano de junção (feito); ② `... on Tipo` de forma segura; ③ reler os valores após escrever; ④ `Option` e tipos mais ricos no Reaxt; ⑤ testes de conexão real mais amplos; ⑥ usar o RPoRea de outras linguagens (C ABI): primeiro Mojo, depois Mac, Android e iPhone.
- **Regra de trabalho**: o que falta continua **automaticamente, sem pedir confirmação**. Testamos, verificamos em servidores reais e só então escrevemos «concluído». Sem segredos (senhas) nos arquivos e sem novas APIs REST.

### Русский
**Эти четыре проекта мы всегда делаем вместе.** `RPoem` — основа, `open-cosmo` — входная дверь, `Reaxt` — инструмент для экранов и серверных задач, `RPoRea` соединяет RPoem и Reaxt. Если меняем один, проверяем остальные три и в тот же день обновляем README, CLAUDE, PORTING и POLICY всех четырёх.
- **Уже работает**: сервер Reaxt может присоединиться к Cosmo (`POST /_reaxt/graphql`); шлюз делит один вопрос между серверами и собирает ответы; `@key` связывает одного и того же человека или предмет из разных серверов (вложенно, до 4 шагов); `@requires`, `@provides`, `@skip`, `@include` (в том числе во фрагментах без условия типа); действия Reaxt имеют типы и открываются как сущности `@key`; на двух настоящих серверах один вопрос успешно объединён.
- **Планируется**: ① фрагменты в плане соединения (готово); ② безопасная поддержка `... on Тип`; ③ перечитывание значений после записи; ④ `Option` и более сложные типы в Reaxt; ⑤ более широкие проверки с реальными соединениями; ⑥ использование RPoRea из других языков (C ABI): сначала Mojo, затем Mac, Android и iPhone.
- **Правило работы**: незавершённое мы продолжаем **автоматически, не спрашивая подтверждения**. Сначала проверяем на настоящих серверах и только потом пишем «готово». Секреты (пароли) в файлы не пишем, новых REST-API не создаём.


## 2026-11-19 リミット前のチェックポイント(4姉妹・日本語、別アカウントでの再開用)
**到達点(すべてpush済み、試験成功)**
- ゲートウェイ(open-cosmo `remote_subgraphs.rs`): 複数サブグラフへの分割、`@key`結合(入れ子・4段連鎖)、`@requires`/`@provides`、`@skip`/`@include`、型条件なしのインラインフラグメントの展開、`... on 型`(親と同じ型のときだけ展開)。ゲートウェイ試験57件成功。
- Reaxt(`reaxt-actions`): アクションが型を持つ(`Wire::gql_type`・`wire_struct!`)、`@key`エンティティ(`ModuleActions::entity`)、`_entities`、選択の射影(別名・`__typename`・`... on`)、`Option<T>`(null可)、`f64`(Float)、JSONに無いフィールドは`Option`なら`None`。試験15件成功。
- 実機: `serve`と`serve_greeter`(どちらも型付き`Visitor`)をゲートウェイ(`OPEN_RUNO_SUBGRAPHS="名前=URL,..."`)の後ろに置き、1つの質問で`visits`と`count`が`@key`で結合されることを確認。2サーバーにまたがる変更は仕様どおり拒否。
- 文書: 4リポジトリとrunoのREADME/CLAUDE/PORTING/POLICYに、現状・完成予定・きまりを日英と8言語(AI翻訳、ネイティブ確認前)で記載済み。

**未了(この順で、確認不要で自動で進める)**
1. 変更(書き込み)の後に最新の値を取り直す機能。
2. 名前付きフラグメント(`...F`)と、型が違う`... on 型`(インターフェース/ユニオン)への安全な対応。
3. Reaxtの複雑な型(`Option`の入れ子、`HashMap`など)、`Wire`の対応型の拡張。
4. open-cosmoのルーターへReaxtをさらに広く実際につなぐ試験。
5. RPoRea(C ABI)をMojoで確認、Mac・Android・iPhoneへ。
6. クラウドでの軽い仕事の定期実行(2時間ごと): **GitHubをClaudeアカウントに接続**(https://claude.ai/connect-github)してから作る。

**注意**: Reaxtの型の登録(レジストリ)はプロセス内で共有。並列試験では`JSON`か型付きかの表記が揺れるので、SDL試験は`(input: JSON):`までで比べる。ゲートウェイの`OPEN_RUNO_SUBGRAPHS`は`名前=URL`形式。新しいRESTは作らない。秘密は書かない。

**再開用メッセージ**: 前の続きです。`F:\runo\PORTING.md`末尾「リミット前のチェックポイント(4姉妹)」を読んでから、open-cosmo・RPoem・Reaxt・RPoRea の未了を、確認なしで自動で進めてください。区切りごとにREADME/CLAUDE/PORTING/POLICYを10か国語で更新し、commitしてpushします。返答は日本語のみ。


## 2026-11-19 まとめと再開のしかた / Summary and how to resume (日本語・English)

### 日本語(やさしい説明)
**今日できたこと**
- Reaxtのサーバーを、入口(open-cosmo)の後ろに並べて使えるようになりました。入口は、1つの質問を複数のサーバーに分けて聞き、答えを1つにまとめます。
- 別々のサーバーにある「同じ人・同じ物」の情報を、`@key`でつなげます。`@requires`・`@provides`・`@skip`・`@include`・フラグメントにも対応しました。
- Reaxtの仕事(アクション)に「型」(中身の形)を持たせられるようになりました。値が無いかもしれない型(`Option`)や小数(`f64`)も使えます。
- 本物のサーバー2つで、1回の質問に両方の答えが入ることを確かめました。試験はゲートウェイ57件・Reaxt15件が成功しています。

**まだ終わっていないこと(この順に進めます)**
1. 書き込みのあと、最新の値を取り直す。
2. 名前付きフラグメントや、型が違う`... on 型`への安全な対応。
3. Reaxtでもっと複雑な型を使えるようにする。
4. open-cosmoのルーターで、もっと広く実際につなぐ試験。
5. RPoReaをMojo・Mac・Android・iPhoneから使えるか確かめる。
6. 軽い仕事は、クラウドで2時間ごとに自動実行する(先にGitHubをClaudeにつなぐ: https://claude.ai/connect-github)。

**再開のしかた**: 新しい会話で「`F:\runo\PORTING.md`の末尾を読んで、4姉妹(open-cosmo・RPoem・Reaxt・RPoRea)の未了を確認なしで自動で進めて」と伝えてください。**進め方のきまり**: 確認は取らずに進める/試験して本物で確かめてから「できた」と書く/秘密は書かない/新しいRESTは作らない。

### English (natural, plain wording)
**What we got done today**
- Reaxt servers can now sit behind the front door (open-cosmo). The gateway splits one question across several servers and merges the answers into one.
- `@key` links the same person or item that lives on different servers. `@requires`, `@provides`, `@skip`, `@include` and fragments work too.
- Reaxt actions can now carry types (the shape of their data), including optional values (`Option`) and decimals (`f64`).
- With two real servers we confirmed that a single question returns data from both. All tests pass: 57 in the gateway and 15 in Reaxt.

**What is still open (we will go in this order)**
1. Re-read the latest values after a write.
2. Safe support for named fragments and for `... on Type` with a different type.
3. Richer types in Reaxt.
4. Wider real-world tests that connect Reaxt to the open-cosmo router.
5. Check that RPoRea works from Mojo, Mac, Android and iPhone.
6. Run the light jobs in the cloud every two hours (first connect GitHub to Claude: https://claude.ai/connect-github).

**How to resume**: in a new conversation, say "Read the end of `F:\runo\PORTING.md`, then keep working through the open items of the four sister projects (open-cosmo, RPoem, Reaxt, RPoRea) without asking for confirmation." **Working rules**: keep going without asking; test and verify on real systems before saying "done"; never write secrets; do not add new REST APIs.


## 2026-11-19 最終まとめ(リミット前) / Final summary before the limit (日本語・English)

### 日本語(やさしい説明)
**さっき終わったこと**
- **名前付きフラグメント**(`...F`)に対応しました。ゲートウェイは、問い合わせの中の`...F`を、`... on 型 { }`の形に置きかえてから計画します。型が親と同じなら展開し、`@skip`/`@include`も効きます。ぐるぐる回る参照(循環)は、無限ループにならず、そのまま転送します。
- **Reaxtのサーバー**も、名前付きフラグメント・型なしの`... { }`・`@skip`/`@include`(変数もOK)を読めるようになりました。これで、全部の項目が除外されて「分割せずそのまま転送」になった場合も、ちゃんと動きます。
- 本物のサーバー2つをゲートウェイの後ろに置いて、`...F @include(if: $x)`で、`true`なら`visits`と`count`が結合されて返り、`false`なら`id`だけが返ることを確かめました。
- 試験: ゲートウェイ58件、Reaxt16件が成功。

**まだ終わっていないこと**
1. 書き込みのあと、最新の値を取り直す。
2. 型が違う`... on 型`(インターフェース/ユニオン)への安全な対応。
3. Reaxtでもっと複雑な型(`Option`の入れ子、`HashMap`など)を使えるようにする。
4. open-cosmoのルーターで、もっと広く実際につなぐ試験。
5. RPoReaをMojo・Mac・Android・iPhoneから使えるか確かめる。
6. 軽い仕事のクラウド定期実行(先にGitHubをClaudeにつなぐ)。

**再開のしかた**: 新しい会話で「`F:\runo\PORTING.md`の末尾を読んで、4姉妹の未了を確認なしで自動で進めて」と伝えてください。

### English (natural, plain wording)
**What we just finished**
- **Named fragments** (`...F`) are now supported. The gateway rewrites each `...F` into `... on Type { }` before planning. When the type matches the parent it is flattened, and `@skip`/`@include` work. A circular reference does not loop forever; the query is simply forwarded as it is.
- **Reaxt servers** can now read named fragments, untyped `... { }` and `@skip`/`@include` (variables too). So even when every field is excluded and the query is forwarded unsplit, it still works.
- With two real servers behind the gateway, `...F @include(if: $x)` returned `visits` and `count` joined together for `true`, and only `id` for `false`.
- Tests: 58 in the gateway and 16 in Reaxt all pass.

**What is still open**
1. Re-read the latest values after a write.
2. Safe support for `... on Type` with a different type (interfaces and unions).
3. Richer Reaxt types (nested `Option`, `HashMap`, and so on).
4. Wider real-world tests that connect Reaxt to the open-cosmo router.
5. Check that RPoRea works from Mojo, Mac, Android and iPhone.
6. Run the light jobs in the cloud on a schedule (first connect GitHub to Claude).

**How to resume**: in a new conversation, say "Read the end of `F:\runo\PORTING.md`, then keep working through the open items of the four sister projects without asking for confirmation."


## 2026-11-19 追記(リミット前の最後) / Last addition before the limit (日本語・English)

### 日本語(やさしい説明)
- Reaxtのアクションで使える整数の種類を増やしました(`u64`・`i8`・`i16`・`isize`を追加)。範囲の外の数(たとえば`i8`に`300`)や、負の数を`u64`に入れようとすると、エラーになります。試験は17件成功です。
- ゲートウェイの試験は58件成功のままです。名前付きフラグメント(`...F`)と`@skip`/`@include`は、本物のサーバー2つで確かめ済みです。
- **残りの仕事**: ①書き込みのあとに最新の値を取り直す ②型が違う`... on 型`(インターフェース/ユニオン) ③Reaxtのもっと複雑な型 ④open-cosmoのルーターでの広い実機試験 ⑤RPoReaをMojo・Mac・Android・iPhoneから使う ⑥軽い仕事のクラウド定期実行(先にGitHubをClaudeにつなぐ)。
- 再開のしかた: 新しい会話で「`F:\runo\PORTING.md`の末尾を読んで、4姉妹の未了を確認なしで自動で進めて」と伝えてください。

### English (natural, plain wording)
- Reaxt actions can now use more integer types (`u64`, `i8`, `i16` and `isize` were added). Out-of-range numbers (for example `300` as an `i8`) and negative numbers for a `u64` are rejected with an error. 17 tests pass.
- The gateway still passes its 58 tests. Named fragments (`...F`) and `@skip`/`@include` were checked with two real servers.
- **Still open**: (1) re-read the latest values after a write; (2) `... on Type` with a different type (interfaces and unions); (3) richer Reaxt types; (4) wider real-world tests with the open-cosmo router; (5) use RPoRea from Mojo, Mac, Android and iPhone; (6) run the light jobs in the cloud on a schedule (first connect GitHub to Claude).
- How to resume: in a new conversation, say "Read the end of `F:\runo\PORTING.md`, then keep working through the open items of the four sister projects without asking for confirmation."


## 2026-11-19 追記: 書き込みのあとの取り直し(実機確認) / Re-reading after a write (verified)

### 日本語(やさしい説明)
- 書き込み(変更)が型付きの結果を返すとき、**同じ1回の質問で、別のサーバーの最新の値も受け取れる**ことを、本物のサーバー2つで確かめました。例: `mutation { Touch(input:{id:"a1"}) { id visits count } }` は、書き込み(`visits`)を**1回だけ**実行し、そのあと`count`を別のサーバーから読みます(2回続けて呼ぶと`visits`は1、2と増えます)。
- 仕組みは、これまでの「変更は1回だけ実行して、読み取りだけをほかのサーバーへつなぐ」計画です。新しい仕組みを足さなくても、取り直しの目的を満たしています。`serve_greeter`に`Touch`の例を足しました。
- この項目(書き込み後の取り直し)は完了にします。残りは、型が違う`... on 型`、Reaxtのもっと複雑な型、広い実機試験、Mojo/Mac/Android/iPhone、クラウドの定期実行(先にGitHubをClaudeにつなぐ)です。

### English (natural, plain wording)
- When a write returns a typed result, **one single request can also bring back the latest values from another server**. We checked this with two real servers. Example: `mutation { Touch(input:{id:"a1"}) { id visits count } }` runs the write (`visits`) **exactly once**, then reads `count` from the other server (calling it twice makes `visits` go 1, then 2).
- It works through the existing plan: run the mutation once and join only read-only fields from other servers. No new mechanism was needed to meet the goal. We added a `Touch` example to `serve_greeter`.
- We mark this item (re-reading after a write) as done. Still open: `... on Type` with a different type, richer Reaxt types, wider real-world tests, Mojo/Mac/Android/iPhone, and the scheduled cloud runs (first connect GitHub to Claude).


## 2026-11-19 リミット前の最終まとめ(2回目) / Final summary before the limit, part 2 (日本語・English)

### 日本語(やさしい説明)
**終わったこと**
- **`... on 型`**: スキーマに「インターフェース」や「ユニオン」が1つも無いときは、型が違う`... on 別の型`は絶対に当てはまらないので、安全に取り除けるようにしました(`SubgraphModel.has_abstract`で判定)。インターフェース/ユニオンがあるときは、これまでどおり通常の転送にします。ゲートウェイ試験59件・federation試験16件が成功。
- **Androidスマホでの実機試験**: `librporea_ffi.so`(aarch64)をスマホ(A401OP)に入れ、Cのテストプログラム(`RPoRea/bindings/c/android_test.c`)で、サーバー起動・Connectの呼び出し(200)・ファイル配信・拒否(403)・GETは405(RESTなし)を確認し、すべて成功しました。
- **書き込み後の取り直し**: 実機で確認済み(前の節)。

**まだ終わっていないこと**
1. **Mojo**: WSLにMojoを入れて、`RPoRea`のC ABIを呼ぶ確認と`bindings/mojo`の作成(導入中。入ったら、動かして書く)。
2. Reaxtのもっと複雑な型(`Option`の入れ子、`HashMap`など)。
3. open-cosmoのルーターでの、もっと広い実機試験。
4. Mac・iPhoneでの確認。
5. 軽い仕事のクラウド定期実行(先にGitHubをClaudeにつなぐ)。

**再開のしかた**: 新しい会話で「`F:\runo\PORTING.md`の末尾を読んで、4姉妹の未了を確認なしで自動で進めて」と伝えてください。

### English (natural, plain wording)
**What is done**
- **`... on Type`**: when a schema has no interface or union at all, a fragment on a different type can never match, so it is now safely dropped (decided by `SubgraphModel.has_abstract`). When interfaces or unions exist, the query is forwarded unsplit as before. 59 gateway tests and 16 federation tests pass.
- **Real Android phone test**: we put `librporea_ffi.so` (aarch64) on the phone (A401OP) and ran a C test program (`RPoRea/bindings/c/android_test.c`). Starting the server, a Connect call (200), static assets, a denial (403) and GET returning 405 (no REST) all passed.
- **Re-reading after a write**: verified on real servers (previous section).

**What is still open**
1. **Mojo**: install Mojo in WSL, check that it can call RPoRea's C ABI, and write `bindings/mojo` (the install is running; once it finishes we run and write it).
2. Richer Reaxt types (nested `Option`, `HashMap`, and so on).
3. Wider real-world tests with the open-cosmo router.
4. Checks on Mac and iPhone.
5. Scheduled cloud runs for light jobs (first connect GitHub to Claude).

**How to resume**: in a new conversation, say "Read the end of `F:\runo\PORTING.md`, then keep working through the open items of the four sister projects without asking for confirmation."


## 2026-11-20 Mojoでの確認(WSL) / Mojo check (WSL) (日本語・English)

### 日本語(やさしい説明)
- **Mojo 1.2.0をWSL(Ubuntu)に入れました**(`pip install mojo`、仮想環境`~/mojo-venv`)。MojoはWindowsでは直接動かないので、WSLで使います。
- **MojoからRPoReaのC ABIを実際に呼ぶ試験**(`RPoRea/bindings/mojo/test_rporea.mojo`)が成功しました: ライブラリ読み込み → バージョン取得 → ビルダー作成 → ファイル登録 → サーバー起動 → HTTPで`hello from mojo`を取得(200) → 未登録のメソッドは404。実行のしかた: `RPOREA_LIB=/root/rporea-target/release/librporea_ffi.so mojo test_rporea.mojo`(WSL)。
- **Mojo 1.2での書き方のコツ**: ポインタは`Int`で渡す(`Int(s.unsafe_ptr())`)。`len(String)`は使えないので`byte_length()`。Pythonの値は`Int(py=...)`・`String(py=...)`で変換する。
- **まだ**: Mojoの関数をC側のコールバックとして渡す部分(`rporea_method`)は未了。Mac・iPhoneの確認も未了。
- Androidスマホ(A401OP)での実機試験は、前の節のとおり成功済みです。

### English (natural, plain wording)
- **Mojo 1.2.0 is installed in WSL (Ubuntu)** (`pip install mojo`, virtual environment `~/mojo-venv`). Mojo does not run natively on Windows, so we use it through WSL.
- **A test that calls RPoRea's C ABI from Mojo** (`RPoRea/bindings/mojo/test_rporea.mojo`) passes: load the library, read the version, create a builder, register a file, start the server, fetch `hello from mojo` over HTTP (200), and get 404 for an unregistered method. Run it in WSL with `RPOREA_LIB=/root/rporea-target/release/librporea_ffi.so mojo test_rporea.mojo`.
- **Tips for Mojo 1.2**: pass pointers as `Int` (`Int(s.unsafe_ptr())`); `len(String)` is not allowed, so use `byte_length()`; convert Python values with `Int(py=...)` and `String(py=...)`.
- **Still open**: passing a Mojo function as a C callback (`rporea_method`), and the Mac and iPhone checks.
- The real-phone test on the Android device (A401OP) passed earlier (see the previous section).
