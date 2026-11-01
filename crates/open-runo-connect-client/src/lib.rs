//! gRPC-web-Connect のブラウザ(WASM)クライアント。
//!
//! **ハイブリッド通信**:
//! - 単項は**Connect**(`application/proto`の素の本文)。エラーは**HTTPステータス**(401/404/…)として
//!   開発者ツールのNetworkタブにそのまま見え、`curl`で同じ要求を再現できる。
//! - サーバーストリーミングは**Connectのエンベロープ**を`ReadableStream`で読む。`AbortController`で
//!   中断でき、複数の呼び出し・購読を同時に走らせても互いに待たない。
//! - Connectを話さないサーバー(gRPC-webだけの古い版)だと分かったら、**自動でgRPC-webへ切り替える**。
//! - すべての呼び出しを記録する([`Client::log_entries`])。`debug`をONにすると、開発者ツールの
//!   Consoleにも折りたたみグループで出す。
//!
//! このクレートは`wasm32`専用(ホスト向けビルドでは空)。型付きの呼び出しは`prost` featureで。

#![cfg(target_arch = "wasm32")]

use futures::future::join_all;
use open_runo_connect_core as core;
use open_runo_connect_core::{code, Status};
use std::cell::{Cell, RefCell};
use std::rc::Rc;
use wasm_bindgen::{JsCast, JsValue};
use wasm_bindgen_futures::JsFuture;
use web_sys::{AbortController, AbortSignal, Request, RequestInit, RequestMode, Response};

pub use open_runo_connect_core as proto_core;

/// 使う通信方式。
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Wire {
    Connect,
    GrpcWeb,
}

impl Wire {
    pub fn name(self) -> &'static str {
        match self {
            Wire::Connect => "Connect",
            Wire::GrpcWeb => "gRPC-web",
        }
    }
}

/// 呼び出しの記録1件(インスペクタ用)。
#[derive(Clone, Debug)]
pub struct LogEntry {
    pub at_ms: f64,
    pub method: String,
    pub wire: &'static str,
    /// HTTPステータス(通信自体が失敗したときは0)。
    pub http_status: u16,
    pub grpc_code: i32,
    pub ms: f64,
    pub request: String,
    pub response: String,
}

const LOG_CAPACITY: usize = 200;
/// 「Connectを話さない」を表す内部用のコード(自動切替の合図)。
const NOT_CONNECT: i32 = -1;

struct Inner {
    base: String,
    service: String,
    headers: RefCell<Vec<(String, String)>>,
    wire: Cell<Wire>,
    log: RefCell<Vec<LogEntry>>,
    debug: Cell<bool>,
}

/// ハイブリッドクライアント。`clone`は安価(同じ設定・記録を共有する)。
#[derive(Clone)]
pub struct Client {
    inner: Rc<Inner>,
}

/// 購読の停止用ハンドル。
pub struct StreamHandle {
    abort: AbortController,
}

impl StreamHandle {
    /// 購読を止める(`fetch`を中断する)。
    pub fn stop(&self) {
        self.abort.abort();
    }
}

struct Fetched {
    status: u16,
    ok: bool,
    request_id: Option<String>,
    content_type: String,
    grpc_status: Option<i32>,
    grpc_message: Option<String>,
    bytes: Vec<u8>,
}

fn js_err(e: JsValue) -> Status {
    Status::new(code::UNKNOWN, format!("{e:?}"))
}

impl Client {
    /// `service`は完全修飾名(例: `open_runo.admin.v1.Admin`)。同一オリジンのパスに対して呼ぶ。
    pub fn new(service: &str) -> Self {
        Self::with_base(service, "")
    }

    /// 別オリジン(`http://host:port`)のサーバーを呼ぶ(CORSが必要)。
    pub fn with_base(service: &str, base: &str) -> Self {
        Self {
            inner: Rc::new(Inner {
                base: base.trim_end_matches('/').to_string(),
                service: service.to_string(),
                headers: RefCell::new(Vec::new()),
                wire: Cell::new(Wire::Connect),
                log: RefCell::new(Vec::new()),
                debug: Cell::new(false),
            }),
        }
    }

    /// すべての呼び出しに付けるヘッダ(`x-api-key`など)。同名は置き換える。
    pub fn set_header(&self, name: &str, value: &str) {
        let mut h = self.inner.headers.borrow_mut();
        h.retain(|(k, _)| !k.eq_ignore_ascii_case(name));
        h.push((name.to_string(), value.to_string()));
    }

    pub fn remove_header(&self, name: &str) {
        self.inner
            .headers
            .borrow_mut()
            .retain(|(k, _)| !k.eq_ignore_ascii_case(name));
    }

    pub fn wire(&self) -> Wire {
        self.inner.wire.get()
    }

    pub fn set_wire(&self, w: Wire) {
        self.inner.wire.set(w);
    }

    pub fn log_entries(&self) -> Vec<LogEntry> {
        self.inner.log.borrow().clone()
    }

    pub fn clear_log(&self) {
        self.inner.log.borrow_mut().clear();
    }

    /// ONにすると、各呼び出しを開発者ツールのConsoleにも出す。
    pub fn set_debug(&self, on: bool) {
        self.inner.debug.set(on);
    }

    pub fn is_debug(&self) -> bool {
        self.inner.debug.get()
    }

    fn path(&self, method: &str) -> String {
        format!("{}/{}/{}", self.inner.base, self.inner.service, method)
    }

    fn mode(&self) -> RequestMode {
        if self.inner.base.is_empty() {
            RequestMode::SameOrigin
        } else {
            RequestMode::Cors
        }
    }

    fn record(&self, entry: LogEntry) {
        if self.inner.debug.get() {
            let title = format!(
                "[{}] {} → HTTP {} / {} ({:.0} ms)",
                entry.wire,
                entry.method,
                entry.http_status,
                code::name(entry.grpc_code),
                entry.ms
            );
            web_sys::console::group_collapsed_1(&JsValue::from_str(&title));
            web_sys::console::log_1(&JsValue::from_str(&format!("request:  {}", entry.request)));
            web_sys::console::log_1(&JsValue::from_str(&format!("response: {}", entry.response)));
            web_sys::console::group_end();
        }
        let mut l = self.inner.log.borrow_mut();
        l.push(entry);
        if l.len() > LOG_CAPACITY {
            let drop = l.len() - LOG_CAPACITY;
            l.drain(..drop);
        }
    }

    async fn fetch(
        &self,
        path: &str,
        body: &[u8],
        content_type: &str,
        extra: &[(&str, &str)],
        signal: Option<&AbortSignal>,
    ) -> Result<web_sys::Response, Status> {
        let opts = RequestInit::new();
        opts.set_method("POST");
        opts.set_mode(self.mode());
        opts.set_body(&js_sys::Uint8Array::from(body));
        if let Some(s) = signal {
            opts.set_signal(Some(s));
        }
        let request = Request::new_with_str_and_init(path, &opts).map_err(js_err)?;
        let h = request.headers();
        h.set("content-type", content_type).map_err(js_err)?;
        for (k, v) in self.inner.headers.borrow().iter() {
            h.set(k, v).map_err(js_err)?;
        }
        for (k, v) in extra {
            h.set(k, v).map_err(js_err)?;
        }
        let window = web_sys::window().ok_or_else(|| Status::new(code::UNKNOWN, "no window"))?;
        JsFuture::from(window.fetch_with_request(&request))
            .await
            .map_err(|e| Status::new(code::UNAVAILABLE, format!("fetch error: {e:?}")))?
            .dyn_into()
            .map_err(js_err)
    }

    async fn fetch_all(
        &self,
        path: &str,
        body: &[u8],
        content_type: &str,
        extra: &[(&str, &str)],
    ) -> Result<Fetched, Status> {
        let resp = self.fetch(path, body, content_type, extra, None).await?;
        let buf = JsFuture::from(resp.array_buffer().map_err(js_err)?)
            .await
            .map_err(|e| Status::new(code::UNKNOWN, format!("body read error: {e:?}")))?;
        let h = resp.headers();
        let get = |n: &str| h.get(n).ok().flatten();
        Ok(Fetched {
            status: resp.status(),
            ok: resp.ok(),
            request_id: get("x-request-id"),
            content_type: get("content-type").unwrap_or_default(),
            grpc_status: get("grpc-status").and_then(|v| v.parse().ok()),
            grpc_message: get("grpc-message").map(|m| core::percent_decode(&m)),
            bytes: js_sys::Uint8Array::new(&buf).to_vec(),
        })
    }

    fn http_failure(f: &Fetched) -> Status {
        let suffix = f
            .request_id
            .as_ref()
            .map(|id| format!(" (request-id: {id})"))
            .unwrap_or_default();
        Status::new(
            code::from_http(f.status),
            if f.status == 429 {
                format!("HTTP 429 (rate limited){suffix}")
            } else {
                format!("HTTP {}{suffix}", f.status)
            },
        )
    }

    /// **Connect**の単項呼び出し(HTTPステータスがそのまま結果を表す)。
    async fn call_connect(&self, method: &str, req: &[u8]) -> Result<(Vec<u8>, u16), Status> {
        let f = self
            .fetch_all(
                &self.path(method),
                req,
                "application/proto",
                &[
                    ("connect-protocol-version", "1"),
                    ("accept", "application/proto"),
                ],
            )
            .await?;
        // gRPC-webだけ話すサーバーは、gRPC-web形式でエラーを返す → 自動切替の合図。
        if f.content_type.starts_with("application/grpc-web") {
            return Err(Status::new(NOT_CONNECT, ""));
        }
        if f.status == 200 {
            return Ok((f.bytes, f.status));
        }
        if f.status == 429 || f.content_type.is_empty() {
            return Err(Self::http_failure(&f));
        }
        // Connectのエラー: HTTPステータス + JSON `{code, message}`(gRPCのコードはヘッダにも載る)。
        let json: Option<core::serde_json::Value> = serde_json_from(&f.bytes);
        let message = f
            .grpc_message
            .clone()
            .or_else(|| {
                json.as_ref()
                    .and_then(|j| j["message"].as_str().map(str::to_string))
            })
            .unwrap_or_default();
        let suffix = f
            .request_id
            .as_ref()
            .map(|id| format!(" (request-id: {id})"))
            .unwrap_or_default();
        Err(Status::new(
            f.grpc_status.unwrap_or_else(|| code::from_http(f.status)),
            format!("{message}{suffix}"),
        ))
    }

    /// **gRPC-web**の単項呼び出し(フォールバック)。
    async fn call_grpc_web(&self, method: &str, req: &[u8]) -> Result<(Vec<u8>, u16), Status> {
        let f = self
            .fetch_all(
                &self.path(method),
                &core::encode_frame(req),
                "application/grpc-web+proto",
                &[
                    ("x-grpc-web", "1"),
                    ("accept", "application/grpc-web+proto"),
                ],
            )
            .await?;
        if !f.ok {
            return Err(Self::http_failure(&f));
        }
        let suffix = f
            .request_id
            .as_ref()
            .map(|id| format!(" (request-id: {id})"))
            .unwrap_or_default();
        let mut message = Vec::new();
        let mut st = (code::UNKNOWN, "missing grpc-web trailers".to_string());
        let mut rest: &[u8] = &f.bytes;
        while let Some((flag, body, tail)) = core::split_frame(rest) {
            if flag & 0x80 != 0 {
                st = core::parse_web_trailers(body);
            } else if message.is_empty() {
                message = body.to_vec();
            }
            rest = tail;
        }
        if st.0 != code::OK {
            return Err(Status::new(st.0, format!("{}{suffix}", st.1)));
        }
        Ok((message, f.status))
    }

    /// 単項呼び出し(protoバイト列)。既定はConnect、Connectを話さないと分かれば以後はgRPC-web。
    pub async fn unary(&self, method: &str, request: &[u8]) -> Result<Vec<u8>, Status> {
        let started = js_sys::Date::now();
        let mut wire = self.wire();
        let mut result = match wire {
            Wire::Connect => self.call_connect(method, request).await,
            Wire::GrpcWeb => self.call_grpc_web(method, request).await,
        };
        if matches!(&result, Err(e) if e.code == NOT_CONNECT) {
            self.set_wire(Wire::GrpcWeb);
            wire = Wire::GrpcWeb;
            result = self.call_grpc_web(method, request).await;
        }
        let (http_status, grpc_code, response) = match &result {
            Ok((b, st)) => (*st, code::OK, format!("{} bytes", b.len())),
            Err(e) => (0, e.code, e.message.clone()),
        };
        self.record(LogEntry {
            at_ms: started,
            method: method.to_string(),
            wire: wire.name(),
            http_status,
            grpc_code,
            ms: js_sys::Date::now() - started,
            request: format!("{} bytes", request.len()),
            response,
        });
        result.map(|(b, _)| b)
    }

    /// 複数の単項呼び出しを**同時に**走らせる(互いに待たない)。結果は入力と同じ順。
    pub async fn unary_many(&self, calls: &[(&str, Vec<u8>)]) -> Vec<Result<Vec<u8>, Status>> {
        join_all(calls.iter().map(|(m, b)| self.unary(m, b))).await
    }

    /// サーバーストリーミング。メッセージごとに`on_message`(protoバイト列)、終了時に`on_end`
    /// (正常なら`None`、失敗・中断以外の終了理由は`Some(Status)`)が呼ばれる。
    /// 戻り値の[`StreamHandle::stop`]で中断(中断は正常終了として`on_end(None)`)。
    pub fn server_stream(
        &self,
        method: &str,
        request: &[u8],
        on_message: impl Fn(Vec<u8>) + 'static,
        on_end: impl Fn(Option<Status>) + 'static,
    ) -> Result<StreamHandle, Status> {
        let abort = AbortController::new().map_err(js_err)?;
        let signal = abort.signal();
        let this = self.clone();
        let method = method.to_string();
        let request = request.to_vec();
        wasm_bindgen_futures::spawn_local(async move {
            let r = this
                .run_stream(&method, &request, &signal, &on_message)
                .await;
            on_end(r.err());
        });
        Ok(StreamHandle { abort })
    }

    async fn run_stream(
        &self,
        method: &str,
        request: &[u8],
        signal: &AbortSignal,
        on_message: &dyn Fn(Vec<u8>),
    ) -> Result<(), Status> {
        let started = js_sys::Date::now();
        let resp = self
            .fetch(
                &self.path(method),
                &core::encode_frame(request),
                "application/connect+proto",
                &[("connect-protocol-version", "1")],
                Some(signal),
            )
            .await?;
        if !resp.ok() {
            return Err(Status::new(
                code::from_http(resp.status()),
                format!("HTTP {}", resp.status()),
            ));
        }
        let body = resp
            .body()
            .ok_or_else(|| Status::new(code::UNKNOWN, "response has no body stream"))?;
        let reader: web_sys::ReadableStreamDefaultReader = body
            .get_reader()
            .dyn_into()
            .map_err(|e: js_sys::Object| js_err(e.into()))?;
        let mut buf: Vec<u8> = Vec::new();
        let mut count = 0usize;
        let result = loop {
            let chunk = match JsFuture::from(reader.read()).await {
                Ok(c) => c,
                // 中断(AbortController)は正常な停止として扱う。
                Err(_) if signal.aborted() => break Ok(()),
                Err(e) => break Err(js_err(e)),
            };
            let done = js_sys::Reflect::get(&chunk, &"done".into())
                .ok()
                .and_then(|d| d.as_bool())
                .unwrap_or(true);
            if done {
                break Ok(());
            }
            let value = match js_sys::Reflect::get(&chunk, &"value".into()) {
                Ok(v) => v,
                Err(e) => break Err(js_err(e)),
            };
            buf.extend_from_slice(&js_sys::Uint8Array::new(&value).to_vec());
            let mut end: Option<Result<(), Status>> = None;
            while let Some((flag, msg, tail)) = core::split_frame(&buf.clone()) {
                let consumed = buf.len() - tail.len();
                if flag == 0 {
                    count += 1;
                    on_message(msg.to_vec());
                } else if flag & 0x02 != 0 {
                    // 終端エンベロープ: `{}`なら正常、`{"error":{...}}`なら失敗。
                    end = Some(core::parse_connect_end(msg).map_or(Ok(()), Err));
                    break;
                }
                buf.drain(..consumed);
            }
            if let Some(r) = end {
                break r;
            }
        };
        self.record(LogEntry {
            at_ms: started,
            method: format!("{method} (stream)"),
            wire: Wire::Connect.name(),
            http_status: 200,
            grpc_code: result.as_ref().err().map_or(code::OK, |e| e.code),
            ms: js_sys::Date::now() - started,
            request: format!("{} bytes", request.len()),
            response: format!("{count} message(s)"),
        });
        result
    }

    /// 型付きの単項呼び出し(`prost` feature)。
    #[cfg(feature = "prost")]
    pub async fn call<Req, Resp>(&self, method: &str, req: &Req) -> Result<Resp, Status>
    where
        Req: prost::Message + std::fmt::Debug,
        Resp: prost::Message + Default + std::fmt::Debug,
    {
        let bytes = self.unary(method, &req.encode_to_vec()).await?;
        let resp = Resp::decode(&bytes[..])
            .map_err(|e| Status::new(code::INTERNAL, format!("decode error: {e}")))?;
        // 直近の記録に、型付きの中身を足す。
        if let Some(last) = self
            .inner
            .log
            .borrow_mut()
            .iter_mut()
            .rev()
            .find(|e| e.method == method)
        {
            last.request = format!("{req:?}");
            last.response = format!("{resp:?}");
        }
        Ok(resp)
    }
}

/// エラーJSONを(serde_jsonに依存せず軽く)読む。`core`が`serde_json`を持つので再利用する。
fn serde_json_from(bytes: &[u8]) -> Option<core::serde_json::Value> {
    core::parse_json(bytes)
}

#[allow(dead_code)]
fn _response_type_check(_: &Response) {}
