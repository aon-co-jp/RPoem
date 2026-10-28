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
        Self {
            status,
            message: message.into(),
            body: None,
        }
    }
    /// 本文を自由に指定する(既存のAPIが独自のエラー形 `{"ok":false,"error":..}` を持つとき用)。
    pub fn with_body(status: StatusCode, body: impl Serialize) -> Self {
        let v = serde_json::to_value(body).unwrap_or(serde_json::Value::Null);
        Self {
            status,
            message: v.to_string(),
            body: Some(v),
        }
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
pub(crate) type DataMap =
    std::collections::HashMap<std::any::TypeId, Arc<dyn std::any::Any + Send + Sync>>;

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
        self.data
            .as_ref()?
            .get(&std::any::TypeId::of::<T>())?
            .downcast_ref::<T>()
    }
    /// `data` の、無いときは500にする版(登録忘れは開発時に気付ける)。
    pub fn state<T: Send + Sync + 'static>(&self) -> ApiResult<&T> {
        self.data::<T>().ok_or_else(|| {
            ApiError::internal(format!(
                "app data not registered: {}",
                std::any::type_name::<T>()
            ))
        })
    }
    /// パスパラメータ(`/items/:id` の `id`)。パーセントエンコードは復号して返す
    /// (`a%20b` → `a b`)。
    pub fn param(&self, key: &str) -> Option<String> {
        self.params.get(key).map(percent_decode)
    }
    /// パスパラメータを型に変換する。無い/変換できないときは400。
    pub fn param_as<T: std::str::FromStr>(&self, key: &str) -> ApiResult<T> {
        self.param(key).and_then(|s| s.parse().ok()).ok_or_else(|| {
            ApiError::bad_request(format!("invalid or missing path parameter: {key}"))
        })
    }
    /// ヘッダの値(UTF-8として読める場合)。
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers.get(name).and_then(|v| v.to_str().ok())
    }
    /// `Authorization: Bearer <token>` のトークン。
    pub fn bearer_token(&self) -> Option<&str> {
        self.header("authorization").and_then(|v| {
            v.strip_prefix("Bearer ")
                .or_else(|| v.strip_prefix("bearer "))
        })
    }
    /// クエリ文字列を型 `Q` として取り出す。不正なら400。
    /// 同じキーの繰り返し(`?tag=a&tag=b`)は `Vec<T>` のフィールドで受けられる。
    pub fn query<Q: DeserializeOwned>(&self) -> ApiResult<Q> {
        serde_html_form::from_str(self.uri.query().unwrap_or(""))
            .map_err(|e| ApiError::bad_request(format!("invalid query: {e}")))
    }
    /// `data` の、所有権つき(`Arc<T>`)の版。`tokio::spawn` やストリームの中へ
    /// 持ち出せる(`&T` はハンドラの外へ持ち出せない)。無ければ500。
    pub fn shared<T: Send + Sync + 'static>(&self) -> ApiResult<Arc<T>> {
        let missing = || {
            ApiError::internal(format!(
                "app data not registered: {}",
                std::any::type_name::<T>()
            ))
        };
        let any = self
            .data
            .as_ref()
            .and_then(|m| m.get(&std::any::TypeId::of::<T>()))
            .ok_or_else(missing)?;
        Arc::clone(any).downcast::<T>().map_err(|_| missing())
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
        None => hyper_compat::json_response(
            e.status,
            &ApiErrorBody {
                error: e.message.clone(),
            },
        ),
    }
}

fn finish<Res: Serialize>(r: ApiResult<Res>, ok_status: StatusCode) -> Response {
    match r {
        Ok(v) => hyper_compat::json_response(ok_status, &v),
        Err(e) => error_response(&e),
    }
}

fn with_errors(op: Operation, with_input: bool) -> Operation {
    let op = if with_input {
        op.response::<ApiErrorBody>(400, "入力不正")
    } else {
        op
    };
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
    let op = with_errors(
        op.body::<Req>()
            .response_empty(status.as_u16(), "成功(本文なし)"),
        true,
    );
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

// ───────────────────────── multipart ─────────────────────────

pub use open_runo_openapi::FileField;

/// 受け取ったファイル1つ(`multipart/form-data` の `filename=` が付いたパート)。
#[derive(Debug, Clone)]
pub struct UploadedFile {
    /// フォームのフィールド名。
    pub field: String,
    pub filename: Option<String>,
    pub content_type: Option<String>,
    pub data: Vec<u8>,
}

/// `multipart/form-data` を型で受けた結果。テキストフィールドは `form`(構造体 `T`)、
/// ファイルは `files`。
#[derive(Debug)]
pub struct Multipart<T> {
    pub form: T,
    pub files: Vec<UploadedFile>,
}

impl<T> Multipart<T> {
    /// 指定フィールドの最初のファイル。
    pub fn file(&self, field: &str) -> Option<&UploadedFile> {
        self.files.iter().find(|f| f.field == field)
    }
    /// 指定フィールドの全ファイル(`FileField::multiple()` 用)。
    pub fn files_named<'a>(&'a self, field: &'a str) -> impl Iterator<Item = &'a UploadedFile> {
        self.files.iter().filter(move |f| f.field == field)
    }
}

/// テキストフィールドが無い `multipart` 用の空の型(`api::multipart::<NoFields, _, _, _>`)。
#[derive(serde::Deserialize, JsonSchema)]
pub struct NoFields {}

/// `Route::max_body` を明示したときだけの値(multipartの既定は別に大きい)。
fn explicit_body_limit(ctx: &Ctx) -> Option<usize> {
    ctx.data::<BodyLimit>().map(|l| l.0)
}

async fn read_multipart<T: DeserializeOwned>(
    req: Request,
    files: &[FileField],
    explicit_limit: Option<usize>,
) -> Result<Multipart<T>, Response> {
    let mut limits = crate::multipart_stream::MultipartLimits::default();
    if let Some(n) = explicit_limit {
        limits.max_total_bytes = n as u64;
        limits.max_field_bytes = n as u64;
    }
    let mut mp = crate::multipart_stream::MultipartStream::from_request(req, limits)?;
    let mut text: Vec<(String, String)> = Vec::new();
    let mut uploaded = Vec::new();
    while let Some(field) = mp.next_field().await.map_err(|e| e.into_response())? {
        let data = mp
            .field_bytes(limits.max_field_bytes as usize)
            .await
            .map_err(|e| e.into_response())?;
        if field.filename.is_some() {
            uploaded.push(UploadedFile {
                field: field.name,
                filename: field.filename,
                content_type: field.content_type,
                data,
            });
        } else {
            text.push((field.name, String::from_utf8_lossy(&data).into_owned()));
        }
    }
    for f in files.iter().filter(|f| f.required) {
        if !uploaded.iter().any(|u| u.field == f.name) {
            return Err(hyper_compat::json_response(
                StatusCode::BAD_REQUEST,
                &ApiErrorBody {
                    error: format!("missing file field: {}", f.name),
                },
            ));
        }
    }
    let encoded = serde_urlencoded::to_string(&text).unwrap_or_default();
    let form = serde_html_form::from_str::<T>(&encoded).map_err(|e| {
        hyper_compat::json_response(
            StatusCode::BAD_REQUEST,
            &ApiErrorBody {
                error: format!("invalid form fields: {e}"),
            },
        )
    })?;
    Ok(Multipart {
        form,
        files: uploaded,
    })
}

/// `multipart/form-data`(ファイルのアップロード)を受け、`Res` をJSONで返す。
/// テキストフィールドは構造体 `Form`、ファイルは `files` の宣言で文書に載る。
/// 必須のファイルが無ければ400、上限超過は413(既定: 全体64MiB・1ファイル32MiB、
/// `Route::max_body` を指定すればそれが上限)。
pub fn multipart<Form, Res, F, Fut>(op: Operation, files: &[FileField], f: F) -> ApiHandler
where
    Form: DeserializeOwned + JsonSchema + Send + 'static,
    Res: Serialize + JsonSchema + 'static,
    F: Fn(Multipart<Form>, Ctx) -> Fut + Send + Sync + 'static,
    Fut: Future<Output = ApiResult<Res>> + Send + 'static,
{
    let op = with_errors(op.body_multipart::<Form>(files).ok::<Res>("OK"), true)
        .response::<ApiErrorBody>(413, "本文が大きすぎる");
    let files: Arc<Vec<FileField>> = Arc::new(files.to_vec());
    let f = Arc::new(f);
    let handler: Handler = Arc::new(move |req: Request, params: Params| {
        let f = Arc::clone(&f);
        let files = Arc::clone(&files);
        Box::pin(async move {
            let ctx = Ctx::from_request(&req, params);
            let form = match read_multipart::<Form>(req, &files, explicit_body_limit(&ctx)).await {
                Ok(m) => m,
                Err(resp) => return resp,
            };
            finish(f(form, ctx).await, StatusCode::OK)
        })
    });
    ApiHandler { op, handler }
}

// ───────────────── multipart(逐次処理版) ─────────────────

pub use crate::multipart_stream::FieldInfo;

impl From<crate::multipart_stream::MultipartError> for ApiError {
    fn from(e: crate::multipart_stream::MultipartError) -> Self {
        ApiError::new(e.status(), e.to_string())
    }
}

/// `multipart/form-data` を到着順に1パートずつ読む入れ物(`api::upload` のハンドラが受け取る)。
/// ファイル全体をメモリへ集めない: `chunk()` で届いたぶんだけ受け、ディスクや下流へ流せる
/// (使うメモリは1チャンク程度でファイルの大きさに依存しない)。
pub struct UploadStream {
    inner: crate::multipart_stream::MultipartStream<
        http_body_util::BodyDataStream<hyper::body::Incoming>,
    >,
}

impl UploadStream {
    /// 次のパートへ進む(読み残しは読み捨てる)。無ければ `None`。
    /// フォームのフィールドは送られた順に来るので、ファイルより前に必要な値は
    /// クライアントが先に送る約束にする。
    pub async fn next_field(&mut self) -> ApiResult<Option<FieldInfo>> {
        Ok(self.inner.next_field().await?)
    }
    /// 現在のパートの次のチャンク。終わりで `None`。
    pub async fn chunk(&mut self) -> ApiResult<Option<bytes::Bytes>> {
        Ok(self.inner.chunk().await?)
    }
    /// 現在のパートを文字列として読む(テキストフィールド向け、`limit` バイトまで)。
    pub async fn text(&mut self, limit: usize) -> ApiResult<String> {
        Ok(String::from_utf8_lossy(&self.inner.field_bytes(limit).await?).into_owned())
    }
    /// 現在のパートをまとめてメモリへ読む(`limit` バイトまで。大きなファイルには `chunk` を使う)。
    pub async fn bytes(&mut self, limit: usize) -> ApiResult<Vec<u8>> {
        Ok(self.inner.field_bytes(limit).await?)
    }
}

/// 大きなファイルのアップロード用: `multipart/form-data` を逐次処理で受け、`Res` をJSONで返す。
/// 文書は `api::multipart` と同じ(`Form` はテキストフィールドの宣言用で、値の取り出しは
/// ハンドラ側で `UploadStream::text` を使う)。上限の既定は全体・1パートとも1GiB
/// (`Route::max_body` で変更)、超過は413。
pub fn upload<Form, Res, F, Fut>(op: Operation, files: &[FileField], f: F) -> ApiHandler
where
    Form: JsonSchema + 'static,
    Res: Serialize + JsonSchema + 'static,
    F: Fn(UploadStream, Ctx) -> Fut + Send + Sync + 'static,
    Fut: Future<Output = ApiResult<Res>> + Send + 'static,
{
    let op = with_errors(op.body_multipart::<Form>(files).ok::<Res>("OK"), true)
        .response::<ApiErrorBody>(413, "本文が大きすぎる");
    let f = Arc::new(f);
    let handler: Handler = Arc::new(move |req: Request, params: Params| {
        let f = Arc::clone(&f);
        Box::pin(async move {
            let ctx = Ctx::from_request(&req, params);
            let n = explicit_body_limit(&ctx).unwrap_or(1 << 30) as u64;
            let limits = crate::multipart_stream::MultipartLimits {
                max_total_bytes: n,
                max_field_bytes: n,
                ..Default::default()
            };
            let inner = match crate::multipart_stream::MultipartStream::from_request(req, limits) {
                Ok(m) => m,
                Err(resp) => return resp,
            };
            finish(f(UploadStream { inner }, ctx).await, StatusCode::OK)
        })
    });
    ApiHandler { op, handler }
}

// ───────────────────────── SSE ─────────────────────────

/// SSEの既定のkeep-alive間隔(無通信でプロキシに切られないよう `: keep-alive` を送る)。
pub const SSE_KEEP_ALIVE: std::time::Duration = std::time::Duration::from_secs(15);

/// イベント名つきのSSEイベント(`event: <名前>` + `data: <JSON>`)。`api::sse_named` 用。
#[derive(Debug, Clone)]
pub struct Named<T> {
    pub event: &'static str,
    pub data: T,
}

impl<T> Named<T> {
    pub fn new(event: &'static str, data: T) -> Self {
        Self { event, data }
    }
}

fn sse_handler<T, S, F, Fut>(
    f: F,
    keep_alive: Option<std::time::Duration>,
    to_event: fn(T) -> Option<hyper_compat::SseEvent>,
) -> Handler
where
    T: Send + 'static,
    S: futures::Stream<Item = T> + Send + Unpin + 'static,
    F: Fn(Ctx) -> Fut + Send + Sync + 'static,
    Fut: Future<Output = ApiResult<S>> + Send + 'static,
{
    let f = Arc::new(f);
    Arc::new(move |req: Request, params: Params| {
        let f = Arc::clone(&f);
        Box::pin(async move {
            let ctx = Ctx::from_request(&req, params);
            let mut stream = match f(ctx).await {
                Ok(s) => s,
                Err(e) => return error_response(&e),
            };
            // `sse_response` は `Sync` なストリームを要る。どんなストリームでも渡せるよう、
            // チャネルを1段挟む(受け側が閉じたら=切断されたら送信側のタスクも終わる)。
            let (mut tx, rx) = futures::channel::mpsc::channel::<hyper_compat::SseEvent>(16);
            tokio::spawn(async move {
                use futures::{SinkExt, StreamExt};
                while let Some(item) = stream.next().await {
                    let Some(ev) = to_event(item) else { continue };
                    if tx.send(ev).await.is_err() {
                        break;
                    }
                }
            });
            match keep_alive {
                Some(every) => hyper_compat::sse_response_keepalive(rx, every),
                None => hyper_compat::sse_response(rx),
            }
        })
    })
}

/// Server-Sent Events。`f` が返すストリームの各要素 `Ev` を、1イベント1JSONとして流す。
/// 文書には `text/event-stream` として `Ev` の型が載り、TypeScriptクライアントは
/// `EventSource` で受けるコールバック、GraphQLでは `Subscription` になる。
/// 15秒ごとに keep-alive を送る(変えるなら `sse_opts`)。
/// クライアントが切断するとストリームは次のイベントの送信時に止まる。
pub fn sse<Ev, S, F, Fut>(op: Operation, f: F) -> ApiHandler
where
    Ev: Serialize + JsonSchema + Send + 'static,
    S: futures::Stream<Item = Ev> + Send + Unpin + 'static,
    F: Fn(Ctx) -> Fut + Send + Sync + 'static,
    Fut: Future<Output = ApiResult<S>> + Send + 'static,
{
    sse_opts(op, Some(SSE_KEEP_ALIVE), f)
}

/// `sse` の、keep-alive間隔を指定する版(`None` で送らない)。
pub fn sse_opts<Ev, S, F, Fut>(
    op: Operation,
    keep_alive: Option<std::time::Duration>,
    f: F,
) -> ApiHandler
where
    Ev: Serialize + JsonSchema + Send + 'static,
    S: futures::Stream<Item = Ev> + Send + Unpin + 'static,
    F: Fn(Ctx) -> Fut + Send + Sync + 'static,
    Fut: Future<Output = ApiResult<S>> + Send + 'static,
{
    let op = with_errors(
        op.response_sse::<Ev>("イベントストリーム(1イベント=1JSON)"),
        false,
    );
    let handler = sse_handler(f, keep_alive, |ev: Ev| {
        serde_json::to_string(&ev)
            .ok()
            .map(|data| hyper_compat::SseEvent {
                event_type: None,
                data,
            })
    });
    ApiHandler { op, handler }
}

/// イベント名つきのSSE。ストリームの要素は `Named::new("tick", data)`。`names` は文書に載せる
/// 取りうるイベント名(TypeScriptクライアントはそれぞれに `addEventListener` する)。
/// 名前が `names` に無いイベントは送られず捨てられる(文書と実際がずれない)。
pub fn sse_named<Ev, S, F, Fut>(
    op: Operation,
    names: &'static [&'static str],
    keep_alive: Option<std::time::Duration>,
    f: F,
) -> ApiHandler
where
    Ev: Serialize + JsonSchema + Send + 'static,
    S: futures::Stream<Item = Named<Ev>> + Send + Unpin + 'static,
    F: Fn(Ctx) -> Fut + Send + Sync + 'static,
    Fut: Future<Output = ApiResult<S>> + Send + 'static,
{
    let op = with_errors(
        op.response_sse::<Ev>("イベントストリーム(イベント名つき、1イベント=1JSON)")
            .sse_events(names),
        false,
    );
    let allowed = names;
    // 関数ポインタに状態を持たせられないため、許可名の検査はストリームの手前で行う。
    let f = Arc::new(f);
    let handler = sse_handler(
        move |ctx: Ctx| {
            let f = Arc::clone(&f);
            async move {
                use futures::StreamExt;
                let s = f(ctx).await?;
                Ok(Box::pin(s.filter(move |n| {
                    futures::future::ready(allowed.contains(&n.event))
                })))
            }
        },
        keep_alive,
        |n: Named<Ev>| {
            serde_json::to_string(&n.data)
                .ok()
                .map(|data| hyper_compat::SseEvent {
                    event_type: Some(n.event),
                    data,
                })
        },
    );
    ApiHandler { op, handler }
}

// ───────────────────────── WebSocket ─────────────────────────

/// 型付きのWebSocket接続。クライアントからは `In`、サーバーからは `Out` をJSONテキストで送る。
pub struct WsSession<In, Out> {
    conn: hyper_compat::WebSocketConnection,
    _types: std::marker::PhantomData<fn() -> (In, Out)>,
}

/// `WsSession::recv` が、JSONとして読めないメッセージを受けたときの情報。
#[derive(Debug, Clone)]
pub struct WsDecodeError(pub String);

impl std::fmt::Display for WsDecodeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}
impl std::error::Error for WsDecodeError {}

/// `WsSession::recv_any` が返す、受信メッセージの種類。
#[derive(Debug)]
pub enum WsIncoming<In> {
    /// 型に合うJSONテキスト。
    Message(In),
    /// バイナリメッセージ。
    Binary(Vec<u8>),
    /// 型に合わないテキスト(接続は続く)。
    Invalid(WsDecodeError),
}

impl<In: DeserializeOwned, Out: Serialize> WsSession<In, Out> {
    /// 次のメッセージを `In` として読む。接続が閉じたら `None`。
    /// 型に合わないメッセージは `Some(Err(..))`(接続は閉じない。続けて読める)。
    pub async fn recv(&mut self) -> Option<Result<In, WsDecodeError>> {
        match self.conn.recv().await? {
            hyper_compat::WsMessage::Text(t) => Some(
                serde_json::from_str(&t)
                    .map_err(|e| WsDecodeError(format!("invalid message: {e}"))),
            ),
            hyper_compat::WsMessage::Binary(_) => Some(Err(WsDecodeError(
                "binary messages are not supported".into(),
            ))),
        }
    }
    /// `recv` の、バイナリメッセージも `WsIncoming::Binary` で受ける版(文書には
    /// `ApiHandler::binary` で宣言する)。接続が閉じたら `None`。
    pub async fn recv_any(&mut self) -> Option<WsIncoming<In>> {
        Some(match self.conn.recv().await? {
            hyper_compat::WsMessage::Text(t) => match serde_json::from_str(&t) {
                Ok(v) => WsIncoming::Message(v),
                Err(e) => WsIncoming::Invalid(WsDecodeError(format!("invalid message: {e}"))),
            },
            hyper_compat::WsMessage::Binary(b) => WsIncoming::Binary(b),
        })
    }
    /// バイナリメッセージを送る(画像・音声のチャンクなど)。
    pub async fn send_binary(&mut self, data: &[u8]) -> std::io::Result<()> {
        self.conn.send_binary(data).await
    }
    /// `Out` をJSONテキストで送る。
    pub async fn send(&mut self, msg: &Out) -> std::io::Result<()> {
        let text = serde_json::to_string(msg).map_err(std::io::Error::other)?;
        self.conn.send_text(text).await
    }
    pub async fn close(&mut self) -> std::io::Result<()> {
        self.conn.close().await
    }
    /// 型を付けない生の接続(バイナリなど)。
    pub fn raw(&mut self) -> &mut hyper_compat::WebSocketConnection {
        &mut self.conn
    }
}

/// WebSocket。クライアントが `In`、サーバーが `Out` のJSONメッセージを送る。
/// 文書にはメッセージの型(`x-websocket`)が載り、TypeScriptクライアントは
/// `send(In)` と `onMessage(Out)` を持つ接続になる。メソッドはGET。
/// ハンドシェイクが不正なら400。
pub fn websocket<In, Out, F, Fut>(op: Operation, f: F) -> ApiHandler
where
    In: DeserializeOwned + JsonSchema + Send + 'static,
    Out: Serialize + JsonSchema + Send + 'static,
    F: Fn(WsSession<In, Out>, Ctx) -> Fut + Send + Sync + 'static,
    Fut: Future<Output = ()> + Send + 'static,
{
    assert!(
        op.http_method() == open_runo_openapi::Method::Get,
        "api::websocket needs a GET operation"
    );
    let op = with_errors(
        op.websocket::<In, Out>("WebSocketへアップグレード(JSONテキストのメッセージ)"),
        true,
    );
    let f = Arc::new(f);
    let handler: Handler = Arc::new(move |req: Request, params: Params| {
        let f = Arc::clone(&f);
        Box::pin(async move {
            let ctx = Ctx::from_request(&req, params);
            hyper_compat::websocket_upgrade(req, move |conn| {
                Box::pin(f(
                    WsSession {
                        conn,
                        _types: std::marker::PhantomData,
                    },
                    ctx,
                ))
            })
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
    /// WebSocketがバイナリメッセージも使うことを文書に載せる(`client`=クライアントが送るバイナリ、
    /// `server`=サーバーが送るバイナリの意味。使わない向きは `None`)。TSクライアントに
    /// `sendBinary` / `onBinary` が付く。受信は `WsSession::recv_any`、送信は `send_binary`。
    pub fn binary(mut self, client: Option<&str>, server: Option<&str>) -> Self {
        self.op = self.op.websocket_binary(client, server);
        self
    }
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
            if let (Some(h), Some(l)) = (
                (b[i + 1] as char).to_digit(16),
                (b[i + 2] as char).to_digit(16),
            ) {
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
        hyper_compat::json_response(
            StatusCode::BAD_REQUEST,
            &ApiErrorBody {
                error: format!("invalid JSON body: {e}"),
            },
        )
    })
}
