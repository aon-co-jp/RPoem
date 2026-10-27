//! `poem`クレートのAPI面(`Route::new().at(path, get(h).post(h2))`・
//! `Server::new(TcpListener::bind(addr)).run(app)`・`Json<T>`)を、
//! 外部プロジェクトからそのまま使える形で提供する互換ファサード。
//!
//! **正直な開示**: これは`poem`のソースコードを流用したものでも、
//! `poem::Endpoint`トレイトを実装したものでもない。実体は
//! [`open_runo_router::hyper_compat`](open_runo_router::hyper_compat)
//! (tokio/hyperを直接使う自前実装)を、poemと**同じ名前・同じ呼び出し
//! 形状**で薄くラップしただけのシムである。したがって:
//! - `poem::Endpoint`/`poem::FromRequest`など、poem本体のトレイトを
//!   実装した既存コード(poemのミドルウェアエコシステム等)とは
//!   互換性が無い。
//! - ここで再現しているのは「ルーティングDSLの書き味」と
//!   「サーバー起動の書き味」のみ。CORS・gzip・WebSocket等の個別機能は
//!   `open_runo_router::middleware_hyper`/`hyper_compat`側に実体があり、
//!   本クレートは今回それらを配線していない(今後の増分)。
//! - 2026-07-22時点、このモジュールはRS-Git等の実プロジェクトでは
//!   まだ使われていない(単体テストのみで検証済み)。「poemを完全に
//!   置き換えられる」という主張はしない——実際に使われ、実プロジェクト
//!   規模で検証されるまでは、あくまで「互換API面の第一歩」の位置づけ。

// clippy: このモジュールの unwrap()/expect() は「固定ヘッダからのレスポンス構築」
// 「起動時に読む埋め込み定数のパース」等、構造上失敗しない箇所に限られる
// (各呼び出しに理由コメントあり)。business ロジック側の Quality Gate
// (workspace lints の unwrap_used/expect_used = warn) は維持したまま、
// この基盤モジュールだけ除外する。result_large_err は Result<T, Response>
// (エラー時に完成済み HTTP レスポンスで短絡する意図的な型)に対するもの。
#![allow(clippy::result_large_err)]

use bytes::Bytes;
pub use open_runo_router::hyper_compat::Params;
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;

pub use hyper::Method;
pub use hyper::StatusCode;
/// 逃げ道として`hyper_compat`自体も再公開する——CORS/gzip/WebSocket等、
/// 本ファサードがまだ配線していない機能は、当面この経路で直接使える。
pub use open_runo_router::hyper_compat;
/// 型駆動のAPIドキュメント生成(`Spec`/`Operation`、poem-openapi相当)。
pub use open_runo_openapi as openapi;

/// 型駆動のAPIハンドラ(`api::json`/`api::query`/`api::plain` 等)と `ApiError`/`Ctx`。
pub mod api;
pub use api::{ApiError, ApiResult, Ctx};
/// 実サーバーを立ててHTTPで叩く、テスト用のクライアント。
pub mod testing;
pub use open_runo_router::middleware_hyper;
/// 大きなアップロードをチャンクごとに受けるストリーミングmultipart、
/// Webhookの署名検証つき受信、本文サイズ上限(Axumの`multer`/
/// `DefaultBodyLimit`/署名検証ミドルウェアが担う領域)。
pub use open_runo_router::{body_limit, multipart_stream, webhook};

pub type Request = hyper_compat::Request;
pub type Response = hyper_compat::Response;
pub type Handler = hyper_compat::Handler;
pub type BoxFuture<T> = hyper_compat::BoxFuture<T>;

pub fn empty_status(status: StatusCode) -> Response {
    hyper_compat::empty_status(status)
}

pub fn empty_status_ok() -> Response {
    hyper_compat::empty_status(StatusCode::OK)
}

/// poemのWebSocketハンドラ相当。実体は`hyper_compat::websocket_handler`
/// (RFC 6455手書き実装)への直接委譲——本クレートは独自実装を持たず、
/// 既存機能をそのまま`get(websocket_handler(f))`という書き味で使える
/// ようにするだけ。
pub use hyper_compat::{websocket_handler, WsMessage};

/// poemの`Route::new().at(path, get(handler).post(handler2))`と同じ
/// 書き味を提供する、`hyper_compat::Router`のビルダーラッパー。
#[derive(Default)]
pub struct Route {
    entries: Vec<(String, MethodRouter)>,
    docs: Option<(String, openapi::Spec)>,
    api_ops: Vec<openapi::Operation>,
    data: std::collections::HashMap<std::any::TypeId, Arc<dyn std::any::Any + Send + Sync>>,
    max_body: Option<usize>,
}

impl Route {
    pub fn new() -> Self {
        Self::default()
    }

    /// poemの`.at(path, method_router)`と同じ形状。同一パスへの複数回
    /// `.at()`呼び出しは、それぞれ別エントリとして保持する(poemは
    /// 内部でマージするが、本シムでは単純化のため後勝ちにはせず、
    /// 呼び出し側が1パス1回で組み立てる前提とする——正直な簡略化)。
    pub fn at(mut self, path: &str, method_router: MethodRouter) -> Self {
        self.add_entry(path, method_router);
        self
    }

    /// 同一パスは1エントリにまとめる(poemと同じ。`.at("/x", get(a)).at("/x", post(b))` が両方効く)。
    fn add_entry(&mut self, path: &str, method_router: MethodRouter) {
        if let Some((_, existing)) = self.entries.iter_mut().find(|(p, _)| p == path) {
            existing.handlers.extend(method_router.handlers);
        } else {
            self.entries.push((path.to_string(), method_router));
        }
    }

    /// 型駆動のハンドラを登録する(`api::json` 等)。メソッド・パスは宣言(`Operation`)から取り、
    /// 文書(`.docs(..)`)には引数・戻り値の型からそのまま載る。
    pub fn api(mut self, h: api::ApiHandler) -> Self {
        let method = api::to_hyper_method(h.op.http_method());
        let pattern = h.op.route_pattern();
        self.add_entry(&pattern, MethodRouter { handlers: vec![(method, h.handler)] });
        self.api_ops.push(h.op);
        self
    }

    /// `prefix` の下に別の `Route` をまとめて載せる(poemの`nest`)。型付きの宣言も
    /// パスに接頭辞を付けて引き継ぐ。入れ子側の `.docs(..)` は無視され、外側の文書に載る。
    pub fn nest(mut self, prefix: &str, inner: Route) -> Self {
        let p = prefix.trim_end_matches('/');
        for (path, mr) in inner.entries {
            let full = if path == "/" { if p.is_empty() { "/".to_string() } else { p.to_string() } } else { format!("{p}{path}") };
            self.add_entry(&full, mr);
        }
        for op in inner.api_ops {
            self.api_ops.push(op.prefixed(p));
        }
        self
    }

    /// 型駆動ハンドラ(`api::json` 等)が読むJSON本文の上限(バイト、既定2MiB)。
    /// 超えると413。
    pub fn max_body(mut self, bytes: usize) -> Self {
        self.max_body = Some(bytes);
        self
    }

    /// アプリ共有の値を登録する(同じ型は後勝ち)。ハンドラ内で `ctx.data::<T>()` / `ctx.state::<T>()?` で取り出す。
    /// `Arc` の複製をクロージャへ配る手間が要らない。
    pub fn data<T: Send + Sync + 'static>(mut self, value: T) -> Self {
        self.data.insert(std::any::TypeId::of::<T>(), Arc::new(value));
        self
    }

    /// 登録済みの全ハンドラへミドルウェア(`Handler -> Handler`)を適用する。
    /// 例: `.with(|h| middleware_hyper::with_tracing(h))`。
    pub fn with(mut self, mw: impl Fn(Handler) -> Handler) -> Self {
        for (_, mr) in self.entries.iter_mut() {
            for (_, handler) in mr.handlers.iter_mut() {
                *handler = mw(handler.clone());
            }
        }
        self
    }

    /// リクエストごとの開始/終了ログ(`middleware_hyper::with_tracing`)。
    pub fn with_tracing(self) -> Self {
        self.with(middleware_hyper::with_tracing)
    }

    /// poemの`.with(Cors::new())`相当。登録済みの全ハンドラを
    /// `middleware_hyper::with_cors`で包む(`build()`が別途行う
    /// `OPTIONS`プリフライトの自動登録とは独立した処理——プリフライト
    /// 応答自体もCORSヘッダを付けたいので両方必要)。
    pub fn with_cors(mut self) -> Self {
        for (_, mr) in self.entries.iter_mut() {
            for (_, handler) in mr.handlers.iter_mut() {
                *handler = middleware_hyper::with_cors(handler.clone());
            }
        }
        self
    }

    /// poemの`.with(Compression::new())`相当(gzip、`Accept-Encoding`次第)。
    pub fn with_compression(mut self) -> Self {
        for (_, mr) in self.entries.iter_mut() {
            for (_, handler) in mr.handlers.iter_mut() {
                *handler = middleware_hyper::with_compression(handler.clone());
            }
        }
        self
    }

    /// poem-openapi相当: 型付きの `Spec` から OpenAPI 3.0 / Swagger UI / ReDoc /
    /// Cosmo互換GraphQL SDL / Tauriマニフェスト / TypeScriptクライアントを
    /// `prefix`(例: `"/api"`)の下で配信する。`build()` 時に、ここへ登録した
    /// ルートで `spec` に宣言が無いものを標準エラーへ警告する(書き忘れ検出)。
    pub fn docs(mut self, prefix: &str, spec: openapi::Spec) -> Self {
        self.docs = Some((prefix.to_string(), spec));
        self
    }

    /// `docs` の最小形: 型付きの宣言を1つも書かず、登録済みルートから
    /// 文書を自動で組み立てる(段階的に `docs` へ移行できる)。
    pub fn docs_auto(self, prefix: &str, title: &str, version: &str) -> Self {
        self.docs(prefix, openapi::Spec::new(title, version))
    }

    pub fn build(self) -> hyper_compat::Router {
        let mut router = hyper_compat::Router::new();
        let mut docs = self.docs;
        if let Some((_, spec)) = docs.as_mut() {
            // 型駆動ハンドラ(`Route::api`)の宣言を先に文書へ入れる(引数・戻り値の型がそのまま載る)。
            for op in &self.api_ops {
                let m = api::to_hyper_method(op.http_method());
                if !spec.undocumented([(m.as_str(), op.route_pattern().as_str())]).is_empty() {
                    *spec = std::mem::replace(spec, openapi::Spec::new("", "")).op(op.clone());
                }
            }
            // 型付き宣言の無いルートは、メソッドとパスだけの最小限の操作として自動で足す
            // (文書から漏れない。型を付けたくなったら `Spec` 側で宣言すれば置き換わる)。
            for (path, mr) in &self.entries {
                for (m, _) in &mr.handlers {
                    let method = match m.as_str() {
                        "GET" => openapi::Method::Get,
                        "POST" => openapi::Method::Post,
                        "PUT" => openapi::Method::Put,
                        "PATCH" => openapi::Method::Patch,
                        "DELETE" => openapi::Method::Delete,
                        _ => continue,
                    };
                    if !spec.undocumented([(m.as_str(), path.as_str())]).is_empty() {
                        eprintln!("[open-runo-poem-compat] APIドキュメント未宣言のルート(自動で最小限の操作を追加): {m} {path}");
                        let op = openapi::Operation::new(method, path)
                            .summary(&format!("{m} {path}"))
                            .tag(path.trim_start_matches('/').split('/').next().unwrap_or("root"))
                            .response_empty(200, "OK");
                        *spec = std::mem::replace(spec, openapi::Spec::new("", "")).op(op);
                    }
                }
            }
        }
        let mut data_map = self.data;
        if let Some(n) = self.max_body {
            data_map.insert(std::any::TypeId::of::<api::BodyLimit>(), Arc::new(api::BodyLimit(n)));
        }
        let data = if data_map.is_empty() { None } else { Some(Arc::new(data_map)) };
        for (path, mr) in self.entries {
            for (method, handler) in mr.handlers {
                let handler = match &data {
                    Some(d) => {
                        let d = Arc::clone(d);
                        let inner = handler;
                        let wrapped: Handler = Arc::new(move |mut req: Request, p: Params| {
                            req.extensions_mut().insert(api::DataExt(Arc::clone(&d)));
                            inner(req, p)
                        });
                        wrapped
                    }
                    None => handler,
                };
                router = router.route(method, &path, handler);
            }
        }
        if let Some((prefix, spec)) = docs {
            router = open_runo_router::openapi::mount_docs(router, &prefix, spec);
        }
        router.with_cors_preflight()
    }
}

/// poemの`MethodRouter`(`get(h).post(h2)`でメソッドを積み重ねる型)相当。
pub struct MethodRouter {
    handlers: Vec<(Method, Handler)>,
}

impl MethodRouter {
    fn single(method: Method, handler: Handler) -> Self {
        Self { handlers: vec![(method, handler)] }
    }

    pub fn get(mut self, handler: Handler) -> Self {
        self.handlers.push((Method::GET, handler));
        self
    }
    pub fn post(mut self, handler: Handler) -> Self {
        self.handlers.push((Method::POST, handler));
        self
    }
    pub fn put(mut self, handler: Handler) -> Self {
        self.handlers.push((Method::PUT, handler));
        self
    }
    pub fn delete(mut self, handler: Handler) -> Self {
        self.handlers.push((Method::DELETE, handler));
        self
    }
    /// 2026-08-24新設(open-english側の実用性向上要望——多くのHTTP
    /// クライアント/ヘルスチェックツールが`HEAD`を使うが、従来この
    /// シムには`HEAD`を登録する手段が無かった)。poemの
    /// `.head(handler)`と同じ書き味。呼び出し側がボディ無しの
    /// レスポンスを返すハンドラを渡す想定(このシム自体はGETハンドラ
    /// からボディを自動で取り除いたりはしない、正直な簡略化)。
    pub fn head(mut self, handler: Handler) -> Self {
        self.handlers.push((Method::HEAD, handler));
        self
    }
}

pub fn get(handler: Handler) -> MethodRouter {
    MethodRouter::single(Method::GET, handler)
}
pub fn post(handler: Handler) -> MethodRouter {
    MethodRouter::single(Method::POST, handler)
}
pub fn put(handler: Handler) -> MethodRouter {
    MethodRouter::single(Method::PUT, handler)
}
pub fn delete(handler: Handler) -> MethodRouter {
    MethodRouter::single(Method::DELETE, handler)
}
/// poemの`head(handler)`相当(2026-08-24新設、`MethodRouter::head`参照)。
pub fn head(handler: Handler) -> MethodRouter {
    MethodRouter::single(Method::HEAD, handler)
}

/// poemの`poem::web::Path<T>`相当。`Params`(`HashMap<String,String>`)
/// から名前付きセグメントを取り出す薄いヘルパー。
pub struct PathParams(pub HashMap<String, String>);

impl From<Params> for PathParams {
    fn from(p: Params) -> Self {
        PathParams(p.0)
    }
}

impl PathParams {
    pub fn get(&self, key: &str) -> Option<&str> {
        self.0.get(key).map(|s| s.as_str())
    }
}

/// poemの`poem::web::Json<T>`相当。レスポンス側は`json_response`へ
/// 委譲、リクエスト側は`Json::from_body`で`hyper_compat::read_json_body`
/// を呼ぶ薄いラッパー(poemのような`FromRequest`自動抽出は行わない
/// ——本シムがpoemのトレイト体系そのものを実装していないため、
/// 呼び出し側がハンドラ内で明示的に`Json::from_body(req).await?`する
/// 形になる、という正直な簡略化)。
pub struct Json<T>(pub T);

impl<T: serde::Serialize> Json<T> {
    pub fn into_response(self, status: StatusCode) -> Response {
        hyper_compat::json_response(status, &self.0)
    }
}

impl<T: serde::de::DeserializeOwned> Json<T> {
    pub async fn from_body(req: Request) -> Result<Self, Response> {
        hyper_compat::read_json_body(req).await.map(Json)
    }
}

pub fn fixed_body(bytes: Bytes) -> hyper_compat::Body {
    hyper_compat::fixed_body(bytes)
}

/// poemの`poem::listener::TcpListener::bind(addr)`+
/// `poem::Server::new(listener).run(app)`と同じ書き味を提供する。
pub struct TcpListener {
    addr: SocketAddr,
}

impl TcpListener {
    pub fn bind(addr: impl Into<SocketAddr>) -> Self {
        Self { addr: addr.into() }
    }
}

pub struct Server {
    listener: TcpListener,
}

impl Server {
    pub fn new(listener: TcpListener) -> Self {
        Self { listener }
    }

    /// poemの`.run(app)`相当。`hyper_compat::serve`へ委譲し、実際に
    /// bindされたアドレスとサーバータスクの`JoinHandle`を返す
    /// (poemの`.run()`は`await`し続けてブロックする挙動だが、本シムは
    /// 呼び出し側にシャットダウン制御の余地を残すため、あえて
    /// `serve`と同じ「起動して返す」形状にしている——poemの`.run()`を
    /// そのまま`await`したい場合は、返ってきた`JoinHandle`を呼び出し側
    /// で`.await`すればよい)。
    pub async fn run(self, app: Route) -> std::io::Result<(SocketAddr, tokio::task::JoinHandle<()>)> {
        hyper_compat::serve(app.build(), self.listener.addr).await
    }
}

/// テスト・小規模ハンドラ向けの補助: 同期クロージャからHandlerを組み立てる。
pub fn handler_fn<F, Fut>(f: F) -> Handler
where
    F: Fn(Request, Params) -> Fut + Send + Sync + 'static,
    Fut: std::future::Future<Output = Response> + Send + 'static,
{
    Arc::new(move |req, params| Box::pin(f(req, params)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use http_body_util::BodyExt;
    use hyper::{Request as HyperRequest, StatusCode as SC};

    /// `hyper_compat::Request` = `hyper::Request<Incoming>`は手組みでは
    /// 構築できない(`Incoming`は実TCPコネクションからしか生成されない
    /// hyper内部型)ため、本クレートのdispatchテストは全て実TCP経由で
    /// 行う(`hyper_compat`自身の既存テストと同じ方針)。
    async fn spawn(app: Route) -> (SocketAddr, tokio::task::JoinHandle<()>) {
        Server::new(TcpListener::bind(([127, 0, 0, 1], 0)))
            .run(app)
            .await
            .expect("server should bind an ephemeral port")
    }

    async fn send(addr: SocketAddr, method: Method, path: &str) -> hyper::Response<hyper::body::Incoming> {
        let stream = tokio::net::TcpStream::connect(addr).await.unwrap();
        let io = hyper_util::rt::TokioIo::new(stream);
        let (mut sender, conn) = hyper::client::conn::http1::handshake(io).await.unwrap();
        tokio::spawn(conn);
        let req = HyperRequest::builder()
            .method(method)
            .uri(path)
            .body(http_body_util::Empty::<Bytes>::new())
            .unwrap();
        sender.send_request(req).await.unwrap()
    }

    #[tokio::test]
    async fn with_cors_actually_adds_access_control_headers_over_real_tcp() {
        let app = Route::new()
            .at("/cors-ping", get(handler_fn(|_req, _p| async { hyper_compat::empty_status(SC::OK) })))
            .with_cors();
        let (addr, handle) = spawn(app).await;
        let resp = send(addr, Method::GET, "/cors-ping").await;
        assert_eq!(resp.status(), SC::OK);
        assert!(resp.headers().get("access-control-allow-origin").is_some());
        handle.abort();
    }

    #[tokio::test]
    async fn route_at_dispatches_by_method_like_poem() {
        let app = Route::new().at(
            "/ping",
            get(handler_fn(|_req, _p| async { hyper_compat::empty_status(SC::OK) }))
                .post(handler_fn(|_req, _p| async { hyper_compat::empty_status(SC::CREATED) })),
        );
        let (addr, handle) = spawn(app).await;

        let get_resp = send(addr, Method::GET, "/ping").await;
        assert_eq!(get_resp.status(), SC::OK);

        let post_resp = send(addr, Method::POST, "/ping").await;
        assert_eq!(post_resp.status(), SC::CREATED);
        handle.abort();
    }

    #[tokio::test]
    async fn path_params_extract_named_segments() {
        let app = Route::new().at(
            "/items/:id",
            get(handler_fn(|_req, params| async move {
                let p = PathParams::from(params);
                hyper_compat::html_response(SC::OK, p.get("id").unwrap_or("").to_string())
            })),
        );
        let (addr, handle) = spawn(app).await;

        let resp = send(addr, Method::GET, "/items/42").await;
        assert_eq!(resp.status(), SC::OK);
        let body = resp.into_body().collect().await.unwrap().to_bytes();
        assert_eq!(&body[..], b"42");
        handle.abort();
    }

    #[tokio::test]
    async fn json_response_matches_poem_web_json_shape() {
        #[derive(serde::Serialize)]
        struct Ping {
            ok: bool,
        }
        let resp = Json(Ping { ok: true }).into_response(SC::OK);
        assert_eq!(resp.status(), SC::OK);
        let body = resp.into_body().collect().await.unwrap().to_bytes();
        assert_eq!(&body[..], br#"{"ok":true}"#);
    }

    #[tokio::test]
    async fn unmatched_path_returns_404_like_poem() {
        let app = Route::new().at("/known", get(handler_fn(|_req, _p| async { hyper_compat::empty_status(SC::OK) })));
        let (addr, handle) = spawn(app).await;
        let resp = send(addr, Method::GET, "/unknown").await;
        assert_eq!(resp.status(), SC::NOT_FOUND);
        handle.abort();
    }

    #[tokio::test]
    async fn head_method_dispatches_separately_from_get_over_real_tcp() {
        let app = Route::new().at(
            "/head-ping",
            get(handler_fn(|_req, _p| async { hyper_compat::html_response(SC::OK, "body".to_string()) }))
                .head(handler_fn(|_req, _p| async { hyper_compat::empty_status(SC::OK) })),
        );
        let (addr, handle) = spawn(app).await;

        let head_resp = send(addr, Method::HEAD, "/head-ping").await;
        assert_eq!(head_resp.status(), SC::OK);
        let head_body = head_resp.into_body().collect().await.unwrap().to_bytes();
        assert!(head_body.is_empty());

        let get_resp = send(addr, Method::GET, "/head-ping").await;
        assert_eq!(get_resp.status(), SC::OK);
        let get_body = get_resp.into_body().collect().await.unwrap().to_bytes();
        assert_eq!(&get_body[..], b"body");
        handle.abort();
    }

    #[tokio::test]
    async fn server_run_actually_binds_and_serves_over_real_tcp() {
        let app = Route::new().at(
            "/healthz",
            get(handler_fn(|_req, _p| async { hyper_compat::empty_status(SC::OK) })),
        );
        let (addr, handle) = spawn(app).await;
        let resp = send(addr, Method::GET, "/healthz").await;
        assert_eq!(resp.status(), SC::OK);
        handle.abort();
    }

    #[tokio::test]
    async fn docs_mounts_openapi_swagger_and_graphql_over_real_tcp() {
        use openapi::{Operation, Spec};
        let spec = Spec::new("poem-compat demo", "1.0.0")
            .op(Operation::get("/ping").summary("ping").public().response_empty(200, "pong"));
        let app = Route::new()
            .at("/ping", get(handler_fn(|_req, _p| async { hyper_compat::empty_status(SC::OK) })))
            .docs("/api", spec);
        let (addr, handle) = spawn(app).await;
        for (path, needle) in [
            ("/api/openapi.json", "poem-compat demo"),
            ("/api/docs", "SwaggerUIBundle"),
            ("/api/redoc", "Redoc"),
            ("/api/openapi.graphql", "getPing"),
            ("/api/client.ts", "getPing"),
        ] {
            let resp = send(addr, Method::GET, path).await;
            assert_eq!(resp.status(), SC::OK, "{path}");
            let body = resp.into_body().collect().await.unwrap().to_bytes();
            assert!(String::from_utf8_lossy(&body).contains(needle), "{path}");
        }
        handle.abort();
    }

    #[tokio::test]
    async fn docs_auto_documents_every_registered_route_and_keeps_typed_ones() {
        use openapi::{Operation, Spec};
        let h = || handler_fn(|_req, _p| async { hyper_compat::empty_status(SC::OK) });
        // /typed は型付き宣言(summaryが保たれる)、/auto/:id は宣言なし(自動追加)。
        let spec = Spec::new("mix", "1").op(Operation::get("/typed").summary("typed one").public().response_empty(200, "ok"));
        let app = Route::new()
            .at("/typed", get(h()))
            .at("/auto/:id", get(h()).post(h()))
            .docs("/api", spec);
        let (addr, handle) = spawn(app).await;
        let resp = send(addr, Method::GET, "/api/openapi.json").await;
        let body = resp.into_body().collect().await.unwrap().to_bytes();
        let doc: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(doc["paths"]["/typed"]["get"]["summary"], "typed one");
        assert!(doc["paths"]["/auto/{id}"]["get"].is_object() && doc["paths"]["/auto/{id}"]["post"].is_object());
        assert_eq!(doc["paths"]["/auto/{id}"]["get"]["parameters"][0]["name"], "id");
        handle.abort();
    }
}
