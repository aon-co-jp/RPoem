# Changelog

All notable changes to open-runo are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/) once a
`1.0.0` is cut. Before `1.0.0`, breaking changes may land in minor versions.

## [Unreleased]

### Added

- `open-runo-router`: Axum(`multer`等)が担う領域の不足分を追加。
  `multipart_stream`(チャンクごとのストリーミングmultipart、合計/1フィールド/件数/ヘッダの上限、
  チャンク分割位置の総当たり試験済み)、`webhook`(GitHub/Slack/汎用のHMAC-SHA256署名検証、
  鍵ローテーション、タイムスタンプ窓、配送IDの重複拒否)、`body_limit`(`Content-Length`先行拒否+
  上限つき本文読み取り)、`middleware_hyper::{with_api_key_auth, with_rate_limit_by,
  rate_limit_key_api_key_or_ip}`(APIキー別レート制限)。`open-runo-poem-compat`から再公開。
  発端: `backup-to-discord-by-github`(大きなGit bundleの受信とWebhook駆動)。
- Initial Rust workspace with 11 crates: `open-runo-core`, `open-runo-router`,
  `open-runo-federation`, `open-runo-schema-registry`, `open-runo-ai-routing`,
  `open-runo-versionless-api`, `open-runo-db`, `open-runo-backup`,
  `open-runo-history`, `open-runo-observability`, `open-runo-security`.
- `open-runo-router`: Poem-based HTTP gateway with `/health` and `/healthz`,
  structured logging via `open-runo-observability`, and per-client rate
  limiting via `open-runo-security`.
- Quality gate tooling: `rustfmt.toml`, `clippy.toml`, `deny.toml`,
  workspace-wide `[lints]` policy, `Makefile`, and GitHub Actions CI
  (`fmt`, `clippy`, `test`, `build`, `audit`, `deny`).
- `docs/` design documentation mapping each README architecture component
  to its implementing crate.
- `DEVELOPMENT.md`, `CONTRIBUTING.md`, `SECURITY.md`, dual
  Apache-2.0/MIT licensing.
- `docker-compose.yml` for local PostgreSQL + gateway development.
- `cargo run --example` for `open-runo-federation`, `open-runo-schema-registry`,
  and `open-runo-ai-routing`, doubling as living API documentation.

### Status

This is pre-`0.1.0` scaffolding work (Development Roadmap Phase 1, see
README). APIs, crate boundaries, and the compatibility guarantees implied by
`open-runo-versionless-api` do not yet apply to open-runo's own crates.

[Unreleased]: https://github.com/aon-co-jp/open-runo/compare/main...HEAD
