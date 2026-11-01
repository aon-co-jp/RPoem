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

use async_graphql::{Context, ErrorExtensions, InputObject, Object, SimpleObject};
use open_runo_api_types::{
    FeatureFlagEvaluationResponse, FeatureFlagRequest, FeatureFlagResponse, RegisterSchemaRequest,
    SchemaVersion,
};
use open_runo_router::admin_ops::{self, AdminError, ComposeService};
use open_runo_router::auth_hyper::Actor;
use open_runo_router::state::AppState;
use std::collections::BTreeMap;
use std::sync::Arc;

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
