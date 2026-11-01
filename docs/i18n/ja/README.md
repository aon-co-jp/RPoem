# RPoem — 概要

- RPoemは、Rust製のGraphQL Federation基盤(WunderGraph Cosmo互換)です。
- RESTの/api/は2026-11-01に全廃しました。管理操作はgRPC(同じ定義をgRPC-web・Connectでも受信)で、1つのポートで受けます。
- GraphQLはFederationの問い合わせ専用で、本番では4つの防御(永続化クエリ必須・深さ/複雑さ制限・イントロスペクション無効・バッチ拒否)を有効にします。
- gRPCはTLS 1.3必須、必要に応じてmTLS。ブラウザ画面はWASM(382KB)で、RPC Inspectorと開発者ツール拡張(apps/devtools-extension)を備えます。
- 通信方式の共通部品は open-runo-connect-core / server / client です(旧リポジトリgRPC-web-ConnectをRPoemへ統合)。

> 日本語 · 2026-11-01 時点の要約 / summary of the state on 2026-11-01. 正本・詳細: リポジトリ直下の README.md (ja/en).
