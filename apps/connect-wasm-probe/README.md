# connect-wasm-probe — connect-rust のブラウザ(WASM)クライアントで RPoem の管理サービスを呼ぶ実機確認

RPoem のブラウザ向け入口(`POST /open_runo.admin.v1.Admin/<Method>`)は、gRPC-web・gRPC-web-text に加えて
**Connect プロトコル**(`application/proto` / `application/json` / `application/connect+*`)も受ける
(`crates/open-runo-router/src/connect.rs`)。プロキシ無しで普通の HTTP として呼べるので、
[connect-rust](https://github.com/connectrpc/connect-rust)(`connectrpc` 0.9、WASM は fetch ベースの
`ClientTransport`)や connect-es、`curl` から直接使える。

このプローブはそれを実ブラウザで確かめるためのもの(2026-11-01 に実施)。

## 確認した結果(Chromium 系の組み込みブラウザ、別オリジンのページ → RPoem ゲートウェイ)

- `SelfIssueKey` / `RegisterSchema` / `SchemaHistory` / `DbStatus`(`google.protobuf.Struct`)が
  **proto・JSON の両コーデックで成功**。未登録スキーマは Connect の `not_found`(HTTP 404)として届く。
- CORS のプリフライト(`OPTIONS`)を通り、`x-api-key` メタデータでの認証も効く。
- **制約**: connect-rust 同梱の `FetchTransport` は応答本文を一括で読む(`Full<Bytes>`)ため、
  ブラウザからは**単項呼び出しのみ**。サーバーストリーミング(`WatchEvents`)を WASM で受けるには
  `ReadableStream` を読む独自の transport が要る(未実装)。connect-rust は pre-1.0(API が変わりうる)。

## 再現手順(connect-rust の checkout を使う。RPoem 本体は connect-rust に依存しない)

1. `git clone https://github.com/connectrpc/connect-rust` し、`examples/admin-probe/` を作って
   `Cargo.toml.example`(→`Cargo.toml`)・`build.rs.example`(→`build.rs`)・`src/` をここからコピー。
   ワークスペースの `members` に `examples/admin-probe` を足す。
2. `protoc` が無い環境でも、`protox`(純Rust)で記述子セットを作れる:
   `FileDescriptorSet`を`admin.fds`に出力(`crates/open-runo-proto/build.rs`と同じ`protox::compile`)。
3. `cargo build -p admin-probe --target wasm32-unknown-unknown --release` →
   `wasm-bindgen`(**Cargo.lock の wasm-bindgen と同じバージョンのCLI**。ずれると「schema version」エラー)で
   `--target web --out-dir pkg`。`index.html` と `pkg/` を静的配信し、
   RPoem ゲートウェイ(`cargo run -p open-runo-gateway`、`OPEN_RUNO_BIND_ADDR=127.0.0.1:8080`)を起動して
   `index.html?base=http://127.0.0.1:8080` を開く。
