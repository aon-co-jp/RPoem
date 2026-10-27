//! 型駆動のAPIハンドラ(poem-openapiの`#[oai]`相当)。
//!
//! 1つの宣言で **ルーティング・入力の取り出し・出力のJSON化・エラー応答・
//! APIドキュメント(OpenAPI/Swagger/SDL/TSクライアント)** を同時に得る:
//!
//! ```ignore
//! use open_runo_poem_compat::{api, openapi::Operation as Op, ApiError, Route};
//!
//! let app = Route::new()
//!     .api(api::json(Op::post("/items").summary("作成"), |body: NewItem, _ctx| async move {
//!         if body.name.is_empty() { return Err(ApiError::bad_request("name is required")); }
//!         Ok(Item { id: 1, name: body.name })
//!     }))
//!     .api(api::plain(Op::get("/items/:id").summary("取得"), |ctx| async move {
//!         let id: u64 = ctx.param("id").and_then(|s| s.parse().ok()).ok_or_else(|| ApiError::bad_request("id"))?;
//!         Ok(Item { id, name: "x".into() })
//!     }))
//!     .docs("/api", Spec::new("demo", "1.0"));
//! ```
//!
//! ドキュメントは、ハンドラの引数・戻り値の型から作られる(型を変えれば文書も変わる)ので、
//! 手書きの宣言とハンドラがずれることが構造上起きない。

use crate::{hyper_compat, Handler, Method, Params, Request, Response, StatusCode};
use open_runo_openapi::Operation;
use schemars::JsonSchema;
use serde::{de::DeserializeOwned, Serialize};
use std::future::Future;
use std::sync::Arc;

/// APIハンドラが返すエラー。`{"error": "..."}` のJSONとして、指定のステータスで返る。
#[derive(Debug, Clone)]
pub struct ApiError {
    pub status: StatusCode,
    pub message: String,
    /// 指定すると `{"error": message}` の代わりにこのJSONを本文にする。
    pub body: Option<serde_json::Value>,
}

impl ApiError {
    pub fn new(status: StatusCode, message: impl Into<String>) -> Self {
        Self { status, message: message.into(), body: None }
    }
    /// 本文を自由に指定する(既存のAPIが独自のエラー形 `{"ok":false,"error":..}` を持つとき用)。
    pub fn with_body(status: StatusCode, body: impl Serialize) -> Self {
        let v = serde_json::to_value(body).unwrap_or(serde_json::Value::Null);
        Self { status, message: v.to_string(), body: Some(v) }
    }
    pub fn bad_request(message: impl Into<String>) -> Self {
        Self::new(StatusCode::BAD_REQUEST, message)
    }
    pub fn unauthorized(message: impl Into<String>) -> Self {
        Self::new(StatusCode::UNAUTHORIZED, message)
    }
    pub fn forbidden(message: impl Into<String>) -> Self {
        Self::new(StatusCode::FORBIDDEN, message)
    }
    pub fn not_found(message: impl Into<String>) -> Self {
        Self::new(StatusCode::NOT_FOUND, message)
    }
    pub fn conflict(message: impl Into<String>) -> Self {
        Self::new(StatusCode::CONFLICT, message)
    }
    pub fn internal(message: impl Into<String>) -> Self {
        Self::new(StatusCode::INTERNAL_SERVER_ERROR, message)
    }
}

impl std::fmt::Display for ApiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.status, self.message)
    }
}

impl std::error::Error for ApiError {}

/// `?` で `std::io::Error` 等を500にできる。
impl From<std::io::Error> for ApiError {
    fn from(e: std::io::Error) -> Self {
        Self::internal(e.to_string())
    }
}

pub type ApiResult<T> = Result<T, ApiError>;

/// エラー応答の本文(文書上の型)。
#[derive(Serialize, JsonSchema)]
pub struct ApiErrorBody {
    pub error: String,
}

/// リクエストの文脈(パスパラメータ・ヘッダ・URI)。本文は含まない。
pub struct Ctx {
    pub params: Params,
    pub headers: hyper::HeaderMap,
    pub uri: hyper::Uri,
    pub method: Method,
    data: Option<Arc<DataMap>>,
}

/// `Route::data(..)` で登録したアプリ共有の値(DBプール・設定など)を型で引く入れ物。
pub(crate) type DataMap = std::collections::HashMap<std::any::TypeId, Arc<dyn std::any::Any + Send + Sync>>;

/// リクエストの拡張に載せて、ハンドラまで運ぶ。
#[derive(Clone)]
pub(crate) struct DataExt(pub(crate) Arc<DataMap>);

impl Ctx {
    fn from_request(req: &Request, params: Params) -> Self {
        Self {
            params,
            headers: req.headers().clone(),
            uri: req.uri().clone(),
            method: req.method().clone(),
            data: req.extensions().get::<DataExt>().map(|d| Arc::clone(&d.0)),
        }
    }
    /// `Route::data(v)` で登録した値を型で取り出す(poemの`Data<&T>`相当)。
    pub fn data<T: Send + Sync + 'static>(&self) -> Option<&T> {
        self.data.as_ref()?.get(&std::any::TypeId::of::<T>())?.downcast_ref::<T>()
    }
    /// `data` の、無いときは500にする版(登録忘れは開発時に気付ける)。
    pub fn state<T: Send + Sync + 'static>(&self) -> ApiResult<&T> {
        self.data::<T>().ok_or_else(|| ApiError::internal(format!("app data not registered: {}", std::any::type_name::<T>())))
    }
    /// パスパラメータ(`/items/:id` の `id`)。パーセントエンコードは復号して返す
    /// (`a%20b` → `a b`)。
    pub fn param(&self, key: &str) -> Option<String> {
        self.params.get(key).map(percent_decode)
    }
    /// パスパラメータを型に変換する。無い/変換できないときは400。
    pub fn param_as<T: std::str::FromStr>(&self, key: &str) -> ApiResult<T> {
        self.param(key)
            .and_then(|s| s.parse().ok())
            .ok_or_else(|| ApiError::bad_request(format!("invalid or missing path parameter: {key}")))
    }
    /// ヘッダの値(UTF-8として読める場合)。
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers.get(name).and_then(|v| v.to_str().ok())
    }
    /// `Authorization: Bearer <token>` のトークン。
    pub fn bearer_token(&self) -> Option<&str> {
        self.header("authorization").and_then(|v| v.strip_prefix("Bearer ").or_else(|| v.strip_prefix("bearer ")))
    }
    /// クエリ文字列を型 `Q` として取り出す。不正なら400。
    pub fn query<Q: DeserializeOwned>(&self) -> ApiResult<Q> {
        serde_urlencoded::from_str(self.uri.query().unwrap_or(""))
            .map_err(|e| ApiError::bad_request(format!("invalid query: {e}")))
    }
}

/// 操作の宣言とハンドラの組。`Route::api` に渡す。
pub struct ApiHandler {
    pub(crate) op: Operation,
    pub(crate) handler: Handler,
}

fn error_response(e: &ApiError) -> Response {
    match &e.body {
        Some(b) => hyper_compat::json_response(e.status, b),
        None => hyper_compat::json_response(e.status, &ApiErrorBody { error: e.message.clone() }),
    }
}

fn finish<Res: Serialize>(r: ApiResult<Res>, ok_status: StatusCode) -> Response {
    match r {
        Ok(v) => hyper_compat::json_response(ok_status, &v),
        Err(e) => error_response(&e),
    }
}

fn with_errors(op: Operation, with_input: bool) -> Operation {
    let op = if with_input { op.response::<ApiErrorBody>(400, "入力不正") } else { op };
    op.response::<ApiErrorBody>(500, "内部エラー")
}

/// JSON本文 `Req` を受け、`Res` をJSONで返す。本文が不正なら400。
/// 文書: リクエスト本文=`Req`、200=`Res`、400/500=エラー本文。
pub fn json<Req, Res, F, Fut>(op: Operation, f: F) -> ApiHandler
where
    Req: DeserializeOwned + JsonSchema + Send + 'static,
    Res: Serialize + JsonSchema + 'static,
    F: Fn(Req, Ctx) -> Fut + Send + Sync + 'static,
    Fut: Future<Output = ApiResult<Res>> + Send + 'static,
{
    let op = with_errors(op.body::<Req>().ok::<Res>("OK"), true);
    let f = Arc::new(f);
    let handler: Handler = Arc::new(move |req: Request, params: Params| {
        let f = Arc::clone(&f);
        Box::pin(async move {
            let ctx = Ctx::from_request(&req, params);
            let body = match read_json_limited::<Req>(req, ctx.body_limit()).await {
                Ok(b) => b,
                Err(resp) => return resp,
            };
            finish(f(body, ctx).await, StatusCode::OK)
        })
    });
    ApiHandler { op, handler }
}

/// クエリ文字列 `Q` を受け、`Res` をJSONで返す。クエリが不正なら400。
/// 文書: クエリパラメータ=`Q`の各フィールド(必須/任意も反映)。
pub fn query<Q, Res, F, Fut>(op: Operation, f: F) -> ApiHandler
where
    Q: DeserializeOwned + JsonSchema + Send + 'static,
    Res: Serialize + JsonSchema + 'static,
    F: Fn(Q, Ctx) -> Fut + Send + Sync + 'static,
    Fut: Future<Output = ApiResult<Res>> + Send + 'static,
{
    let op = with_errors(op.query::<Q>().ok::<Res>("OK"), true);
    let f = Arc::new(f);
    let handler: Handler = Arc::new(move |req: Request, params: Params| {
        let f = Arc::clone(&f);
        Box::pin(async move {
            let ctx = Ctx::from_request(&req, params);
            let q = match ctx.query::<Q>() {
                Ok(q) => q,
                Err(e) => return error_response(&e),
            };
            finish(f(q, ctx).await, StatusCode::OK)
        })
    });
    ApiHandler { op, handler }
}

/// 入力なし(パスパラメータ・ヘッダのみ)で `Res` をJSONで返す。
pub fn plain<Res, F, Fut>(op: Operation, f: F) -> ApiHandler
where
    Res: Serialize + JsonSchema + 'static,
    F: Fn(Ctx) -> Fut + Send + Sync + 'static,
    Fut: Future<Output = ApiResult<Res>> + Send + 'static,
{
    let op = with_errors(op.ok::<Res>("OK"), false);
    let f = Arc::new(f);
    let handler: Handler = Arc::new(move |req: Request, params: Params| {
        let f = Arc::clone(&f);
        Box::pin(async move {
            let ctx = Ctx::from_request(&req, params);
            finish(f(ctx).await, StatusCode::OK)
        })
    });
    ApiHandler { op, handler }
}

/// 入力なしで、成功時に本文なしの `status`(既定は204等)を返す。
pub fn no_content<F, Fut>(op: Operation, status: StatusCode, f: F) -> ApiHandler
where
    F: Fn(Ctx) -> Fut + Send + Sync + 'static,
    Fut: Future<Output = ApiResult<()>> + Send + 'static,
{
    let op = with_errors(op.response_empty(status.as_u16(), "成功(本文なし)"), false);
    let f = Arc::new(f);
    let handler: Handler = Arc::new(move |req: Request, params: Params| {
        let f = Arc::clone(&f);
        Box::pin(async move {
            let ctx = Ctx::from_request(&req, params);
            match f(ctx).await {
                Ok(()) => hyper_compat::empty_status(status),
                Err(e) => error_response(&e),
            }
        })
    });
    ApiHandler { op, handler }
}

/// JSON本文 `Req` を受け、成功時に本文なしの `status` を返す(DELETE/更新通知など)。
pub fn json_no_content<Req, F, Fut>(op: Operation, status: StatusCode, f: F) -> ApiHandler
where
    Req: DeserializeOwned + JsonSchema + Send + 'static,
    F: Fn(Req, Ctx) -> Fut + Send + Sync + 'static,
    Fut: Future<Output = ApiResult<()>> + Send + 'static,
{
    let op = with_errors(op.body::<Req>().response_empty(status.as_u16(), "成功(本文なし)"), true);
    let f = Arc::new(f);
    let handler: Handler = Arc::new(move |req: Request, params: Params| {
        let f = Arc::clone(&f);
        Box::pin(async move {
            let ctx = Ctx::from_request(&req, params);
            let body = match read_json_limited::<Req>(req, ctx.body_limit()).await {
                Ok(b) => b,
                Err(resp) => return resp,
            };
            match f(body, ctx).await {
                Ok(()) => hyper_compat::empty_status(status),
                Err(e) => error_response(&e),
            }
        })
    });
    ApiHandler { op, handler }
}

/// 作成系: 成功時に `201 Created` で `Res` を返す(`json` の201版)。
pub fn json_created<Req, Res, F, Fut>(op: Operation, f: F) -> ApiHandler
where
    Req: DeserializeOwned + JsonSchema + Send + 'static,
    Res: Serialize + JsonSchema + 'static,
    F: Fn(Req, Ctx) -> Fut + Send + Sync + 'static,
    Fut: Future<Output = ApiResult<Res>> + Send + 'static,
{
    let op = with_errors(op.body::<Req>().response::<Res>(201, "作成した"), true);
    let f = Arc::new(f);
    let handler: Handler = Arc::new(move |req: Request, params: Params| {
        let f = Arc::clone(&f);
        Box::pin(async move {
            let ctx = Ctx::from_request(&req, params);
            let body = match read_json_limited::<Req>(req, ctx.body_limit()).await {
                Ok(b) => b,
                Err(resp) => return resp,
            };
            finish(f(body, ctx).await, StatusCode::CREATED)
        })
    });
    ApiHandler { op, handler }
}

pub(crate) fn to_hyper_method(m: open_runo_openapi::Method) -> Method {
    match m {
        open_runo_openapi::Method::Get => Method::GET,
        open_runo_openapi::Method::Post => Method::POST,
        open_runo_openapi::Method::Put => Method::PUT,
        open_runo_openapi::Method::Patch => Method::PATCH,
        open_runo_openapi::Method::Delete => Method::DELETE,
    }
}

impl ApiHandler {
    /// このハンドラが返しうるエラーを文書に載せる(本文は `{"error": ..}`)。
    /// 例: `.error(404, "見つからない").error(409, "重複")`。
    pub fn error(mut self, status: u16, description: &str) -> Self {
        self.op = self.op.response::<ApiErrorBody>(status, description);
        self
    }
    /// `error` の、本文の型を指定する版(独自のエラー形を返すとき)。
    pub fn error_as<T: JsonSchema>(mut self, status: u16, description: &str) -> Self {
        self.op = self.op.response::<T>(status, description);
        self
    }
    /// 操作の宣言をその場で調整する(説明文・タグ・認証の要否など)。
    pub fn with_op(mut self, f: impl FnOnce(Operation) -> Operation) -> Self {
        self.op = f(self.op);
        self
    }
}

/// `%XX` を復号する(不正な並びはそのまま残す。UTF-8として不正なバイトは置換文字)。
fn percent_decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' && i + 2 < b.len() {
            if let (Some(h), Some(l)) = ((b[i + 1] as char).to_digit(16), (b[i + 2] as char).to_digit(16)) {
                out.push((h * 16 + l) as u8);
                i += 3;
                continue;
            }
        }
        out.push(b[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// 型駆動ハンドラが読むJSON本文の上限(バイト)。`Route::max_body` で変更する。
#[derive(Clone, Copy)]
pub(crate) struct BodyLimit(pub(crate) usize);

pub(crate) const DEFAULT_MAX_BODY: usize = 2 * 1024 * 1024;

impl Ctx {
    pub(crate) fn body_limit(&self) -> usize {
        self.data::<BodyLimit>().map_or(DEFAULT_MAX_BODY, |l| l.0)
    }
}

/// 上限つきでJSON本文を読む。超過は413、不正なJSONは400(1行のメッセージ)。
async fn read_json_limited<T: DeserializeOwned>(req: Request, limit: usize) -> Result<T, Response> {
    let bytes = crate::body_limit::read_body_limited(req, limit).await?;
    serde_json::from_slice::<T>(&bytes).map_err(|e| {
        hyper_compat::json_response(StatusCode::BAD_REQUEST, &ApiErrorBody { error: format!("invalid JSON body: {e}") })
    })
}
