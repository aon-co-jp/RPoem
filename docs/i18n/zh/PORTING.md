# RPoem — 交接

- 已完成:管理操作 gRPC 化、gRPC-web 与 Connect 混合、通信处理替换为公共 crate、WASM 瘦身(751KB→382KB)、开发者工具扩展、RustSec 扫描(version-control)。
- 未完成:open-web-server 的 HTTP/2 支持(原生 gRPC 需要改为带 trailer 的流式响应)。
- 未完成:connect-rust 的流式传输(上游尚未到 1.0,目前以自有 client crate 代替)。
- 搁置:open-web-server 的 rest-to-graphql 分支已推送但未合并(调用方需要是新版网关)。
- 恢复工作请从 CLAUDE.md 开头的方针转换(gRPC)一节和本交接开始。

> 中文 · 2026-11-01 時点の要約 / summary of the state on 2026-11-01. 正本・詳細: リポジトリ直下の PORTING.md (ja/en).
