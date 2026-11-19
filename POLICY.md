# API 方針(RPoem) / API policy (RPoem)

## 日本語
正本は `aon-co-jp/runo` の `API-POLICY.md`。要点: REST は全廃し、新しい REST は作らない。管理操作は `open_runo.admin.v1.Admin`(gRPC。同じ定義を gRPC-web・Connect でも受ける)、ブラウザや `curl` からは Connect、GraphQL は問い合わせ(読み取り)専用。JSON の読み書きは RS-JSON に統一する。

- **例外(相手の規格が形を決めるもの)**: ヘルスチェック、ACME、SCIM(RFC 7644)、静的ファイル、第三者の Webhook、OTLP。
- **4姉妹**: `open-cosmo`・`RPoem`・`Reaxt`・`RPoRea` は常に同時に開発する。接点を変えたら4つの文書を同じ日に更新する。

## English
The source of truth is `API-POLICY.md` in `aon-co-jp/runo`. In short: REST is being abolished and no new REST is added. Admin operations use `open_runo.admin.v1.Admin` (gRPC, and the same definition is also accepted over gRPC-web and Connect); browsers and `curl` use Connect; GraphQL is read-only. JSON is read and written through RS-JSON only.

- **Exceptions (shapes dictated by someone else's standard)**: health checks, ACME, SCIM (RFC 7644), static files, third-party webhooks, OTLP.
- **The four sisters**: `open-cosmo`, `RPoem`, `Reaxt` and `RPoRea` are always developed together. When an interface between them changes, all four documents are updated on the same day.
