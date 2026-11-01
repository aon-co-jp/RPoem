# RPoem — 开发规则(面向 AI)

- 管理 API 的唯一权威定义是 crates/open-runo-proto/proto/.../admin.proto。
- 格式化使用 cargo fmt -p <crate>,不要使用 --all(会格式化兄弟仓库)。
- 不要提交未跟踪的 RPoem/ 与 target-crossui/。密钥和令牌不得出现在仓库或日志中。
- 报告完成之前,必须在真实浏览器中确认显示与操作(不要漏掉白屏)。
- 对 VPS 只做 git pull;只有在用户明确指示时才重启服务或开放端口。

> 中文 · 2026-11-01 時点の要約 / summary of the state on 2026-11-01. 正本・詳細: リポジトリ直下の CLAUDE.md (ja/en).
