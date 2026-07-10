# 技術スタック・開発ルール(poem-cosmo-tauri)

**このリポジトリが正本(一本化先)です。** `open-runo`は2026-07-10付けで
このリポジトリに統合され、今後更新しません(詳細は下記「方針転換」参照)。

ユーザー指示により、**Tauri・Poem・WunderGraph Cosmo(有料版含む)を
ライブラリ/パッケージとして直接依存させることはしない**方針に転換した。
ただし各ツールが提供する**機能・API形状・コンセプトには互換性を保ちつつ
引き続き活用**する(例: TauriのデスクトップUI体験・`invoke()`的な
コマンド呼び出し、Poemの薄いHTTPルーティング設計、Cosmoの GraphQL
Federation/VersionlessAPIという考え方)。それらを Rust 標準ライブラリ
+ tokio/hyper で自前実装し、外部パッケージへの直接依存を持たない形に
置き換える。

このリポジトリ、および関連プロジェクト(`open-runo`/`open-web-server`/
`aruaru-db`/`aruaru-web`/`open-raid-z`)で開発・保守を行う際は、以下を基本方針とする。
作業ドライブは `F:\open-runo`(E:ドライブは2026-07-10に消失、以後Fが実体)。
この節は [`open-raid-z`](https://github.com/aon-co-jp/open-raid-z) の
`CLAUDE.md` を正本とし、各プロジェクトへコピーして同期する。

**poem-cosmo-tauri とは**: `open-runo`(Rust + Poem 製 GraphQL Federation
プラットフォーム)を正本として分岐した `poem-runo` を、2026-07-10 に
`F:\open-runo\poem-runo` → `F:\open-runo\poem-cosmo-tauri` へリネームし、
GitHub リポジトリも `https://github.com/aon-co-jp/poem-cosmo-tauri` に
移行した最新の後継リポジトリ。REST API の乱立と WunderGraph Cosmo 有料版
への依存を断つという open-runo の目的を、Poem(バックエンド)+ Cosmo
(着想元・非依存)+ Tauri(フロントエンド)の統合をリポジトリ名として明示
する形で引き継ぐ。**WunderGraph Cosmo は今後もあくまで参考・着想元のみで
あり、実装に依存として組み込むことはしない**(2026-07-10 ユーザー確認済み)。
履歴は open-runo / poem-runo のものをそのまま保持しているため、コミット
ログは 2026-07-10 の分岐点まで共通。今後の開発は poem-cosmo-tauri 側を
主軸に進める。

## フロントエンド

- Tauriパッケージには直接依存しない。ただしTauriのデスクトップUI体験・
  `invoke()`的なコマンド呼び出しインターフェースとは互換性を保った形で
  HTML5/CSS3 + 必要最低限のTypeScriptで自前実装する(ロジックはRust側、
  TypeScript側は薄い配線のみという方針は維持)。
- **Bootstrap**

## バックエンド・コア

- **Rust**(メイン言語、標準ライブラリ中心): https://www.rust-lang.org/ja/ | https://github.com/rust-lang/rust
- **tokio** + **hyper**(Webフレームワークなしで直接HTTPサーバを自前実装):
  https://tokio.rs/ | https://docs.rs/hyper/latest/hyper/
- Poemパッケージには直接依存しないが、Poemのルーティング/ハンドラAPI形状
  とは互換性のあるインターフェースを維持しながら移行する(既存ハンドラの
  シグネチャ・レスポンス形式は極力変えない)。

## API設計思想(参考・概念のみ)

- **VersionLess API**という考え方を参考にする(WunderGraphのブログ/podcast参照)。
- **WunderGraph Cosmo**: **有料版を含めパッケージとしては直接依存させない**。
  GraphQL Federation / VersionlessAPI というAPI形状・コンセプトのみ参考に
  し、Rust標準+tokio/hyperで互換性を保ちつつ自前実装する。
  https://github.com/wundergraph/cosmo

## 関連プロジェクト

- **poem-cosmo-tauri**(このリポジトリ。正本・一本化先。open-runo/poem-runo
  の後継。Poem/Tauri/Cosmoの機能を自前実装で統合したGraphQL Federation /
  API Gateway / AI-native routing platform): https://github.com/aon-co-jp/poem-cosmo-tauri
- **open-runo**(分岐元。2026-07-10付けでこのリポジトリに統合され、今後は
  更新しない): https://github.com/aon-co-jp/open-runo
- **open-web-server**: https://github.com/aon-co-jp/open-web-server
- **aruaru-db**: https://github.com/aon-co-jp/aruaru-db
- **aruaru-web**: https://github.com/aon-co-jp/aruaru-web
- **open-raid-z**(開発ルールの正本): https://github.com/aon-co-jp/open-raid-z
- **rs-to-readme**: https://github.com/aon-co-jp/rs-to-readme

## 運用ルール

- **開発中はこの`CLAUDE.md`を、コード変更のコミット/pushと必ず一緒に
  push する**(内容を更新した場合はもちろん、変更が無い場合も他の変更と
  一緒にコミット対象へ含めておくこと)。
- 実装で迷った場合や、API仕様の詳細確認が必要な場合は、学習データからの
  推測より公式ドキュメント(上記URL)を優先して参照する。
- 作業ドライブが変わった場合は、この節を更新し、関連プロジェクトの
  引き継ぎ資料にも変更の経緯を記録すること。
- **無人自動開発(確認不要・自動デバッグ)のタイミングでは、20〜30分おきの
  スケジュール実行待ちにせず、1パス内でできる限り連続して作業を進める**
  こと(ユーザー指示、2026-07-10)。小さく検証可能な単位(1ハンドラ/1関数
  ごとに `cargo test` → commit → push)を保ちながらも、次の増分に進む前に
  バックグラウンド待機で止まらない。スケジュールされたウェイクアップは
  「セッションが終わっても翌朝まで継続する」ためのフォールバックであり、
  同一パス内で作業を続けられる間は使わない。

## 現状(このリポジトリ固有)

- `cargo check --workspace` / `cargo test --workspace --no-run` は成功する
  (15クレート構成、テストコンパイル済み)。todo!()/unimplemented!()マーカーなし。
- README多言語版は `README-<言語>.md` 形式で日本語・英語・中国語簡体字・
  韓国語・スペイン語・フランス語・ドイツ語・イタリア語・ロシア語・
  アラビア語の10言語が揃っている。

## HANDOFF(直近の自動実行パス)

- **2026-07-10 open-runo-router poem→tokio/hyper 移行: X-Api-Key認証を移植・
  2ハンドラに適用**: 新規 `crates/open-runo-router/src/auth_hyper.rs` に
  `check_api_key(headers, guardian) -> Result<(), StatusCode>` を追加 —
  `auth.rs`の`ApiKeyAuthEndpoint`(poem Middleware)からX-Api-Keyチェック
  部分のみを抜き出した素の関数(KeyGuardian.verify()呼び出し、
  RegistryEmpty/Ok→通過、Rejected/Suspended→401)。**JWT/OIDC/SCIM固定
  トークン/RBACは意図的に未移植**(必要になった時点で追加、それまでは
  該当ルートはpoem側に残す)。`handlers_hyper.rs`の
  `federation_status_handler`/`db_status_handler`をこの関数で保護するよう
  更新(シグネチャに`Arc<KeyGuardian>`追加)、「キー無し→401」テストを
  2本追加。`cargo test -p open-runo-router` で12テスト全green(前回の
  5+2に加えauth_hyper 3件・401テスト2件)。`cargo check --workspace` /
  `cargo test --workspace --no-run` もgreen。poemのbuild_appは引き続き
  唯一の実バイナリ経路(未切替)。
  次回パスがすべきこと: (1) 他の認証必須GET(db_routing, get_schema,
  get_schema_history, get_persisted_query)をfederation_status/db_statusと
  同じパターン(check_api_key呼び出し)で移植、(2) POST/PUT/DELETE系
  (register_schema, compose_schemas, db_put/db_delete等)は
  `read_json_body`ヘルパーを使ってリクエストボディも扱う形で移植、
  (3) 1つずつ増やすたびに`cargo test -p open-runo-router` →
  `cargo test --workspace --no-run`→commit→push、(4) 全ハンドラ移行後に
  lib.rsのbuild_appをhyper_compat版Routerに切替・main.rsを
  `hyper_compat::serve`起動に変更、(5) その時点でJWT/OIDC/RBAC/SCIM
  トークンが必要なルートが残っていればそれらも移植するか、必要性を
  再評価、(6) 最後にCargo.tomlからpoem削除・open-runo-gateway側の
  `Route::nest`合成コードを追従。

- **2026-07-10 open-runo-router poem→tokio/hyper 移行: 認証不要GET2本を移植**:
  新規 `crates/open-runo-router/src/handlers_hyper.rs` に
  `federation_status_handler`(GET /api/federation/status、poem版と同一
  JSON形状)と `db_status_handler`(GET /api/db/status、test-modeの
  in-memoryバックエンド応答を再現)を追加。各々 `hyper_compat::serve` で
  実ポートにbindしreqwestで叩く統合テストを追加、2件ともgreen。
  `cargo test -p open-runo-router` 全体・`cargo test --workspace --no-run`
  ともgreen。**まだ認証(X-Api-Key)を通していない**点に注意 — 本番の
  `/api/federation/status`・`/api/db/status`はauth必須だが、
  hyper_compat版はまだ認証層がないため未認証状態のロジックのみを
  移植している(認証はauth.rs抽出後に追加予定)。poemのbuild_appは
  相変わらず唯一の実バイナリ経路。
  次回パスがすべきこと: (1) `auth.rs`から「X-Api-Keyヘッダを見てOK/NGを
  判定する」部分だけを`poem::Middleware`実装から独立した素の関数
  (例: `fn check_api_key(headers: &HeaderMap, guardian: &KeyGuardian) ->
  Result<Claims, StatusCode>`)として`handlers_hyper.rs`か新規
  `auth_hyper.rs`に切り出す、(2) それを使ってfederation_status/db_statusの
  hyper_compat版に認証チェックを追加し、「キー無し→401」テストを足す、
  (3) 次に他のGET系ハンドラ(db_routing, get_schema等)を同様に移植、
  (4) 引き続き1つ増やすごとにcargo test → commit → push、(5) 全て揃ったら
  lib.rsのbuild_appをhyper_compat版に切替・main.rsをserve()起動に変更・
  Cargo.tomlからpoem削除・open-runo-gateway側の統合更新。

- **2026-07-10 open-runo-router poem→tokio/hyper 移行: health エンドポイント
  実サーバー動作確認まで完了**: `hyper_compat.rs` に `health_handler()`
  (poem版 `health` と同一JSON形状: status/service/version)と `serve()`
  (実TCPリスナー上でhyperのhttp1コネクションを捌く汎用サーバー起動関数)を
  追加。新規テスト `health_endpoint_serves_over_real_http` は
  `serve()`で実ポートにbindし、`reqwest`で `/health`・`/healthz`・
  存在しないパス(404確認)を実際にHTTP経由で叩いて検証 — 型チェックだけ
  でなく実通信で動作確認済み。`cargo test -p open-runo-router hyper_compat`
  で5テスト全green、`cargo check --workspace` / `cargo test --workspace
  --no-run` も green。**poem は引き続き削除しておらず既存の poem
  ベース `build_app` と並存**(まだどこからも `hyper_compat::serve` は
  呼ばれておらず、実際のバイナリは相変わらず poem 版で起動する)。
  次回パスがすべきこと: (1) 次に単純なハンドラ1〜2個
  (例: `handlers/federation.rs` の `federation_status`、または
  `handlers/db.rs` の `db_status` — 認証なしで動くGETから)を
  `hyper_compat::Handler` へ移植、(2) 認証(`auth.rs`のAPIキー検証ロジック
  本体、poemの`Middleware`実装部分は避けて「ヘッダを見て許可/拒否を返す
  関数」だけを先に関数として切り出す)を hyper_compat 用に用意し、
  保護されたハンドラのテストも書く、(3) 1つずつ増やすたびに
  `cargo test -p open-runo-router` → `cargo test --workspace --no-run`
  で確認してcommit+push、(4) 全ハンドラ移行後にlib.rsのbuild_app全体を
  新ルータに切り替え、main.rsを`hyper_compat::serve`起動に変更、
  (5) 最後にCargo.tomlからpoemを削除、open-runo-gateway側の統合を追従。

- **2026-07-10 open-runo-router poem→tokio/hyper 移行: 着手(基盤モジュール
  作成)**: 前回パスが残した計画に従い、`crates/open-runo-router/src/
  hyper_compat.rs` を新規作成。内容: `Router`(method+path→handlerの手書き
  ディスパッチャ、`:param`動的セグメント対応)、`Params`、`json_response`/
  `empty_status`/`read_json_body` レスポンスヘルパー、`Handler` 型
  (`Arc<dyn Fn(Request, Params) -> BoxFuture<Response>>`)。4つのユニット
  テスト全て green。`Cargo.toml` に `hyper`(1.10, full)・`hyper-util`・
  `http-body-util`・`bytes` を追加(dev-dependenciesに`reqwest`も追加、
  次回のhyperベーステストハーネスで使用予定)。**poem はまだ削除していない
  **— 既存の poem ベース `build_app`/ハンドラ群はそのまま並存させており、
  `lib.rs` に `pub mod hyper_compat;` を追加しただけ。`cargo check
  --workspace` / `cargo test --workspace --no-run` とも green を確認済み。
  次回パスがすべきこと: (1) `handlers/schemas.rs` を手本に、1ハンドラ
  (例: `health`)を `hyper_compat::Handler` 形式で書き直し、hyperベースの
  テストハーネス(`tokio::net::TcpListener` + `hyper::server::conn::http1`
  + `reqwest`)を1本書いて動作確認、(2) 確認できたら残りのハンドラを
  auth.rs → 各handlers/*.rs → middleware群 → lib.rsのbuild_app本体の順で
  段階的に置き換え、(3) 置き換えが終わった範囲から `poem::test::TestClient`
  ベースの旧テストを新ハーネスに移行、(4) 全ハンドラ移行後に
  `Cargo.toml` から `poem` を削除、(5) `open-runo-gateway` 側の
  `Route::nest` 合成コードも追従、(6) 各段階で `cargo test -p
  open-runo-router` → `cargo test --workspace --no-run` の順に確認して
  commit+push。旧パスが残した詳細設計(関数コンビネータ方式のミドルウェア
  等)は下記の前エントリを参照。

- **2026-07-10 open-runo-router poem→tokio/hyper 移行: 調査完了・未着手
  (安全のため着手を見送り、旧エントリ)**: `crates/open-runo-router` を poem 依存ゼロで
  tokio+hyper 直書きへ移行するタスクを受けたが、調査の結果 poem への依存が
  非常に深いことが判明したため、ワークスペースを red にするリスクを避け、
  **コード変更は一切行わず**現状の green な状態を維持したまま計画のみを
  ここに残す。次回、十分な作業時間がある session で以下の手順を実行する
  こと。

  **依存の実態(調査結果)**:
  - `src/lib.rs`(759行): `Route`/`get`/`post`/`Endpoint`/`EndpointExt`/
    `#[handler]`/`Json` で全ルート定義。テストは `poem::test::TestClient`
    を30個弱のテストで多用(`assert_status_is_ok`, `assert_json`,
    `assert_header`, `.body_json()` 等)。
  - `src/auth.rs`(545行): `ApiKeyAuth` が `poem::Middleware<E>` +
    `Endpoint<Output=E::Output>` を自前実装(RBAC/OIDC/SCIM token/
    KeyGuardian 統合)。`req.extensions_mut().insert::<Claims>()` で
    後続ハンドラに認証情報を渡す。ここが一番複雑。
  - `src/rate_limit.rs`: 同様に `Middleware`/`Endpoint` 自前実装(単純、
    移行は比較的容易)。
  - `src/middleware/cors.rs`: `poem::middleware::Cors` をラップしているだけ
    → 自前 CORS ヘッダ付与ロジックに置き換えが必要。
  - `src/middleware/html_cache.rs`(747行、最複雑): singleflight ロック・
    stale-while-revalidate バックグラウンド再レンダリング・
    `CachePredictor` AI 予測を `Endpoint`/`Middleware` trait 上に実装。
    `Response::builder()`, `resp.into_body().into_string()`,
    `poem::http::Uri` 等を多用。
  - `src/handlers/*.rs`(9ファイル: ai_routing, cache, db, events,
    federation, maintenance, persisted_queries, scim, schemas):
    すべて `#[handler]` マクロ + `Data<&Arc<AppState>>` / `Path` / `Query` /
    `Json` エクストラクタ。`events.rs` は `poem::web::sse::{Event, SSE}` で
    SSE 実装。
  - `crates/open-runo-gateway/src/main.rs` と
    `crates/open-runo-router/src/main.rs`: `Route::new().nest("/", build_app(...))`
    で this crate の `build_app()` の戻り値(`impl Endpoint`)を
    gateway 側の GraphQL ルートと合成している。gateway 側も
    `async-graphql-poem`, `poem::web::Data`, `IntoResponse` 等に依存。

  **推奨移行方針(次回セッション向け設計)**:
  1. `build_app()` の戻り値型を `impl Endpoint`(poem)から、
     `tower::Service<hyper::Request<Incoming>, Response=hyper::Response<...>>`
     相当の自前トレイト、または単純に
     `Arc<dyn Fn(Request<Incoming>) -> BoxFuture<Response<Full<Bytes>>>>`
     的な単一関数ディスパッチャに置き換える。ミドルウェア(auth/cors/
     rate_limit/html_cache/tracing)は「関数を受け取り関数を返す」
     コンビネータとして再実装すれば trait 地獄を避けられる
     (poem の `Middleware<E>` パターンを模倣する必要はない)。
  2. 自前の軽量ルータ(path + method → handler fn の `HashMap` か
     `matchit`/手書き match)を用意し、`:param` 動的セグメントを
     手動パースする(`matchit` crate 追加が最も安全。workspace 未使用
     なので追加要検討 — ただし「フレームワーク直接依存禁止」は
     Tauri/Poem/Cosmo に限定される方針なので matchit 等の薄いルータ
     crate は許容範囲と解釈できる。迷う場合は手書き match でも可)。
  3. `#[handler]` マクロ相当は不要 — 各ハンドラを
     `async fn(Arc<AppState>, hyper::Request<Incoming>) -> Result<Response<...>, ...>`
     形式の素の async fn に書き換える。`Data<&Arc<AppState>>` は
     クロージャで `Arc<AppState>` を capture するだけで代替可能。
  4. テストは `poem::test::TestClient` の代わりに、hyper 1.x の
     `hyper::server::conn::http1` + `tokio::net::TcpListener` で
     実際に127.0.0.1:0にbindしてreqwestかhyper Clientで叩く
     小さなテストハーネス(`fn spawn_test_server(app) -> (addr, JoinHandle)`)
     を書くのが一番安全(assert_status_is_ok 等のアサーションヘルパーは
     手動で書き直す必要あり、30テスト全部の書き直しが必要)。
  5. gateway 側(`open-runo-gateway`)は async-graphql-poem に依存したままで
     良い(このタスクのスコープ外)。router 側が生の
     `hyper::service::Service` を返すようになったら、gateway の
     `main.rs`/`lib.rs` 側で「path prefix で振り分ける」小さな
     アダプタ関数を書いて両方を束ねる(poem 経由の合成 `.nest()` は
     使えなくなるので、生 hyper で最上位ディスパッチを書く必要がある)。
  6. 作業順序の推奨: (a) `handlers/schemas.rs` のような依存の少ない
     ハンドラ群から先に素の hyper 関数へ書き換えてコンパイルを保つ
     (poem は残したまま、新旧ハンドラを共存させる形でインクリメンタルに
     進める)、(b) auth.rs の `ApiKeyAuth` を関数コンビネータに書き換える、
     (c) middleware 群(cors/rate_limit/html_cache)を順に置き換える、
     (d) lib.rs の `build_app` を新ルータに切り替える、(e) 全テストを
     新ハーネスに移行、(f) Cargo.toml から poem を削除、(g) gateway 側を
     追従、(h) `cargo check --workspace` / `cargo test --workspace --no-run`
     で確認。

  **今回変更したファイル**: なし(調査のみ)。ワークスペースは調査前と
  同じ green 状態。次回セッションはこの計画に従い、上記手順(a)から着手
  すること。1ハンドラ群ごとに `cargo test -p open-runo-router` を回して
  グリーンを確認しながら進める運用ルールは既存の WORKFLOW 指示通り継続。

- **2026-07-10 方針転換・正本確定**: ユーザーから複数回の確認を経て最終確定:
  (1) Tauri・Poem・WunderGraph Cosmo(有料版含む)はパッケージとして直接
  依存させない、(2) ただし各機能・API形状には互換性を保ちつつRust標準+
  tokio/hyperで自前実装して使う、(3) 正本(一本化先)リポジトリは
  **poem-cosmo-tauri**(open-runoではない)、open-runoはこちらに統合され
  今後更新しない。README作成・push後、2026-07-11 12:00まで確認不要で
  無人自動開発/デバッグを継続する指示。次回パスがすべきこと:
  (1) README.md / README-Japan.md / README-English.md をpoem-cosmo-tauri
  正本・新方針(no Tauri/Poem/Cosmo依存、tokio/hyper自前実装)に合わせて
  更新、(2) 開発ルールをCLAUDE.mdとして保存(このファイル自体が該当、
  内容の同期を確認)、(3) 他プロジェクトへ移植可能なporting用ファイル
  (PORTING.md相当)を作成、(4) commit & push、(5) open-runo-router を
  Poemからtokio/hyperへAPI互換を保ちつつ移行開始、(6) cargo check/testで
  健全性確認、(7) 12:00まで各パスでHANDOFFを上書きしループ継続。

- **2026-07-10 poem-cosmo-tauri へリネーム**: ユーザーから
  poem-cosmo-tauri (https://github.com/aon-co-jp/poem-cosmo-tauri) への
  統合を指示された。同名の空リポジトリが既に存在したため `gh repo rename`
  は使わず、`poem-runo` ディレクトリを `poem-cosmo-tauri` にローカル
  リネームし、git remote を張り替えて `git push -u origin main` で移行。
  ユーザーは「Cosmo は参考のみ・Pure Rust 再実装」の従来方針を明示的に
  再確認(有料版を依存として組み込む案は却下)。2026-07-11 12:00 まで
  確認不要で無人自動開発/自動デバッグを継続する指示を受けた。次回パスが
  すべきこと: (1) CLAUDE.md の内容が正しく反映されているか確認、
  (2) README.md 冒頭を poem-cosmo-tauri 名義に更新、(3) 全 README-*.md の
  タイトル/バッジURLの poem-runo 表記を確認し必要に応じて更新、
  (4) `cargo check --workspace` / `cargo test --workspace --no-run` で
  ビルド健全性を確認、(5) `docs/HANDOFF.md` の次点候補から実装を1つ進める、
  (6) 作業ごとに commit して `git push origin main`、(7) 12:00 まで
  この HANDOFF を毎回上書きしてループを継続。

- **2026-07-10 20:30 poem-runo 発足**: `open-runo` を正本として
  `F:\open-runo\open-runo` から `git clone` し、リモートを
  `https://github.com/aon-co-jp/poem-runo.git` に付け替えて push、
  `poem-runo` リポジトリを発足させた(履歴は open-runo と共通)。
  無人自動開発を 2026-07-11 12:00 まで継続する運用中。21:30 に安全のため
  push、21:40 前後で利用制限のリセットが想定されるが、回復後は
  scheduled task 経由でこの CLAUDE.md の HANDOFF を読み、続きから開発を
  再開すること。次回パスがまず行うべきこと: (1) このリポジトリ固有の
  README.md 冒頭を poem-runo 向けに書き換え(open-runo からの fork/継承
  である旨を明記)、(2) `cargo check --workspace` で現状ビルドが壊れて
  いないことを確認、(3) `docs/HANDOFF.md` の次点候補から実装を1つ進める、
  (4) 作業ごとに commit して `git push origin main` する。
  次回パス実行時は毎回この項目を上書き更新し、進捗と次にやることを
  明記すること(セッションを跨いで記憶が引き継がれないため)。

- **2026-07-10 (open-runo 側)**: 定時の自律メンテナンスパス。`cargo check --workspace` /
  `cargo test --workspace --no-run` は変更前から成功済みを確認(ビルド破損なし)。
  `todo!()`/`unimplemented!()`/フェイクデータを返すスタブ関数は見つからず
  (実装は本当に完了している)。README-Japan.md と README-English.md が
  Phase A 以前の古い「ビジョン文書」のまま放置されており、実際の実装
  (15クレート・176テスト・自己学習AI・KeyGuardian・DUAL DATABASE・
  VersionlessAPI 等)と矛盾していた(例: 英語版は「設計・開発初期段階」
  「License TBD」「外部LLMプロバイダへのルーティング」と記載)ため、
  ルートの `README.md`(正しい最新情報)を基準に両ファイルを修正した:
  README-Japan.md はルート README.md の内容をそのまま反映、
  README-English.md は他8言語版と同じ構成(機能比較表・open-runo限定機能・
  クイックスタート・15クレート構成)の正確な英語版に書き換えた。
  他8言語版(中/韓/西/仏/独/伊/露/アラビア語)は内容確認済みで正確、変更不要。
  次回パスへの引き継ぎ: 特に緊急の課題は残っていない。次点候補は
  `docs/HANDOFF.md` の「次セッション候補」(Google Drive API 直接統合、
  FederatedBackend の TOML 設定化など)。
