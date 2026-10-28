//! テスト用: `Route` を実サーバーとして一時ポートで起動し、HTTPで叩く。
//!
//! poemの`TestClient`相当。`hyper_compat::Request` は実TCPからしか作れないので、
//! 本物のサーバー+本物のHTTPクライアントでテストする(ミドルウェアやCORSも
//! 本番と同じ経路を通る)。
//!
//! ```ignore
//! let srv = TestServer::start(app).await;
//! let res = srv.post_json("/items", &serde_json::json!({"name": "a"})).await;
//! assert_eq!(res.status, 201);
//! let item: Item = res.json();
//! ```

use crate::{Method, Route, Server, TcpListener};
use bytes::Bytes;
use http_body_util::{BodyExt, Full};
use std::net::SocketAddr;

pub struct TestServer {
    pub addr: SocketAddr,
    handle: tokio::task::JoinHandle<()>,
}

pub struct TestResponse {
    pub status: u16,
    pub headers: hyper::HeaderMap,
    pub body: Bytes,
}

impl TestResponse {
    pub fn text(&self) -> String {
        String::from_utf8_lossy(&self.body).into_owned()
    }
    /// 本文をJSONとして読む。失敗時はpanic(テスト用)。
    pub fn json<T: serde::de::DeserializeOwned>(&self) -> T {
        serde_json::from_slice(&self.body)
            .unwrap_or_else(|e| panic!("response is not the expected JSON ({e}): {}", self.text()))
    }
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers.get(name).and_then(|v| v.to_str().ok())
    }
}

impl Drop for TestServer {
    fn drop(&mut self) {
        self.handle.abort();
    }
}

impl TestServer {
    /// `127.0.0.1:0`(空きポート)で起動する。
    pub async fn start(app: Route) -> Self {
        let (addr, handle) = Server::new(TcpListener::bind(([127, 0, 0, 1], 0)))
            .run(app)
            .await
            .expect("test server should bind an ephemeral port");
        Self { addr, handle }
    }

    pub fn url(&self, path: &str) -> String {
        format!("http://{}{}", self.addr, path)
    }

    /// 任意のメソッド・ヘッダ・本文でリクエストする。失敗時はpanic(テスト用)。
    pub async fn request(
        &self,
        method: Method,
        path: &str,
        headers: &[(&str, &str)],
        body: Option<Vec<u8>>,
    ) -> TestResponse {
        self.try_request(method, path, headers, body)
            .await
            .unwrap_or_else(|e| panic!("request failed: {e}"))
    }

    /// `request` の、接続エラーを `Err` で返す版(サーバーが巨大な本文を途中で拒否して
    /// 接続を閉じる場合など、エラーも正しい結果になる試験用)。
    pub async fn try_request(
        &self,
        method: Method,
        path: &str,
        headers: &[(&str, &str)],
        body: Option<Vec<u8>>,
    ) -> Result<TestResponse, String> {
        let stream = tokio::net::TcpStream::connect(self.addr)
            .await
            .map_err(|e| format!("connect: {e}"))?;
        let io = hyper_util::rt::TokioIo::new(stream);
        let (mut sender, conn) = hyper::client::conn::http1::handshake(io)
            .await
            .map_err(|e| format!("handshake: {e}"))?;
        tokio::spawn(conn);
        let mut b = hyper::Request::builder()
            .method(method)
            .uri(path)
            .header("host", self.addr.to_string());
        for (k, v) in headers {
            b = b.header(*k, *v);
        }
        let req = b
            .body(Full::new(Bytes::from(body.unwrap_or_default())))
            .expect("build request");
        let resp = sender
            .send_request(req)
            .await
            .map_err(|e| format!("send: {e}"))?;
        let (parts, incoming) = resp.into_parts();
        let body = incoming
            .collect()
            .await
            .map_err(|e| format!("body: {e}"))?
            .to_bytes();
        Ok(TestResponse {
            status: parts.status.as_u16(),
            headers: parts.headers,
            body,
        })
    }

    pub async fn get(&self, path: &str) -> TestResponse {
        self.request(Method::GET, path, &[], None).await
    }
    pub async fn delete(&self, path: &str) -> TestResponse {
        self.request(Method::DELETE, path, &[], None).await
    }
    pub async fn post_json<T: serde::Serialize>(&self, path: &str, body: &T) -> TestResponse {
        self.send_json(Method::POST, path, body).await
    }
    pub async fn put_json<T: serde::Serialize>(&self, path: &str, body: &T) -> TestResponse {
        self.send_json(Method::PUT, path, body).await
    }
    async fn send_json<T: serde::Serialize>(
        &self,
        method: Method,
        path: &str,
        body: &T,
    ) -> TestResponse {
        let bytes = serde_json::to_vec(body).expect("serialize request body");
        self.request(
            method,
            path,
            &[("content-type", "application/json")],
            Some(bytes),
        )
        .await
    }
}
