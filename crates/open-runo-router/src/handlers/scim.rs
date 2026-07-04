//! `/scim/v2/Users` — SCIM 2.0 provisioning surface (RFC 7644).
//!
//! | Method | Path                  | Description                        |
//! |--------|-----------------------|------------------------------------|
//! | GET    | `/scim/v2/Users`      | List (optional `?filter=userName eq "x"`) |
//! | POST   | `/scim/v2/Users`      | Create (201)                       |
//! | GET    | `/scim/v2/Users/:id`  | Fetch                              |
//! | PUT    | `/scim/v2/Users/:id`  | Replace                            |
//! | DELETE | `/scim/v2/Users/:id`  | Delete (204)                       |
//!
//! Auth: X-Api-Key / JWT / OIDC のいずれか、または IdP 用の固定トークン
//! `OPEN_RUNO_SCIM_TOKEN`（`Authorization: Bearer`）。RBAC 有効時は
//! `Resource::Admin` 権限が必要。

use crate::keyring::KeyGuardian;
use crate::state::AppState;
use open_runo_scim::{
    Group, GroupInput, GroupListResponse, ListResponse, ScimGroupStore, ScimUserStore, User,
    UserInput,
};
use poem::{
    handler,
    http::StatusCode,
    web::{Data, Json, Path, Query},
    IntoResponse, Response,
};
use serde::Deserialize;
use std::sync::Arc;

fn store(state: &AppState) -> ScimUserStore {
    ScimUserStore::new(Arc::clone(&state.db))
}

fn scim_error(status: StatusCode, msg: impl std::fmt::Display) -> poem::Error {
    poem::Error::from_string(msg.to_string(), status)
}

#[derive(Debug, Deserialize)]
pub struct ListParams {
    #[serde(default)]
    pub filter: Option<String>,
}

/// GET /scim/v2/Users
#[handler]
pub async fn list_users(
    state: Data<&Arc<AppState>>,
    Query(params): Query<ListParams>,
) -> poem::Result<Json<ListResponse>> {
    let filter = params
        .filter
        .as_deref()
        .and_then(ScimUserStore::parse_user_name_filter);

    let users = store(&state)
        .list(filter.as_deref())
        .await
        .map_err(|e| scim_error(StatusCode::INTERNAL_SERVER_ERROR, e))?;

    Ok(Json(ListResponse {
        schemas: vec!["urn:ietf:params:scim:api:messages:2.0:ListResponse".into()],
        total_results: users.len(),
        resources: users,
    }))
}

/// POST /scim/v2/Users → 201 Created
///
/// Provisioning is also the moment KeyGuardian **auto-issues** an API key
/// bound to the user's RBAC roles. The plaintext appears exactly once, in
/// the `urn:open-runo:params:scim:api-key` extension of this response —
/// nobody ever manages keys by hand.
#[handler]
pub async fn create_user(
    req: &poem::Request,
    state: Data<&Arc<AppState>>,
    guardian: Data<&Arc<KeyGuardian>>,
    Json(input): Json<UserInput>,
) -> poem::Result<Response> {
    let user = store(&state)
        .create(input)
        .await
        .map_err(|e| scim_error(StatusCode::CONFLICT, e))?;

    crate::audit::record(
        &state,
        &crate::audit::actor_from(req),
        "scim.user.create",
        user.user_name.clone(),
    )
    .await;

    // Auto-issue: key lifecycle follows the user lifecycle.
    let mut body = serde_json::to_value(&user)
        .map_err(|e| scim_error(StatusCode::INTERNAL_SERVER_ERROR, e))?;
    match guardian.issue(&user.user_name, user.roles.clone(), None).await {
        Ok(plaintext) => {
            body["urn:open-runo:params:scim:api-key"] = serde_json::Value::String(plaintext);
            crate::audit::record(
                &state,
                "key-guardian",
                "key.auto_issue",
                user.user_name.clone(),
            )
            .await;
        }
        Err(e) => {
            tracing::warn!(error = %e, owner = %user.user_name, "auto key issue failed");
        }
    }

    Ok((StatusCode::CREATED, Json(body)).into_response())
}

/// GET /scim/v2/Users/:id
#[handler]
pub async fn get_user(
    Path(id): Path<String>,
    state: Data<&Arc<AppState>>,
) -> poem::Result<Json<User>> {
    store(&state)
        .get(&id)
        .await
        .map_err(|e| scim_error(StatusCode::INTERNAL_SERVER_ERROR, e))?
        .map(Json)
        .ok_or_else(|| scim_error(StatusCode::NOT_FOUND, format!("user not found: {id}")))
}

/// PUT /scim/v2/Users/:id — replace all mutable attributes.
/// Deactivation (`active: false`) **auto-revokes** every key of the user.
#[handler]
pub async fn replace_user(
    req: &poem::Request,
    Path(id): Path<String>,
    state: Data<&Arc<AppState>>,
    guardian: Data<&Arc<KeyGuardian>>,
    Json(input): Json<UserInput>,
) -> poem::Result<Json<User>> {
    let user = store(&state).replace(&id, input).await.map_err(|e| {
        let msg = e.to_string();
        if msg.contains("not found") {
            scim_error(StatusCode::NOT_FOUND, msg)
        } else {
            scim_error(StatusCode::CONFLICT, msg)
        }
    })?;

    crate::audit::record(
        &state,
        &crate::audit::actor_from(req),
        "scim.user.replace",
        user.user_name.clone(),
    )
    .await;

    if !user.active {
        if let Ok(n) = guardian.revoke_owner(&user.user_name).await {
            if n > 0 {
                crate::audit::record(
                    &state,
                    "key-guardian",
                    "key.auto_revoke",
                    format!("{} ({n} keys, deactivated)", user.user_name),
                )
                .await;
            }
        }
    }

    Ok(Json(user))
}

/// DELETE /scim/v2/Users/:id → 204 No Content.
/// The user's API keys are **auto-revoked** with them.
#[handler]
pub async fn delete_user(
    req: &poem::Request,
    Path(id): Path<String>,
    state: Data<&Arc<AppState>>,
    guardian: Data<&Arc<KeyGuardian>>,
) -> poem::Result<StatusCode> {
    // Look up the owner before deleting so keys can follow.
    let owner = store(&state)
        .get(&id)
        .await
        .map_err(|e| scim_error(StatusCode::INTERNAL_SERVER_ERROR, e))?
        .map(|u| u.user_name);

    store(&state)
        .delete(&id)
        .await
        .map_err(|e| scim_error(StatusCode::INTERNAL_SERVER_ERROR, e))?;

    crate::audit::record(&state, &crate::audit::actor_from(req), "scim.user.delete", id).await;

    if let Some(owner) = owner {
        if let Ok(n) = guardian.revoke_owner(&owner).await {
            if n > 0 {
                crate::audit::record(
                    &state,
                    "key-guardian",
                    "key.auto_revoke",
                    format!("{owner} ({n} keys, deleted)"),
                )
                .await;
            }
        }
    }

    Ok(StatusCode::NO_CONTENT)
}

// ── Groups ─────────────────────────────────────────────────────────────────

fn group_store(state: &AppState) -> ScimGroupStore {
    ScimGroupStore::new(Arc::clone(&state.db))
}

/// GET /scim/v2/Groups
#[handler]
pub async fn list_groups(
    state: Data<&Arc<AppState>>,
    Query(params): Query<ListParams>,
) -> poem::Result<Json<GroupListResponse>> {
    let filter = params
        .filter
        .as_deref()
        .and_then(ScimGroupStore::parse_display_name_filter);

    let groups = group_store(&state)
        .list(filter.as_deref())
        .await
        .map_err(|e| scim_error(StatusCode::INTERNAL_SERVER_ERROR, e))?;

    Ok(Json(GroupListResponse {
        schemas: vec!["urn:ietf:params:scim:api:messages:2.0:ListResponse".into()],
        total_results: groups.len(),
        resources: groups,
    }))
}

/// POST /scim/v2/Groups → 201 Created
#[handler]
pub async fn create_group(
    req: &poem::Request,
    state: Data<&Arc<AppState>>,
    Json(input): Json<GroupInput>,
) -> poem::Result<Response> {
    let group = group_store(&state)
        .create(input)
        .await
        .map_err(|e| scim_error(StatusCode::CONFLICT, e))?;

    crate::audit::record(
        &state,
        &crate::audit::actor_from(req),
        "scim.group.create",
        group.display_name.clone(),
    )
    .await;

    Ok((StatusCode::CREATED, Json(group)).into_response())
}

/// GET /scim/v2/Groups/:id
#[handler]
pub async fn get_group(
    Path(id): Path<String>,
    state: Data<&Arc<AppState>>,
) -> poem::Result<Json<Group>> {
    group_store(&state)
        .get(&id)
        .await
        .map_err(|e| scim_error(StatusCode::INTERNAL_SERVER_ERROR, e))?
        .map(Json)
        .ok_or_else(|| scim_error(StatusCode::NOT_FOUND, format!("group not found: {id}")))
}

/// PUT /scim/v2/Groups/:id — replace displayName + full membership.
#[handler]
pub async fn replace_group(
    req: &poem::Request,
    Path(id): Path<String>,
    state: Data<&Arc<AppState>>,
    Json(input): Json<GroupInput>,
) -> poem::Result<Json<Group>> {
    let group = group_store(&state).replace(&id, input).await.map_err(|e| {
        let msg = e.to_string();
        if msg.contains("not found") {
            scim_error(StatusCode::NOT_FOUND, msg)
        } else {
            scim_error(StatusCode::CONFLICT, msg)
        }
    })?;

    crate::audit::record(
        &state,
        &crate::audit::actor_from(req),
        "scim.group.replace",
        group.display_name.clone(),
    )
    .await;

    Ok(Json(group))
}

/// DELETE /scim/v2/Groups/:id → 204 No Content
#[handler]
pub async fn delete_group(
    req: &poem::Request,
    Path(id): Path<String>,
    state: Data<&Arc<AppState>>,
) -> poem::Result<StatusCode> {
    group_store(&state)
        .delete(&id)
        .await
        .map_err(|e| scim_error(StatusCode::INTERNAL_SERVER_ERROR, e))?;

    crate::audit::record(&state, &crate::audit::actor_from(req), "scim.group.delete", id).await;

    Ok(StatusCode::NO_CONTENT)
}
