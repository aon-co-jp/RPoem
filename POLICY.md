# API 方針(RPoem) / API policy (RPoem)

## 日本語
正本は `aon-co-jp/runo` の `API-POLICY.md`。要点: REST は全廃し、新しい REST は作らない。管理操作は `open_runo.admin.v1.Admin`(gRPC。同じ定義を gRPC-web・Connect でも受ける)、ブラウザや `curl` からは Connect、GraphQL は問い合わせ(読み取り)専用。JSON の読み書きは RS-JSON に統一する。

- **例外(相手の規格が形を決めるもの)**: ヘルスチェック、ACME、SCIM(RFC 7644)、静的ファイル、第三者の Webhook、OTLP。
- **4姉妹**: `open-cosmo`・`RPoem`・`Reaxt`・`RPoRea` は常に同時に開発する。接点を変えたら4つの文書を同じ日に更新する。

## English
The source of truth is `API-POLICY.md` in `aon-co-jp/runo`. In short: REST is being abolished and no new REST is added. Admin operations use `open_runo.admin.v1.Admin` (gRPC, and the same definition is also accepted over gRPC-web and Connect); browsers and `curl` use Connect; GraphQL is read-only. JSON is read and written through RS-JSON only.

- **Exceptions (shapes dictated by someone else's standard)**: health checks, ACME, SCIM (RFC 7644), static files, third-party webhooks, OTLP.
- **The four sisters**: `open-cosmo`, `RPoem`, `Reaxt` and `RPoRea` are always developed together. When an interface between them changes, all four documents are updated on the same day.

## 2026-11-19 追記(続き3): 実サーバー2台での「読み取りの分割」を確認、Reaxt に読み取り用アクションを追加(4姉妹共通) / Read-query split verified on two real servers; read actions added to Reaxt (shared by the four sisters)

### 日本語
- **Reaxt の規約**: メソッド名が `Get` / `List` / `Read` で始まるアクション(例: `GetCount`、`GetVisits`)は、Cosmo 向けのサブグラフで GraphQL の **Query** として公開されます。それ以外は従来どおり **Mutation** です。変更用のアクションが1つも無いときは、SDL に `Mutation` 型を出しません。`reaxt_ready` は引き続き全サブグラフで共有されます。
- **実機での確認**: カウンター(`GetCount`)とグリーター(`GetVisits`)の2台を `open-cosmo` のゲートウェイ越しに動かし、`query { c: demo_v1_Counter_GetCount g: demo_v1_Greeter_GetVisits }` が、2台に分けて実行され、`{"data":{"c":2,"g":1}}` と1つの結果にまとまることを確認しました。2台にまたがる変更系は、従来どおり「読み取り専用の問い合わせだけが分割できる」というメッセージで拒否されます。`input:` のリテラルも実機で通りました。
- **試験**: Reaxt の `reaxt-actions` は11件に成功。
- **未了**: `@requires` / `@provides`、変更系の後の再取得。

### English
- **Reaxt convention**: actions whose method name starts with `Get`, `List` or `Read` (for example `GetCount`, `GetVisits`) are exposed by the Cosmo subgraph as GraphQL **Query** fields; all other actions remain **Mutation** fields. When there are no mutating actions, the SDL has no `Mutation` type. `reaxt_ready` is still shared by every subgraph.
- **Verified on a real machine**: with the counter (`GetCount`) and greeter (`GetVisits`) servers running behind the `open-cosmo` gateway, `query { c: demo_v1_Counter_GetCount g: demo_v1_Greeter_GetVisits }` was split across the two servers and merged into one answer, `{"data":{"c":2,"g":1}}`. A mutation spanning both is still rejected, now with the message that only read-only queries can be split. `input:` literals also worked on the real servers.
- **Tests**: `reaxt-actions` passes 11 tests.
- **Open**: `@requires` / `@provides`, and re-fetching after a mutation.
