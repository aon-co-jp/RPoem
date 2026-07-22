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

use bytes::Bytes;
use open_runo_router::hyper_compat::{self, Params};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;

pub use hyper::Method;
pub use hyper::StatusCode;

pub type Request = hyper_compat::Request;
pub type Response = hyper_compat::Response;
pub type Handler = hyper_compat::Handler;
pub type BoxFuture<T> = hyper_compat::BoxFuture<T>;

/// poemの`Route::new().at(path, get(handler).post(handler2))`と同じ
/// 書き味を提供する、`hyper_compat::Router`のビルダーラッパー。
#[derive(Default)]
pub struct Route {
    entries: Vec<(String, MethodRouter)>,
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
        self.entries.push((path.to_string(), method_router));
        self
    }

    pub fn build(self) -> hyper_compat::Router {
        let mut router = hyper_compat::Router::new();
        for (path, mr) in self.entries {
            for (method, handler) in mr.handlers {
                router = router.route(method, &path, handler);
            }
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
}
