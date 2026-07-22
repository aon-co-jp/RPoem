//! `#[handler]`マクロ(`open-runo-poem-compat-macro`)がpoemと同じ
//! `get(handler_name)`という書き味を実際に成立させることを、実TCP経由で
//! 検証する統合テスト(単体テストではcrateの自己参照dev-dependencyが
//! 使えないため、この`tests/`統合テストの形にした)。

use open_runo_poem_compat::{empty_status_ok, get, handler_fn, Method, Params, Request, Response, Route, Server, StatusCode, TcpListener};
use std::net::SocketAddr;

#[open_runo_poem_compat_macro::handler]
async fn macro_ping(_req: Request, _params: Params) -> Response {
    empty_status_ok()
}

async fn spawn(app: Route) -> (SocketAddr, tokio::task::JoinHandle<()>) {
    Server::new(TcpListener::bind(([127, 0, 0, 1], 0))).run(app).await.expect("bind ephemeral port")
}

async fn send(addr: SocketAddr, method: Method, path: &str) -> hyper::Response<hyper::body::Incoming> {
    let stream = tokio::net::TcpStream::connect(addr).await.unwrap();
    let io = hyper_util::rt::TokioIo::new(stream);
    let (mut sender, conn) = hyper::client::conn::http1::handshake(io).await.unwrap();
    tokio::spawn(conn);
    let req = hyper::Request::builder()
        .method(method)
        .uri(path)
        .body(http_body_util::Empty::<bytes::Bytes>::new())
        .unwrap();
    sender.send_request(req).await.unwrap()
}

#[tokio::test]
async fn handler_macro_generates_poem_style_zero_arg_route_reference() {
    // poemの`#[handler] async fn hello() {...}` → `get(hello)`と同じ書き味:
    // handler_fn(...)で手で包まず、マクロが生成した`macro_ping()`を
    // 直接`get()`へ渡せることを確認する。
    let app = Route::new().at("/macro-ping", get(macro_ping()));
    let (addr, handle) = spawn(app).await;
    let resp = send(addr, Method::GET, "/macro-ping").await;
    assert_eq!(resp.status(), StatusCode::OK);
    handle.abort();
}

// マクロを使わない従来のhandler_fn経由でも変わらず動くことの回帰確認。
#[tokio::test]
async fn handler_fn_style_still_works_alongside_macro_style() {
    let app = Route::new().at(
        "/manual-ping",
        get(handler_fn(|_req: Request, _p: Params| async { empty_status_ok() })),
    );
    let (addr, handle) = spawn(app).await;
    let resp = send(addr, Method::GET, "/manual-ping").await;
    assert_eq!(resp.status(), StatusCode::OK);
    handle.abort();
}
