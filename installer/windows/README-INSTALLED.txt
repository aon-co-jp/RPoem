open-runo-router (RPoem) — インストール後のご案内 / Post-install notes
========================================================================

日本語
------
open-runo-router.exe は、open-web-server(第二のApache+Nginx)とSETで
使うアプリケーションサーバー(第二のTomcat)です。既定では
0.0.0.0:8081 で待ち受けます(OPEN_RUNO_ROUTER_BIND環境変数で変更可)。

- 自動アップデート機能は既定で無効です。有効化するには環境変数
  OPEN_RUNO_ROUTER_SELF_UPDATE=true を設定してから起動してください
  (src/self_update.rs のモジュールコメントに正直な開示あり)。
- GUIは持たないバックエンドサービスです。常駐させるにはWindows
  サービス化(nssm等の別ツール)またはタスクスケジューラでの起動を
  別途設定してください(本インストーラーのスコープ外)。

English
-------
open-runo-router.exe is the application-server counterpart (a "second
Tomcat") used together with open-web-server. It listens on
0.0.0.0:8081 by default (override with OPEN_RUNO_ROUTER_BIND).

- Automatic self-update is OFF by default. Set
  OPEN_RUNO_ROUTER_SELF_UPDATE=true before launching to opt in (see
  the module doc in src/self_update.rs for the full disclosure).
- This is a headless backend service with no GUI. To keep it running
  persistently, set it up as a Windows service (e.g. via nssm) or a
  Scheduled Task yourself — that is outside this installer's scope.
