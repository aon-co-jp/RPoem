# RPoem — 引き継ぎ

- 完了: 管理操作のgRPC化、gRPC-web・Connectのハイブリッド、通信処理の共通クレートへの置換、WASM縮小(751KB→382KB)、開発者ツール拡張、RustSec走査(version-control)。
- 未了: open-web-serverのHTTP/2対応(素のgRPCにはトレーラー付きストリーミング応答への作り替えが必要)。
- 未了: connect-rustのストリーミング転送(上流が1.0前のため、自前のclientクレートで代替中)。
- 保留: open-web-serverの rest-to-graphql ブランチはpush済みだが未マージ(呼び出し先が新しいゲートウェイである必要がある)。
- 再開は CLAUDE.md 冒頭の方針転換(gRPC)の節と、この引き継ぎから。

> 日本語 · 2026-11-01 時点の要約 / summary of the state on 2026-11-01. 正本・詳細: リポジトリ直下の PORTING.md (ja/en).
