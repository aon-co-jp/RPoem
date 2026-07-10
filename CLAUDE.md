# 技術スタック・開発ルール(poem-cosmo-tauri)

**このリポジトリは `open-runo` と同時並行で開発します**(2026-07-10、
再確定。一本化・統合ではありません)。実装(例: Poem→tokio/hyper移行)は
このリポジトリで先行させ、動作確認できたファイルを open-runo へミラー
する運用とする。両リポジトリともTauri/Poemを直接依存に含めない
(詳細は下記「方針転換」参照)。

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
移行したリポジトリ。REST API の乱立と WunderGraph Cosmo 有料版への依存を
断つという open-runo の目的を、Poem(バックエンド)+ Cosmo(着想元・
非依存)+ Tauri(フロントエンド)の統合をリポジトリ名として明示する形で
引き継ぐ(名称は歴史的経緯によるもので、実体はTauri/Poem非依存)。
**WunderGraph Cosmo は今後もあくまで参考・着想元のみであり、実装に依存
として組み込むことはしない**(2026-07-10 ユーザー確認済み)。履歴は
open-runo / poem-runo のものをそのまま保持しているため、コミットログは
2026-07-10 の分岐点まで共通。**2026-07-10、ユーザー指示により「統合」
方針を撤回し、open-runoと本リポジトリを同時並行で開発する方針に
再確定**。実装はこちらで先行させ、動作確認済みのファイルをopen-runoへ
ミラーする。

## フロントエンド(2026-07-10、方針更新)

- Tauriパッケージには直接依存しない。ただしTauriのデスクトップUI体験・
  `invoke()`的なコマンド呼び出しインターフェースとは互換性を保つ。
- **HTML5/CSS3・TypeScript・Bootstrap・Node.jsのスタックは廃止**。
  Rustをメイン言語としてフロントエンドとバックエンドを統合し、
  **WebAssembly (WASM)** に置き換える(コンパイル対象はRust →
  `wasm32-unknown-unknown`)。DOM操作・`invoke()`相当の呼び出しは
  Rust製WASMモジュール側で行い、TypeScript/Node.jsのビルドチェーンには
  依存しない。https://webassembly.org/ | https://rustwasm.github.io/

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

- **poem-cosmo-tauri**(このリポジトリ。open-runoと同時並行開発。実装の
  先行地点。Poem/Tauri/Cosmoの機能を自前実装で統合したGraphQL Federation /
  API Gateway / AI-native routing platform): https://github.com/aon-co-jp/poem-cosmo-tauri
- **open-runo**(分岐元。poem-cosmo-tauriと同時並行開発。2026-07-10付けで
  開発再開): https://github.com/aon-co-jp/open-runo
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

- **2026-07-11 WASMフロントエンド着手 — 実ブラウザで動作確認完了**:
  gateway移行の判断待ちの間、方針転換で決まった新フロントエンド
  (HTML5/CSS3/TypeScript/Bootstrap/Node.js廃止、Rust→WASM統合)に着手。
  `rustup target add wasm32-unknown-unknown`でターゲット追加、
  `cargo install wasm-bindgen-cli --version 0.2.126`でCLI導入(Node.js
  不使用、Rustツールチェーンのみ)。新規`apps/desktop-wasm/`crate
  (独立`[workspace]`、メインのサーバー側workspaceには非加入):
  `src/lib.rs`(`#[wasm_bindgen(start)]`エントリポイント、DOM操作は
  `web_sys`)、`src/api.rs`(`invoke()`相当の代替として`fetch()`ベースの
  素のasync関数、`/health`エンドポイントをJSONデコードして呼ぶ
  `health_check()`)。`www/index.html`(Bootstrap CDN無し、素のCSS、
  wasm-bindgen生成JSグルーはロード用の薄いスクリプトのみでビルド
  チェーンではない)。
  **実際にブラウザでの動作を確認済み**: `cargo build --target
  wasm32-unknown-unknown`→`wasm-bindgen --target web`でJSグルー生成→
  preview_startでPythonの`http.server`によりホスティング→
  preview_console_logs/preview_screenshot/preview_evalで確認:
  WASMモジュールが実際にロード・起動("open-runo-desktop-wasm
  starting"ログ)、DOM操作(`#content`要素への書き込み)、`fetch('/health')`
  呼び出しとエラーハンドリング(静的サーバーなので404、
  "health check failed: HTTP 404"と正しく表示)がすべて実ブラウザで
  機能することを確認。**バグ発見・修正**: 最初mount_shellが独自に
  `#content` divを作成していたが、www/index.htmlの既存`#content`と
  重複していた("Loading…"が2回表示される不具合) → 既存要素を再利用する
  形に修正、再ビルド・再検証してgreen。
  open-runo側へのミラー・push完了。
  **サーバー側(open-runo-router)のPoem→hyper移行は前回パスの状態
  (main.rsはhyper化済み、gatewayはpoem依存のまま)から変更なし。**
  次回パスがすべきこと: (1) WASMフロントエンドに残りのページ
  (schemas/federation/ai-routing、旧`apps/desktop/src/pages/*.ts`相当)を
  Rustで実装、(2) サイドバーのページ切り替えナビゲーションをRust側で
  実装(現状は静的HTMLのみ)、(3) open-runo-router自体を静的ファイル
  サーバーとしても使えるようにする(WASMバンドルを配信するための
  `GET /`, `GET /pkg/*`ルートをhyper_compat側に追加)か、別途配信手段を
  決める、(4) 旧`apps/desktop`(Tauri/TypeScript版)をいつ削除するか
  判断(WASM版が機能的に追いついてから)、(5) gateway移行の判断
  (前々回パスのHANDOFF参照)が引き続き保留、(6) 全体
  `cargo check --workspace` / `cargo test --workspace --no-run`を
  定期的に確認しつつ、両リポジトリへのミラーとpushを継続。

- **2026-07-11 訂正: build_app削除の前にgateway移行が必要と判明**:
  前回パスのHANDOFFが「(1)まずpoem版コードを削除」と書いたが、これは
  誤り。`crates/open-runo-gateway/src/main.rs`が今も
  `open_runo_router::build_app`(poem版)と`poem::Route::nest`で
  REST+GraphQLを合成しており、gateway側を先にhyper_compat化しない限り
  `build_app`は削除できない(gatewayが壊れる)。**正しい順序**:
  gatewayをhyper_compat対応させる(またはgatewayだけpoem依存を意図的に
  残す判断をする)→その後にopen-runo-router側のpoem版コードを削除。
  gateway自体は`async-graphql-poem`(GraphQLエンドポイント)にも依存して
  いるため、gatewayの完全hyper化にはGraphQL実行パスも生hyperで受ける
  実装が必要で、これは新たに大きめの作業。今回のパスではコード変更は
  行っていない(調査・計画の訂正のみ)。
  次回パスがすべきこと(選択肢、要判断): (A) gatewayも完全にhyper化する
  (async-graphql自体はpoem非依存で使えるはずなので、async-graphql-poem
  だけを剥がしてasync-graphql生のexecutorをhyperハンドラから呼ぶ形に
  書き換える)、(B) open-runo-router単体バイナリ(REST専用、main.rsは
  既にhyper化済み)を主軸とし、gateway(REST+GraphQL統合バイナリ)は
  当面poem依存のまま残す(2バイナリ構成なので独立してビルド・デプロイ
  可能、routerクレート本体からpoem依存が切り離せないという意味では
  ないことに注意 — Cargo.tomlのpoem依存自体はgatewayが
  `open_runo_router::build_app`をimportしている限りrouterクレートの
  lib.rsに残り続ける)。**ユーザーに選択肢を確認するか、(B)を採用して
  ひとまずWASMフロントエンド設計に進むのが妥当**(router単体は既に
  実バイナリでpoem-free動作確認済みという十分な成果があるため)。
  この判断がつくまで、`build_app`/`handlers/*.rs`(旧)/`auth.rs`/
  `middleware/cors.rs`/`rate_limit.rs`は削除しないこと。

- **2026-07-11 main.rs切替完了 — open-runo-routerバイナリが実際に
  poem-freeスタックで起動・稼働することを確認**: `main.rs`を
  `build_app`(poem版)+`poem::Server`から`build_hyper_app`+
  `hyper_compat::serve`起動に切替。`config.bind_addr`を
  `std::net::SocketAddr`にparseして使用。**実際に`cargo run -p
  open-runo-router`でバイナリを起動し**(`OPEN_RUNO_BIND_ADDR=
  127.0.0.1:18080`)、`curl`で実通信確認:
  `GET /health`→200 `{"status":"ok",...}`、
  `GET /api/db/status`(x-api-key付き)→200
  `{"backend":"in-memory","status":"ok"}`、
  `GET /api/db/status`(キー無し)→401。tracing経由のリクエストログも
  正しく出力されることを確認(`method/path/status`付きJSON)。
  型チェックだけでなく実バイナリの実通信で動作確認済み。
  `cargo test --workspace --no-run`もgreen(既存テストに影響なし)。
  open-runo側へのミラーはこのパス直後に実施予定。
  **poemはまだCargo.tomlに残っている**(build_app/poem版handlers/
  auth.rs/middleware/cors.rs/rate_limit.rsが依然コンパイルされている
  ため)。これらはもう使われていない(main.rsから参照が外れた)ので
  安全に削除できる状態。
  次回パスがすべきこと: (1) 未使用になったpoem版コード
  (`build_app`/`build_app_with_auth`/`handlers/*.rs`(旧)/`auth.rs`/
  `middleware/cors.rs`/`rate_limit.rs`、および対応する`#[cfg(test)]
  mod tests`ブロック)を削除、(2) `Cargo.toml`から`poem`依存
  (`[dependencies]`と`[dev-dependencies]`両方)を削除、(3) 削除後に
  `cargo check -p open-runo-router` / `cargo test -p open-runo-router`
  で健全性確認、(4) `open-runo-gateway`クレート(`main.rs`/`lib.rs`が
  `poem::Route::nest`でこのクレートの`build_app()`を合成している)を
  hyper_compat向けに更新 — gateway自体は`async-graphql-poem`にも
  依存しているため、GraphQLエンドポイントもhyper直接実装に移行するか、
  gatewayだけpoem依存を残すか判断が必要、(5) 全体の
  `cargo check --workspace` / `cargo test --workspace --no-run`で最終
  確認・commit・push(両リポジトリ)、(6) その後、新方針のWASM
  フロントエンド設計に着手。

- **2026-07-11 build_hyper_app() 実装 — poem-freeスタックが実際にHTTPで
  全ルート動作するところまで到達**: `middleware_hyper.rs`に
  `with_tracing`(リクエストログ)、`with_shared_rate_limit`+
  `build_rate_limiter`(全ルートで1つのRateLimiterを共有するよう
  `with_rate_limit`をリファクタ — 各ルートが別々のリミッタを持つと
  グローバルなレート制限にならないバグを先に発見・修正)を追加。
  `lib.rs`に`pub fn build_hyper_app(state, rate_limit_max,
  rate_limit_window_secs) -> hyper_compat::Router`を新設し、
  health/healthz含む全34ルートをhandlers_hyper側の関数で登録、
  各ルートを`with_cors(with_tracing(with_shared_rate_limit(h)))`で
  ラップ(KeyGuardian/HtmlPageCache/maintenance::spawnの起動時配線も
  ここに移動)。統合テスト2本(`hyper_app_tests`): 実サーバーを起動し
  health(200)・認証必須ルート(401→200)・CORSヘッダ存在・複数ルートに
  またがる共有レート制限の枯渇、をすべて実HTTP経由で検証、green。
  **`build_app`(poem版)はそのまま残しており、`main.rs`はまだpoem版を
  起動している**(`build_hyper_app`は今のところlib.rsに存在するだけで
  どこからも呼ばれていない、次のステップで main.rs を切り替える)。
  HTMLキャッシュのミドルウェアラップ(purge以外のGETルートでのキャッシュ
  ヒット判定)はまだ未統合 — page_cacheインスタンス自体は作って
  purge/backup系ハンドラには渡しているが、GETルートを透過的にキャッシュ
  する機能は今回のbuild_hyper_appには含まれていない(元々
  `OPEN_RUNO_HTML_CACHE`未設定時はデフォルト無効なので機能的な差は
  現状の既定動作では出ない)。
  `cargo test -p open-runo-router`で103テスト全green(poem版・
  hyper_compat版・統合テストすべて)。`cargo check --workspace` /
  `cargo test --workspace --no-run`もgreen。open-runo側へのミラーは
  このパス直後に実施予定。
  次回パスがすべきこと: (1) `main.rs`を`hyper_compat::serve`+
  `build_hyper_app`起動に切り替える(config.bind_addrをSocketAddrに
  parseし、`tokio::signal`等でのgraceful shutdownは後回しでまず起動
  確認を優先)、(2) 切り替え後に実際に`cargo run -p open-runo-router`で
  起動し`curl`等で疎通確認、(3) 確認できたら`build_app`/
  `build_app_with_auth`(poem版)と旧`handlers/*.rs`・`auth.rs`・
  `middleware/cors.rs`・`rate_limit.rs`を削除、`Cargo.toml`から`poem`
  依存を削除、(4) HTMLキャッシュの透過的GETキャッシュ機能が必要なら
  `with_html_cache`関数コンビネータとして追加実装、(5)
  `open-runo-gateway`側の`Route::nest`統合をhyper_compat向けに更新
  (GraphQLエンドポイントの扱いは要検討)、(6) その後、新方針のWASM
  フロントエンド設計に着手。

- **2026-07-11 CORS・レートリミットmiddlewareを関数コンビネータで移植**:
  新規`crates/open-runo-router/src/middleware_hyper.rs`に
  `with_cors(inner: Handler) -> Handler`(OPTIONSプリフライトに直接応答、
  全レスポンスにCORSヘッダ付与、`OPEN_RUNO_CORS_ALLOWED_ORIGINS`環境変数
  対応)と`with_rate_limit(inner, max, window) -> Handler`
  (`open_runo_security::RateLimiter`をそのまま利用、X-Forwarded-For/
  X-Real-IPでキー分離)を追加。poemの`Middleware<E>`traitは使わず、
  「Handlerを受け取りHandlerを返す」関数として実装(HANDOFFで前回パスが
  推奨した設計方針どおり)。テスト3本(preflight応答、レート制限到達、
  キー別バジェット分離)。`cargo test -p open-runo-router middleware_hyper`
  で3テスト全green。`cargo check --workspace` / `cargo test --workspace
  --no-run`もgreen。open-runo側へのミラーはこのパス直後に実施予定。
  次回パスがすべきこと: (1) tracing相当(リクエストログ、poemの
  `poem::middleware::Tracing`)を`with_tracing(inner) -> Handler`として
  簡易実装、(2) HTMLキャッシュミドルウェア(middleware/html_cache.rs、
  最も複雑・singleflight+stale-while-revalidate)を関数コンビネータへ
  移植(これが最後の中間層)、(3) lib.rsに`build_hyper_app() ->
  hyper_compat::Router`を新設し、全hyper_compat版handlerを
  with_cors→with_rate_limit→with_tracing→(html_cache該当ルートのみ)の
  順で合成・登録、KeyGuardian/maintenance::spawn等の起動時配線も移す、
  (4) main.rsを`hyper_compat::serve`起動に切替、(5) `Cargo.toml`から
  `poem`依存を削除、(6) `open-runo-gateway`側の`Route::nest`統合を
  hyper_compat向けに更新(GraphQLエンドポイントの扱いは要検討)、
  (7) 移行完了後、旧`handlers/*.rs`・`auth.rs`・`middleware/*.rs`
  (poem版)を削除、(8) その後、新方針のWASMフロントエンド設計に着手。

- **2026-07-11 events(SSE)を移植 — 全ハンドラ移行完了**:
  `hyper_compat.rs`の`Body`型を`Full<Bytes>`固定から
  `BoxBody<Bytes, Infallible>`(http_body_util::combinators::BoxBody)に
  変更し、固定レスポンスとストリーミングレスポンスを同じ`Response`型で
  扱えるようにした(`json_response`/`empty_status`は`fixed_body()`
  ヘルパー経由でboxする形に更新)。新規`SseEvent`型と`sse_response()`
  (`futures::stream::Stream<Item=SseEvent>`→`text/event-stream`、
  `StreamBody`+`Frame::data`で実装、poem::web::sse::SSEの素の代替)を
  追加。`stream_events_handler`(GET /api/events、15秒heartbeat +
  history変化検知、poem版と同一ロジック)を`handlers_hyper.rs`に追加。
  **ハマった点**: (1) `BodyExt::boxed`と`StreamExt::boxed`が両方scope内で
  曖昧衝突 → `BodyExt::boxed(...)`と明示呼び出しで解消、(2) `boxed()`は
  `Send + Sync + 'static`を要求するため`sse_response`のジェネリック境界に
  `Sync`を追加。テスト2本(content-type確認、401)。
  `cargo test -p open-runo-router hyper`で40テスト全green。
  `cargo check --workspace` / `cargo test --workspace --no-run`もgreen。
  open-runo側へのミラーはこのパス直後に実施予定。

  **これでhandlers/*.rs 9ファイル全てのhyper_compat版が揃った**
  (schemas/federation/ai_routing/db/persisted_queries/cache/
  maintenance/scim/events)。ただし**lib.rsのbuild_appはまだpoem版のまま
  切り替えていない**— 実際に動くバイナリは依然poem経由。

  次回パスがすべきこと(最終フェーズ): (1) 認証(auth.rs)・CORS
  (middleware/cors.rs)・レートリミット(rate_limit.rs)・HTMLキャッシュ
  ミドルウェア(middleware/html_cache.rs)を関数コンビネータ形式で
  hyper_compat向けに再実装(現状はhandlers_hyper.rs内でX-Api-Keyのみ
  個別チェックしており、CORS/rate-limit/tracing/html-cacheは未実装)、
  (2) lib.rsに`build_hyper_app() -> hyper_compat::Router`を新設し、
  全hyper_compat版handlerを登録、(3) main.rsを`hyper_compat::serve`
  起動に切り替え(poem版`build_app`/`poem::Server`は削除またはfeature
  flagで残す)、(4) `Cargo.toml`から`poem`依存を削除、(5)
  `open-runo-gateway`側の`Route::nest`統合をhyper_compat向けに更新
  (async-graphql-poemの扱いも要検討 — gatewayクレート自体もpoem依存の
  ままか、GraphQLエンドポイントも別途移行するか判断が必要)、(6) 移行
  完了後、旧`handlers/*.rs`・`auth.rs`・`middleware/*.rs`(poem版)を削除、
  (7) その後、新方針のWASMフロントエンド設計に着手。

- **2026-07-10 SCIM 10本を移植(最も複雑なグループ完了)**:
  `scim_list_users_handler`/`scim_create_user_handler`(KeyGuardian
  auto-issue含む)/`scim_get_user_handler`/`scim_replace_user_handler`
  (auto-revoke含む)/`scim_delete_user_handler`(auto-revoke含む)/
  Groups側4本(list/create/get/replace/delete)を追加。KeyGuardianとの
  自動連携(SCIM provisioning→キー自動発行、deactivate/delete→キー
  自動失効)もそのまま移植。テスト3本(scim_user_lifecycle_roundtrip、
  scim_group_lifecycle_roundtrip、key_guardian_full_auto_lifecycle —
  いずれも元のlib.rsテストと同等の検証範囲)。
  `cargo test -p open-runo-router hyper`で38テスト全green(複雑な
  auto-issue/auto-revokeのライフサイクルテストも含めて一発green)。
  `cargo check --workspace` / `cargo test --workspace --no-run`もgreen。
  open-runo側へのミラーはこのパス直後に実施予定。
  **残るはevents(SSE)のみ**。これでhandlers/*.rsの全9ファイルのうち
  8ファイル分(schemas/federation/ai_routing/db/persisted_queries/cache/
  maintenance/scim)がhyper_compat側に移植済み。
  次回パスがすべきこと: (1) events(handlers/events.rs、SSEストリーミング
  `GET /api/events`)を移植 — `hyper_compat`にchunked/SSEレスポンス
  ヘルパー(`text/event-stream`、`Response`のbodyをstreamにする仕組み)の
  追加が必要、既存poem版は`poem::web::sse::{Event, SSE}`を使用しており
  hyperではストリーミングBodyの実装(`http_body_util::StreamBody`等)が
  必要になる、(2) 移植できたらcargo test→commit→push→open-runo側にも
  ミラー、(3) 全ハンドラ移行後にlib.rsのbuild_appをhyper_compat版
  Routerに切替(auth/cors/rate_limit/html_cacheミドルウェアも関数
  コンビネータとして実装しつつ)・main.rsをhyper_compat::serve起動に
  変更・Cargo.tomlからpoem削除・open-runo-gateway側の統合更新、
  (4) その後、新方針のWASMフロントエンド設計に着手。

- **2026-07-10 backup/migrate/integrity 6本を移植**:
  `backup_export_handler`/`backup_import_handler`/
  `backup_restore_latest_handler`/`migrate_export_sql_handler`/
  `migrate_export_csv_handler`/`integrity_check_handler`を追加。
  すべて`crate::maintenance`のpoem非依存ヘルパー関数(export_backup/
  import_backup/find_latest_backup/export_sql/export_csv/
  write_to_backup_dirs)をそのまま呼ぶだけで素直に移植できた。
  テスト2本追加(integrity_check応答確認、backup export→import
  roundtrip・OPEN_RUNO_BACKUP_DIR環境変数使用)。
  `cargo test -p open-runo-router hyper`で35テスト全green。
  `cargo check --workspace` / `cargo test --workspace --no-run`もgreen。
  open-runo側へのミラーはこのパス直後に実施予定。
  **残りはSCIM(最も複雑)とevents(SSE)の2グループのみ**。
  次回パスがすべきこと: (1) SCIM(handlers/scim.rs、Users/Groups の
  CRUD、KeyGuardianとの自動連携・RBAC寄り)を移植、(2) events
  (handlers/events.rs、SSEストリーミング、`hyper_compat`にchunked/SSE
  レスポンスヘルパーの追加が必要)を移植、(3) 1つ増やすたびにcargo
  test→commit→push→open-runo側にもミラー、(4) 全ハンドラ移行後に
  lib.rsのbuild_appをhyper_compat版Routerに切替・main.rsを
  hyper_compat::serve起動に変更・Cargo.tomlからpoem削除・
  open-runo-gateway側の統合更新、(5) その後、新方針のWASM
  フロントエンド設計に着手。

- **2026-07-10 cache管理系3本を移植**: `purge_page_handler`/
  `purge_all_pages_handler`/`ai_stats_handler`を追加。`HtmlPageCache`は
  元々poem非依存な設計だったため素直に移植できた。テスト2本
  (purge/purge-all roundtrip、ai-stats無効時の応答)。**ハマった点**:
  `HtmlCacheConfig::from_env()`は`ai`フィールドが未設定時デフォルトtrueに
  なる仕様(min-hitsフィルタよりAI予測がデフォルト)ため、テストで
  ai_enabled=falseを検証する際は明示的に`config.ai = false`を設定する
  必要があった(最初`from_env()`のみで書いて1件失敗、修正済み)。
  `cargo test -p open-runo-router hyper`で33テスト全green。
  `cargo check --workspace` / `cargo test --workspace --no-run`もgreen。
  open-runo側へのミラーはこのパス直後に実施予定。
  次回パスがすべきこと: (1) backup/migrate/integrity
  (handlers/maintenance.rs、ファイルI/Oあり、環境変数
  OPEN_RUNO_BACKUP_DIR参照)を移植、(2) SCIM(Users/Groups、CRUD+RBAC寄り
  で最も複雑、KeyGuardianとの自動連携あり)を移植、(3) events(SSE
  ストリーミング、`hyper_compat`にchunked/SSEレスポンスヘルパーの追加が
  必要)を移植、(4) 1つ増やすたびにcargo test→commit→push→open-runo側
  にもミラー、(5) 全ハンドラ移行後にlib.rsのbuild_appをhyper_compat版
  Routerに切替・main.rsをhyper_compat::serve起動に変更・Cargo.tomlから
  poem削除・open-runo-gateway側の統合更新、(6) その後、新方針のWASM
  フロントエンド設計に着手。

- **2026-07-10 Persisted Queries 2本を移植**:
  `register_persisted_query_handler`(POST /api/persisted-queries)・
  `get_persisted_query_handler`(GET /api/persisted-queries/:hash)を
  追加。`open_runo_persisted_queries::PersistedQueryStore`呼び出し・
  audit記録まで同一実装。テスト2本(register→fetch roundtrip含む
  404ケース、401)。`cargo test -p open-runo-router hyper`で31テスト
  全green。`cargo check --workspace` / `cargo test --workspace --no-run`
  もgreen。open-runo側へのミラーはこのパス直後に実施予定。
  次回パスがすべきこと: (1) cache(purge/purge-all/ai-stats)を移植、
  (2) backup/migrate/integrity(ファイルI/Oあり、環境変数
  OPEN_RUNO_BACKUP_DIR参照)を移植、(3) SCIM(Users/Groups、CRUD+RBAC寄り
  で最も複雑、KeyGuardianとの自動連携あり)を移植、(4) events(SSE
  ストリーミング、`hyper_compat`にchunked/SSEレスポンスヘルパーの追加が
  必要)を移植、(5) 1つ増やすたびにcargo test→commit→push→open-runo側
  にもミラー、(6) 全ハンドラ移行後にlib.rsのbuild_appをhyper_compat版
  Routerに切替・main.rsをhyper_compat::serve起動に変更・Cargo.tomlから
  poem削除・open-runo-gateway側の統合更新、(7) その後、新方針のWASM
  フロントエンド設計に着手。

- **2026-07-10 /api/db/:table* CRUD 4本を移植**: `db_list_handler`/
  `db_get_handler`/`db_put_handler`(DB_UPSERT_REQUEST検証+audit記録)/
  `db_delete_handler`(audit記録)を`handlers_hyper.rs`に追加。全て
  `Params`から`:table`/`:key`を取得、既存poem版と同一のJSON形状・404/200
  挙動。テスト3本追加(CRUD roundtrip、missing key 404、put時401)。
  `cargo test -p open-runo-router hyper`で29テスト全green。
  `cargo check --workspace` / `cargo test --workspace --no-run`もgreen。
  open-runo側へのミラーはこのパス直後に実施予定。
  **これでopen-runo-routerの主要データパス(schemas/federation/ai/db)は
  hyper_compat側に揃った**。残るはSCIM/persisted-queries/cache/backup/
  migrate/integrity/events(SSE)という運用系ハンドラのみ。
  次回パスがすべきこと: (1) persisted-queries(POST register/GET get、
  比較的単純)を移植、(2) cache(purge/purge-all/ai-stats)を移植、
  (3) backup/migrate/integrity(ファイルI/Oあり、環境変数
  OPEN_RUNO_BACKUP_DIR参照)を移植、(4) SCIM(Users/Groups、CRUD+RBAC寄り
  で最も複雑)を移植、(5) events(SSEストリーミング、`hyper_compat`に
  chunked/SSEレスポンスヘルパーの追加が必要)を移植、(6) 1つ増やす
  たびにcargo test→commit→push→open-runo側にもミラー、(7) 全ハンドラ
  移行後にlib.rsのbuild_appをhyper_compat版Routerに切替・main.rsを
  hyper_compat::serve起動に変更・Cargo.tomlからpoem削除・
  open-runo-gateway側の統合更新、(8) その後、新方針のWASMフロントエンド
  設計に着手。

- **2026-07-10 フロントエンド方針転換 + compose_schemasを移植**:
  ユーザー指示で**フロントエンド方針を更新**: HTML5/CSS3・TypeScript・
  Bootstrap・Node.jsのスタックを廃止し、**Rustをメイン言語として統合、
  WebAssembly (WASM)に置き換える**方針に確定(open-raid-z/
  poem-cosmo-tauri/open-runoの3リポジトリCLAUDE.md「フロントエンド」節を
  同期・push済み)。実際のWASMフロントエンド実装はまだ着手していない
  (次回以降のタスク)。バックエンド側は継続: `compose_schemas_handler`
  (POST /api/federation/compose)を移植 — `open_runo_federation::compose`/
  `detect_breaking_changes`呼び出し、federation_schemaへの書き込みまで
  同一実装。テスト2本(compose→status roundtrip、401)。
  `cargo test -p open-runo-router hyper`で26テスト全green。
  `cargo check --workspace` / `cargo test --workspace --no-run`もgreen。
  open-runo側へのミラーはこのパス直後に実施予定。
  次回パスがすべきこと: (1) db.rsのdb_list/db_get/db_put/db_delete
  (Path paramあり、PUT/DELETEはbody処理も)を移植、(2) 残りは
  SCIM/persisted-queries/cache/backup/migrate/integrity/events(SSE)
  ハンドラ、(3) 1つ増やすたびにcargo test→commit→push→open-runo側にも
  ミラー、(4) 全ハンドラ移行後にlib.rsのbuild_appをhyper_compat版
  Routerに切替・main.rsをhyper_compat::serve起動に変更・Cargo.tomlから
  poem削除・open-runo-gateway側の統合更新、(5) 上記バックエンド移行が
  落ち着いたら、新方針のWASMフロントエンド(apps/desktop配下、
  Rust→wasm32-unknown-unknown、TypeScript/Node.js不使用)の設計・実装に
  着手する。

- **2026-07-10 route_request(POST /api/ai/route)を移植**: 状態を持たない
  最もシンプルなPOSTハンドラとして`route_request_handler`を追加
  (`open_runo_ai_routing::route`呼び出しのみ、AppState不要)。テスト2本
  (200で最適provider選択、401)。`cargo test -p open-runo-router hyper`で
  24テスト全green。`cargo check --workspace` / `cargo test --workspace
  --no-run`もgreen。open-runo側へのミラーはこのパス直後に実施予定。
  次回パスがすべきこと: (1) `compose_schemas`(POST /api/federation/compose、
  federation_schemaへの書き込みあり)を移植、(2) db.rsの
  db_list/db_get/db_put/db_delete(Path paramあり、PUT/DELETEはbody処理も)
  を移植、(3) 1つ増やすたびにcargo test→commit→push→open-runo側にも
  ミラー、(4) 残りはSCIM/persisted-queries/cache/backup/migrate/
  integrity/events(SSE)ハンドラ、(5) 全ハンドラ移行後にlib.rsの
  build_appをhyper_compat版Routerに切替・main.rsをhyper_compat::serve
  起動に変更・Cargo.tomlからpoem削除・open-runo-gateway側の統合更新。

- **2026-07-10 register_schema(POST・bodyあり)を移植**:
  `handlers_hyper.rs`に`register_schema_handler`を追加 —
  `hyper_compat::read_json_body`でボディ読み取り、`validation.rs`の
  `REGISTER_SCHEMA_REQUEST`(jsonschema Validator)を直接呼んで検証
  (poem::Errorを返す`validation::validate`ラッパーは使わず
  `iter_errors`を直呼び)、`audit::record`(poem非依存な素の関数)で
  監査ログ記録、`state.events`へSchemaEvent送信、まで同一実装。
  認証待ちのactor識別は新規`actor_from_headers`(X-Api-Keyのみ、
  audit.rsの`actor_from`のClaims対応版は未移植)。テスト3本追加
  (register+get roundtrip、不正bodyで422、キー無しで401)。
  `cargo test -p open-runo-router hyper`で22テスト全green。
  `cargo check --workspace` / `cargo test --workspace --no-run`もgreen。
  **open-runo側にも同じ2ファイル(hyper_compat.rs, handlers_hyper.rs)を
  ミラー・確認・commit・push予定**(このパスの直後に実施)。
  次回パスがすべきこと: (1) `compose_schemas`(POST /api/federation/compose)・
  `route_request`(POST /api/ai/route)をread_json_body併用で移植、
  (2) db.rsのdb_list/db_get/db_put/db_delete(Path paramあり、PUT/DELETEは
  body処理も)を移植、(3) 1つ増やすたびにcargo test→commit→push→
  open-runo側にもミラー、(4) 残りはSCIM/persisted-queries/cache/backup/
  migrate/integrity/events(SSE)ハンドラ、(5) 全ハンドラ移行後にlib.rsの
  build_appをhyper_compat版Routerに切替・main.rsをhyper_compat::serve
  起動に変更・Cargo.tomlからpoem削除・open-runo-gateway側の統合更新。

- **2026-07-10 統合方針を撤回・open-runoと同時並行開発に再確定 +
  get_schema/get_schema_history移植**: ユーザー指示で「poem-cosmo-tauriへ
  統合」方針を撤回し、**open-runoとpoem-cosmo-tauriを同時並行開発**する
  ことに再確定(CLAUDE.md冒頭・関連プロジェクト節を更新、open-raid-zの
  正本CLAUDE.mdも同期)。open-runo側の`CLAUDE.md`も「廃止」表記を撤回し
  開発再開。**このリポジトリのcrates/open-runo-router配下の
  hyper_compat.rs / handlers_hyper.rs / auth_hyper.rs と Cargo.toml差分
  (hyper/hyper-util/http-body-util/bytes/reqwest追加)をopen-runo側に
  ミラー済み**(open-runo側もcargo test -p open-runo-router hyperで
  15/15 green確認済み)。今後、実装はこちらのpoem-cosmo-tauriで先行させ、
  動作確認できたファイルをopen-runoへコピーする運用とする。
  加えて `query_params`/`percent_decode`(hyper_compat.rs)、
  `get_schema_handler`/`get_schema_history_handler`(handlers_hyper.rs、
  Path+Queryパラメータ対応)を追加。`cargo test -p open-runo-router hyper`
  で19テスト全green(両リポジトリとも)。`cargo check --workspace` /
  `cargo test --workspace --no-run` もgreen。
  次回パスがすべきこと: (1) `register_schema`(POST、bodyあり)を
  `read_json_body`ヘルパー使用で移植、(2) `compose_schemas`(POST)・
  `route_request`(POST /api/ai/route)を移植、(3) db.rsの
  db_list/db_get/db_put/db_delete(Path paramあり、PUT/DELETEはbody処理も)
  を移植、(4) 1つ増やすたびに`cargo test -p open-runo-router`→
  `cargo test --workspace --no-run`→commit→push→**open-runo側にも同じ
  ファイルをコピーして同様に確認・commit・push**(2リポジトリ同時並行の
  運用ルール)、(5) 残りはSCIM/persisted-queries/cache/backup/migrate/
  integrity/events(SSE)ハンドラ、(6) 全ハンドラ移行後にlib.rsの
  build_appをhyper_compat版Routerに切替・main.rsを`hyper_compat::serve`
  起動に変更・Cargo.tomlからpoem削除・open-runo-gateway側の統合更新
  (両リポジトリで実施)。

- **2026-07-10 open-runo-router poem→tokio/hyper 移行: db_status修正 +
  db_routing移植**: `db_status_handler`が固定文字列"in-memory"を返して
  いたのを`state.db.backend_name()`(実際のbackend trait呼び出し、
  テストモードでは同じく"in-memory"を返すので挙動は変わらないが本番でも
  正しいbackend名を返すよう修正)に変更。新規`db_routing_handler`
  (GET /api/db/routing、静的ルーティングテーブルを返す、認証必須)を
  `handlers_hyper.rs`に追加、テスト2本(200/401)追加。
  `cargo test -p open-runo-router hyper` で14テスト全green。
  `cargo check --workspace` / `cargo test --workspace --no-run` もgreen。
  **このパスから「20〜30分おきのスケジュール待ちにせず1パス内で連続して
  進める」運用ルールを追加**(ユーザー指示、CLAUDE.md運用ルール節参照)。
  次回パスがすべきこと: (1) `get_schema`/`get_schema_history`/
  `get_persisted_query`(認証必須GET、Path paramあり)を同パターンで移植、
  (2) POST/PUT/DELETE系(register_schema, compose_schemas, db_put/
  db_delete等)を`read_json_body`ヘルパー併用で移植、(3) 1つずつ増やす
  たびに`cargo test -p open-runo-router`→`cargo test --workspace
  --no-run`→commit→push、(4) 全ハンドラ移行後にlib.rsのbuild_appを
  hyper_compat版Routerに切替・main.rsを`hyper_compat::serve`起動に変更、
  (5) Cargo.tomlからpoem削除・open-runo-gateway側の`Route::nest`合成
  コードを追従。

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
