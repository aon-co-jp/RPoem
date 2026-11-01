//! 管理操作のGraphQL窓口(REST廃止計画 第1段階、CLAUDE.md「2026-11-01追記」)。
//!
//! `query { admin { ... } }` / `mutation { admin { ... } }`の配下に、REST
//! `/api/schemas*`・`/api/federation/*`・`/api/feature-flags*`・
//! `/api/persisted-queries*`・`/api/cache/purge*`と同じ操作を置く。ロジックは
//! `open_runo_router::admin_ops`をRESTと共有する(二重実装しない)。
//!
//! 認証: HTTP層(`graphql_hyper`)が`X-Api-Key`またはセッションCookie
//! (+CSRF)を`auth_hyper::authenticate_with_session`で検査し、成功すると
//! [`Actor`]をGraphQLコンテキストへ渡す。`admin`フィールドは`Actor`が
//! 無ければ失敗する(RESTの401相当、`extensions.status`=401)。RESTと同じく
//! ロール別の絞り込みは現状無く、認証済みであれば実行できる。

use async_graphql::types::Json;
use async_graphql::{Context, ErrorExtensions, InputObject, Object, SimpleObject};
use open_runo_api_types::{
    FeatureFlagEvaluationResponse, FeatureFlagRequest, FeatureFlagResponse, RegisterSchemaRequest,
    SchemaVersion,
};
use open_runo_router::admin_ops::{self, AdminError, ComposeService};
use open_runo_router::admin_sys::{self, RouteCandidate};
use open_runo_router::auth_hyper::Actor;
use open_runo_router::keyring::KeyGuardian;
use open_runo_router::state::AppState;
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

/// GraphQLエラーへ変換する。`extensions.status`にREST版と同じHTTPステータスを載せる。
fn to_gql_error(e: AdminError) -> async_graphql::Error {
    let status = i32::from(e.status.as_u16());
    async_graphql::Error::new(e.message).extend_with(|_, ext| ext.set("status", status))
}

/// 認証済みの呼び出し元(REST版の`actor`文字列と同じ形式)を取り出す。
fn actor_label(ctx: &Context<'_>) -> async_graphql::Result<String> {
    match ctx.data_opt::<Actor>() {
        Some(a) => Ok(format!(
            "{}{}",
            if a.via_session { "session:" } else { "" },
            a.owner
        )),
        None => Err(async_graphql::Error::new(
            "authentication required (X-Api-Key or session cookie)",
        )
        .extend_with(|_, ext| ext.set("status", 401))),
    }
}

/// HTTP層から渡す、認証・セッション系ミューテーション用の入力と出力。
/// `api_key`は`X-Api-Key`ヘッダ、`session_id`はセッションCookie由来。`set_cookie`に
/// 入れた値をHTTP層が`Set-Cookie`として返す(GraphQLにはヘッダを書く手段が無いため)。
#[derive(Debug, Clone, Default)]
pub struct HttpCtx {
    pub api_key: String,
    pub session_id: Option<String>,
    pub set_cookie: Arc<Mutex<Option<String>>>,
}

fn js<T: serde::Serialize>(v: &T) -> Json<serde_json::Value> {
    Json(serde_json::to_value(v).unwrap_or(serde_json::Value::Null))
}

/// 認証済みであることだけを確認する(Subscriptionなど`admin`の外から使う)。
pub fn require_actor(ctx: &Context<'_>) -> async_graphql::Result<()> {
    actor_label(ctx).map(|_| ())
}

fn state<'a>(ctx: &'a Context<'_>) -> &'a Arc<AppState> {
    ctx.data_unchecked::<Arc<AppState>>()
}

#[derive(Debug, SimpleObject)]
pub struct SchemaVersionAdmin {
    id: String,
    namespace: String,
    service_name: String,
    sdl: String,
    stage: String,
    created_at: String,
}

impl From<SchemaVersion> for SchemaVersionAdmin {
    fn from(v: SchemaVersion) -> Self {
        Self {
            id: v.id,
            namespace: v.namespace,
            service_name: v.service_name,
            sdl: v.sdl,
            stage: v.stage,
            created_at: v.created_at,
        }
    }
}

#[derive(Debug, SimpleObject)]
pub struct FederationSummary {
    contributing_services: Vec<String>,
    type_count: i32,
    field_count: i32,
}

#[derive(Debug, SimpleObject)]
pub struct TypeFields {
    type_name: String,
    fields: Vec<String>,
}

#[derive(Debug, SimpleObject)]
pub struct ComposeResult {
    contributing_services: Vec<String>,
    types: Vec<TypeFields>,
    breaking_changes: Vec<String>,
}

#[derive(Debug, InputObject)]
pub struct TypeFieldsInput {
    type_name: String,
    fields: Vec<String>,
}

#[derive(Debug, InputObject)]
pub struct ComposeServiceInput {
    service_name: String,
    /// 型名 → フィールド名の事前構造化マップ(`sdl`があれば不要)。
    types: Option<Vec<TypeFieldsInput>>,
    sdl: Option<String>,
}

#[derive(Debug, SimpleObject)]
pub struct FeatureFlagAdmin {
    name: String,
    enabled: bool,
    rollout_percent: i32,
    description: String,
}

impl From<FeatureFlagResponse> for FeatureFlagAdmin {
    fn from(f: FeatureFlagResponse) -> Self {
        Self {
            name: f.name,
            enabled: f.enabled,
            rollout_percent: i32::from(f.rollout_percent),
            description: f.description,
        }
    }
}

#[derive(Debug, SimpleObject)]
pub struct FlagEvaluation {
    name: String,
    bucket_key: String,
    enabled: bool,
}

impl From<FeatureFlagEvaluationResponse> for FlagEvaluation {
    fn from(f: FeatureFlagEvaluationResponse) -> Self {
        Self {
            name: f.name,
            bucket_key: f.bucket_key,
            enabled: f.enabled,
        }
    }
}

#[derive(Debug, SimpleObject)]
pub struct PersistedQueryAdmin {
    hash: String,
    query: String,
    registered_at: String,
}

impl From<admin_ops::PersistedQueryRecord> for PersistedQueryAdmin {
    fn from(r: admin_ops::PersistedQueryRecord) -> Self {
        Self {
            hash: r.hash,
            query: r.query,
            registered_at: r.registered_at,
        }
    }
}

#[derive(Debug, InputObject)]
pub struct RouteCandidateInput {
    provider: String,
    estimated_cost_usd_per_1k_tokens: f64,
    estimated_latency_ms: u32,
    is_local: bool,
    context_length: u32,
}

#[derive(Debug, SimpleObject)]
pub struct PurgeResult {
    purged: String,
}

/// `Query.admin`の入口。
#[derive(Debug, Default)]
pub struct AdminQuery;

/// `Query.admin`の配下(認証済みの読み取り)。
#[derive(Debug)]
pub struct AdminQueryFields;

#[Object]
impl AdminQuery {
    /// 管理操作(読み取り)。`X-Api-Key`またはセッションが必要。
    async fn admin(&self, ctx: &Context<'_>) -> async_graphql::Result<AdminQueryFields> {
        actor_label(ctx)?;
        Ok(AdminQueryFields)
    }
}

#[Object]
impl AdminQueryFields {
    /// `GET /api/schemas/:service`
    async fn schema(
        &self,
        ctx: &Context<'_>,
        service_name: String,
        stage: Option<String>,
        namespace: Option<String>,
    ) -> async_graphql::Result<SchemaVersionAdmin> {
        admin_ops::get_schema(
            state(ctx),
            namespace.as_deref(),
            &service_name,
            stage.as_deref(),
        )
        .map(Into::into)
        .map_err(to_gql_error)
    }

    /// `GET /api/schemas/:service/history`
    async fn schema_history(
        &self,
        ctx: &Context<'_>,
        service_name: String,
        namespace: Option<String>,
    ) -> Vec<SchemaVersionAdmin> {
        admin_ops::schema_history(state(ctx), namespace.as_deref(), &service_name)
            .into_iter()
            .map(Into::into)
            .collect()
    }

    /// `GET /api/federation/status`
    async fn federation_status(&self, ctx: &Context<'_>) -> FederationSummary {
        let st = admin_ops::federation_status(state(ctx));
        FederationSummary {
            contributing_services: st.contributing_services,
            type_count: i32::try_from(st.type_count).unwrap_or(i32::MAX),
            field_count: i32::try_from(st.field_count).unwrap_or(i32::MAX),
        }
    }

    /// `GET /api/feature-flags`
    async fn feature_flags(&self, ctx: &Context<'_>) -> Vec<FeatureFlagAdmin> {
        admin_ops::flag_list(state(ctx))
            .into_iter()
            .map(Into::into)
            .collect()
    }

    /// `GET /api/feature-flags/:name`
    async fn feature_flag(
        &self,
        ctx: &Context<'_>,
        name: String,
    ) -> async_graphql::Result<FeatureFlagAdmin> {
        admin_ops::flag_get(state(ctx), &name)
            .map(Into::into)
            .map_err(to_gql_error)
    }

    /// `GET /api/feature-flags/:name/evaluate`
    async fn evaluate_feature_flag(
        &self,
        ctx: &Context<'_>,
        name: String,
        bucket_key: Option<String>,
    ) -> async_graphql::Result<FlagEvaluation> {
        admin_ops::flag_evaluate(state(ctx), &name, &bucket_key.unwrap_or_default())
            .map(Into::into)
            .map_err(to_gql_error)
    }

    /// `GET /api/db/status`
    async fn db_status(&self, ctx: &Context<'_>) -> Json<serde_json::Value> {
        js(&admin_sys::db_status(state(ctx)))
    }

    /// `GET /api/db/routing`
    async fn db_routing(&self) -> Json<serde_json::Value> {
        js(&admin_sys::db_routing())
    }

    /// `GET /api/db/:table`
    async fn db_list(
        &self,
        ctx: &Context<'_>,
        table: String,
    ) -> async_graphql::Result<Json<serde_json::Value>> {
        admin_sys::db_list(state(ctx), &table)
            .await
            .map(|r| js(&r))
            .map_err(to_gql_error)
    }

    /// `GET /api/db/:table/:key`
    async fn db_get(
        &self,
        ctx: &Context<'_>,
        table: String,
        key: String,
    ) -> async_graphql::Result<Json<serde_json::Value>> {
        admin_sys::db_get(state(ctx), &table, &key)
            .await
            .map(|r| js(&r))
            .map_err(to_gql_error)
    }

    /// `GET /api/db/:table/:key/at/:commit_id`
    async fn db_get_at_commit(
        &self,
        ctx: &Context<'_>,
        table: String,
        key: String,
        commit_id: String,
    ) -> async_graphql::Result<Json<serde_json::Value>> {
        admin_sys::db_get_at_commit(state(ctx), &table, &key, &commit_id)
            .await
            .map(|r| js(&r))
            .map_err(to_gql_error)
    }

    /// `GET /api/analytics/requests-per-month`
    async fn analytics_requests_per_month(&self, ctx: &Context<'_>) -> Json<serde_json::Value> {
        Json(admin_sys::requests_per_month(state(ctx)))
    }

    /// `GET /api/analytics/operations`
    async fn analytics_operations(&self, ctx: &Context<'_>) -> Json<serde_json::Value> {
        Json(admin_sys::operations_summary(state(ctx)))
    }

    /// `GET /api/cache/ai-stats`
    async fn cache_ai_stats(&self, ctx: &Context<'_>) -> Json<serde_json::Value> {
        js(&admin_sys::ai_stats(&state(ctx).page_cache))
    }

    /// `POST /api/ai/route`(副作用なしの選択なのでQueryに置く)
    async fn ai_route(
        &self,
        policy: String,
        min_context_length: Option<u32>,
        candidates: Vec<RouteCandidateInput>,
    ) -> async_graphql::Result<Json<serde_json::Value>> {
        let candidates: Vec<RouteCandidate> = candidates
            .into_iter()
            .map(|c| RouteCandidate {
                provider: c.provider,
                estimated_cost_usd_per_1k_tokens: c.estimated_cost_usd_per_1k_tokens,
                estimated_latency_ms: c.estimated_latency_ms,
                is_local: c.is_local,
                context_length: c.context_length,
            })
            .collect();
        admin_sys::ai_route(&policy, min_context_length, &candidates)
            .map(|r| js(&r))
            .map_err(to_gql_error)
    }

    /// `GET /scim/v2/Users`
    async fn scim_users(
        &self,
        ctx: &Context<'_>,
        filter: Option<String>,
    ) -> async_graphql::Result<Json<serde_json::Value>> {
        admin_sys::scim_list_users(state(ctx), filter.as_deref())
            .await
            .map(|r| js(&r))
            .map_err(to_gql_error)
    }

    /// `GET /api/persisted-queries/:hash`
    async fn persisted_query(
        &self,
        ctx: &Context<'_>,
        hash: String,
    ) -> async_graphql::Result<PersistedQueryAdmin> {
        admin_ops::persisted_query_get(state(ctx), &hash)
            .await
            .map(Into::into)
            .map_err(to_gql_error)
    }
}

/// `Mutation.admin`の入口。
#[derive(Debug, Default)]
pub struct AdminMutation;

/// `Mutation.admin`の配下(認証済みの書き込み)。
#[derive(Debug)]
pub struct AdminMutationFields {
    actor: String,
}

#[Object]
impl AdminMutation {
    /// `POST /api/keys/self-issue`(認証不要。発行した鍵自体が資格情報)。
    async fn self_issue_key(
        &self,
        ctx: &Context<'_>,
    ) -> async_graphql::Result<Json<serde_json::Value>> {
        let guardian = ctx.data_unchecked::<Arc<KeyGuardian>>();
        admin_sys::self_issue_key(state(ctx), guardian)
            .await
            .map(|r| js(&r))
            .map_err(to_gql_error)
    }

    /// `POST /api/session/login`。`X-Api-Key`ヘッダをセッションCookie+CSRFへ交換する
    /// (Cookieは応答の`Set-Cookie`で返る)。
    async fn session_login(
        &self,
        ctx: &Context<'_>,
    ) -> async_graphql::Result<Json<serde_json::Value>> {
        let http = ctx.data_opt::<HttpCtx>().cloned().unwrap_or_default();
        let guardian = ctx.data_unchecked::<Arc<KeyGuardian>>();
        let (login, cookie) =
            admin_sys::session_login(guardian, &state(ctx).sessions, &http.api_key)
                .await
                .map_err(to_gql_error)?;
        *http
            .set_cookie
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(cookie);
        Ok(js(&login))
    }

    /// `POST /api/session/logout`(冪等)。
    async fn session_logout(&self, ctx: &Context<'_>) -> bool {
        let http = ctx.data_opt::<HttpCtx>().cloned().unwrap_or_default();
        let cookie =
            admin_sys::session_logout(&state(ctx).sessions, http.session_id.as_deref()).await;
        *http
            .set_cookie
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(cookie);
        true
    }

    /// 管理操作(書き込み)。`X-Api-Key`またはセッション(+CSRF)が必要。
    async fn admin(&self, ctx: &Context<'_>) -> async_graphql::Result<AdminMutationFields> {
        Ok(AdminMutationFields {
            actor: actor_label(ctx)?,
        })
    }
}

impl AdminMutationFields {
    async fn register(
        &self,
        ctx: &Context<'_>,
        service_name: String,
        sdl: String,
        stage: Option<String>,
        namespace: Option<String>,
    ) -> SchemaVersionAdmin {
        admin_ops::register_schema(
            state(ctx),
            &self.actor,
            RegisterSchemaRequest {
                service_name,
                sdl,
                stage: stage.unwrap_or_else(|| "local".to_string()),
                namespace,
            },
        )
        .await
        .into()
    }
}

#[Object]
impl AdminMutationFields {
    /// `POST /api/schemas`
    async fn register_schema(
        &self,
        ctx: &Context<'_>,
        service_name: String,
        sdl: String,
        stage: Option<String>,
        namespace: Option<String>,
    ) -> SchemaVersionAdmin {
        self.register(ctx, service_name, sdl, stage, namespace)
            .await
    }

    /// `POST /api/schemas/upload`(SDLファイルの内容をテキストで渡す。
    /// RESTのmultipart版の`sdl_file`相当)。
    async fn upload_schema(
        &self,
        ctx: &Context<'_>,
        service_name: String,
        sdl_file: String,
        stage: Option<String>,
        namespace: Option<String>,
    ) -> SchemaVersionAdmin {
        self.register(ctx, service_name, sdl_file, stage, namespace)
            .await
    }

    /// `POST /api/federation/compose`
    async fn compose_federation(
        &self,
        ctx: &Context<'_>,
        services: Vec<ComposeServiceInput>,
    ) -> async_graphql::Result<ComposeResult> {
        let services = services
            .into_iter()
            .map(|s| ComposeService {
                service_name: s.service_name,
                types: s
                    .types
                    .unwrap_or_default()
                    .into_iter()
                    .map(|t| (t.type_name, t.fields))
                    .collect::<BTreeMap<_, _>>(),
                sdl: s.sdl,
            })
            .collect();
        let out = admin_ops::compose(state(ctx), services).map_err(to_gql_error)?;
        Ok(ComposeResult {
            contributing_services: out.contributing_services,
            types: out
                .types
                .into_iter()
                .map(|(type_name, fields)| TypeFields { type_name, fields })
                .collect(),
            breaking_changes: out.breaking_changes,
        })
    }

    /// `POST /api/feature-flags`(作成または更新)
    async fn upsert_feature_flag(
        &self,
        ctx: &Context<'_>,
        name: String,
        enabled: Option<bool>,
        rollout_percent: Option<i32>,
        description: Option<String>,
    ) -> async_graphql::Result<FeatureFlagAdmin> {
        let rollout = u8::try_from(rollout_percent.unwrap_or(100)).map_err(|_| {
            to_gql_error(AdminError::new(
                hyper::StatusCode::UNPROCESSABLE_ENTITY,
                "rolloutPercent must be between 0 and 100",
            ))
        })?;
        admin_ops::flag_upsert(
            state(ctx),
            &self.actor,
            FeatureFlagRequest {
                name,
                enabled: enabled.unwrap_or(true),
                rollout_percent: rollout,
                description: description.unwrap_or_default(),
            },
        )
        .await
        .map(Into::into)
        .map_err(to_gql_error)
    }

    /// `DELETE /api/feature-flags/:name`
    async fn delete_feature_flag(
        &self,
        ctx: &Context<'_>,
        name: String,
    ) -> async_graphql::Result<bool> {
        admin_ops::flag_delete(state(ctx), &self.actor, &name)
            .await
            .map(|()| true)
            .map_err(to_gql_error)
    }

    /// `POST /api/persisted-queries`
    async fn register_persisted_query(
        &self,
        ctx: &Context<'_>,
        query: String,
    ) -> async_graphql::Result<PersistedQueryAdmin> {
        admin_ops::persisted_query_register(state(ctx), &self.actor, &query)
            .await
            .map(Into::into)
            .map_err(to_gql_error)
    }

    /// `PUT /api/db/:table/:key`
    async fn db_put(
        &self,
        ctx: &Context<'_>,
        table: String,
        key: String,
        value: Json<serde_json::Value>,
    ) -> async_graphql::Result<Json<serde_json::Value>> {
        admin_sys::db_put(state(ctx), &self.actor, &table, &key, value.0)
            .await
            .map(|r| js(&r))
            .map_err(to_gql_error)
    }

    /// `DELETE /api/db/:table/:key`
    async fn db_delete(
        &self,
        ctx: &Context<'_>,
        table: String,
        key: String,
    ) -> async_graphql::Result<Json<serde_json::Value>> {
        admin_sys::db_delete(state(ctx), &self.actor, &table, &key)
            .await
            .map(|r| js(&r))
            .map_err(to_gql_error)
    }

    /// `POST /api/backup/export`
    async fn backup_export(
        &self,
        ctx: &Context<'_>,
    ) -> async_graphql::Result<Json<serde_json::Value>> {
        let st = state(ctx);
        admin_sys::backup_export(st, &st.page_cache, &self.actor)
            .await
            .map(|r| js(&r))
            .map_err(to_gql_error)
    }

    /// `POST /api/backup/import`
    async fn backup_import(
        &self,
        ctx: &Context<'_>,
        path: String,
    ) -> async_graphql::Result<Json<serde_json::Value>> {
        admin_sys::backup_import(state(ctx), &self.actor, &path)
            .await
            .map(|r| js(&r))
            .map_err(to_gql_error)
    }

    /// `POST /api/backup/restore-latest`
    async fn backup_restore_latest(
        &self,
        ctx: &Context<'_>,
    ) -> async_graphql::Result<Json<serde_json::Value>> {
        admin_sys::backup_restore_latest(state(ctx), &self.actor)
            .await
            .map(|r| js(&r))
            .map_err(to_gql_error)
    }

    /// `POST /api/migrate/export-sql`(`dialect`は postgres / mysql / generic)
    async fn migrate_export_sql(
        &self,
        ctx: &Context<'_>,
        dialect: String,
    ) -> async_graphql::Result<Json<serde_json::Value>> {
        let dialect = admin_sys::parse_dialect(&dialect).map_err(to_gql_error)?;
        admin_sys::migrate_export_sql(state(ctx), &self.actor, dialect)
            .await
            .map(|r| js(&r))
            .map_err(to_gql_error)
    }

    /// `POST /api/migrate/export-csv`
    async fn migrate_export_csv(
        &self,
        ctx: &Context<'_>,
    ) -> async_graphql::Result<Json<serde_json::Value>> {
        admin_sys::migrate_export_csv(state(ctx), &self.actor)
            .await
            .map(|r| js(&r))
            .map_err(to_gql_error)
    }

    /// `POST /api/integrity/check`
    async fn integrity_check(
        &self,
        ctx: &Context<'_>,
    ) -> async_graphql::Result<Json<serde_json::Value>> {
        admin_sys::integrity_check(state(ctx), &self.actor)
            .await
            .map(|r| js(&r))
            .map_err(to_gql_error)
    }

    /// `POST /scim/v2/Users`(作成と同時に鍵を自動発行し、平文鍵を一度だけ返す)
    async fn scim_create_user(
        &self,
        ctx: &Context<'_>,
        user_name: String,
        roles: Option<Vec<String>>,
    ) -> async_graphql::Result<Json<serde_json::Value>> {
        let guardian = ctx.data_unchecked::<Arc<KeyGuardian>>();
        admin_sys::scim_create_user_named(
            state(ctx),
            guardian,
            &self.actor,
            &user_name,
            roles.unwrap_or_default(),
        )
        .await
        .map(Json)
        .map_err(to_gql_error)
    }

    /// `DELETE /scim/v2/Users/:id`
    async fn scim_delete_user(&self, ctx: &Context<'_>, id: String) -> async_graphql::Result<bool> {
        let guardian = ctx.data_unchecked::<Arc<KeyGuardian>>();
        admin_sys::scim_delete_user(state(ctx), guardian, &self.actor, &id)
            .await
            .map(|()| true)
            .map_err(to_gql_error)
    }

    /// `POST /api/cache/purge`
    async fn purge_cache(&self, ctx: &Context<'_>, path: String) -> PurgeResult {
        let st = state(ctx);
        PurgeResult {
            purged: admin_ops::cache_purge(st, &st.page_cache, &self.actor, &path).await,
        }
    }

    /// `POST /api/cache/purge-all`
    async fn purge_all_cache(&self, ctx: &Context<'_>) -> PurgeResult {
        let st = state(ctx);
        PurgeResult {
            purged: admin_ops::cache_purge_all(st, &st.page_cache, &self.actor).await,
        }
    }
}
