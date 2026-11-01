# RPoem — 開発ルール(AI向け)

- 管理APIの定義の正本は crates/open-runo-proto/proto/.../admin.proto の1つだけです。
- 整形は cargo fmt -p <クレート> を使い、--all は使いません(兄弟リポジトリまで整形されるため)。
- 未追跡の RPoem/ と target-crossui/ はコミットしません。秘密(鍵・トークン)はリポジトリやログに出しません。
- 完成と報告する前に、実際のブラウザで表示・操作を確認します(白画面を見逃さない)。
- VPSへは git pull のみ。サービスの再起動やポート公開は、ユーザーの明示的な指示があるときだけ行います。

> 日本語 · 2026-11-01 時点の要約 / summary of the state on 2026-11-01. 正本・詳細: リポジトリ直下の CLAUDE.md (ja/en).
