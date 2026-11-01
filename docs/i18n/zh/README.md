# RPoem — 概览

- RPoem 是用 Rust 编写的 GraphQL Federation 平台(兼容 WunderGraph Cosmo)。
- REST 的 /api/ 已于 2026-11-01 全部废除。管理操作通过 gRPC(同一定义也支持 gRPC-web 和 Connect),并在同一个端口上接收。
- GraphQL 仅用于 Federation 查询;生产环境启用四项防护(强制持久化查询、深度/复杂度限制、关闭内省、拒绝批量请求)。
- gRPC 强制 TLS 1.3,可选 mTLS。浏览器界面为 WASM(382KB),带有 RPC Inspector 和开发者工具扩展(apps/devtools-extension)。
- 协议公共组件为 open-runo-connect-core / server / client(原仓库 gRPC-web-Connect 已并入 RPoem)。

> 中文 · 2026-11-01 時点の要約 / summary of the state on 2026-11-01. 正本・詳細: リポジトリ直下の README.md (ja/en).
