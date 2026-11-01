//! connect-rust のブラウザ(WASM)クライアントで、RPoem の `open_runo.admin.v1.Admin` を
//! Connect プロトコルで呼ぶ実機確認用プローブ。`transport.rs` は connect-rust の
//! `examples/wasm-client` のものをそのまま使う(fetch ベース、本文は一括読み込み)。

mod transport;

mod proto {
    ::connectrpc::include_generated!();
}

use ::connectrpc::client::ClientConfig;
use proto::open_runo::admin::v1::*;
use wasm_bindgen::prelude::*;

fn cfg(base_url: &str, api_key: Option<&str>, json: bool) -> Result<ClientConfig, JsError> {
    let mut c = ClientConfig::new(base_url.parse().map_err(JsError::from)?);
    if json {
        c = c.json();
    }
    if let Some(k) = api_key {
        c = c.with_default_header("x-api-key", k);
    }
    Ok(c)
}

/// 鍵の自己発行(認証不要)。`json`でJSONコーデックに切り替える。
#[wasm_bindgen]
pub async fn self_issue(base_url: &str, json: bool) -> Result<String, JsError> {
    let client = AdminClient::new(transport::FetchTransport, cfg(base_url, None, json)?);
    let r = client
        .self_issue_key(Empty::default())
        .await
        .map_err(|e| JsError::new(&e.to_string()))?;
    Ok(r.into_owned().api_key)
}

/// 鍵つきで、スキーマ登録→履歴→DB状態(Struct)→未登録スキーマ(エラー)を順に呼ぶ。
#[wasm_bindgen]
pub async fn admin_flow(base_url: &str, api_key: &str, json: bool) -> Result<String, JsError> {
    let client = AdminClient::new(transport::FetchTransport, cfg(base_url, Some(api_key), json)?);
    let mut log = Vec::new();

    let v = client
        .register_schema(RegisterSchemaRequest {
            service_name: "probe".into(),
            sdl: "type Query { a: Int }".into(),
            stage: Some("staging".into()),
            ..Default::default()
        })
        .await
        .map_err(|e| JsError::new(&e.to_string()))?
        .into_owned();
    log.push(format!("RegisterSchema -> {}@{} ns={}", v.service_name, v.stage, v.namespace));

    let h = client
        .schema_history(SchemaHistoryRequest {
            service_name: "probe".into(),
            ..Default::default()
        })
        .await
        .map_err(|e| JsError::new(&e.to_string()))?
        .into_owned();
    log.push(format!("SchemaHistory -> {} version(s)", h.versions.len()));

    let s = client
        .db_status(Empty::default())
        .await
        .map_err(|e| JsError::new(&e.to_string()))?
        .into_owned();
    log.push(format!("DbStatus (Struct) -> {} fields", s.fields.len()));

    match client
        .get_schema(GetSchemaRequest {
            service_name: "nope".into(),
            ..Default::default()
        })
        .await
    {
        Ok(_) => log.push("GetSchema(nope) -> unexpectedly OK".into()),
        Err(e) => log.push(format!("GetSchema(nope) -> error: {e}")),
    }
    Ok(log.join("\n"))
}
