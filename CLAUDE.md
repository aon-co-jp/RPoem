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

## 現状(このリポジトリ固有)

- `cargo check --workspace` / `cargo test --workspace --no-run` は成功する
  (15クレート構成、テストコンパイル済み)。todo!()/unimplemented!()マーカーなし。
- README多言語版は `README-<言語>.md` 形式で日本語・英語・中国語簡体字・
  韓国語・スペイン語・フランス語・ドイツ語・イタリア語・ロシア語・
  アラビア語の10言語が揃っている。

## HANDOFF(直近の自動実行パス)

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
