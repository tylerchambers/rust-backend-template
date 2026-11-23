use crate::{
    authentication::actor,
    error::{ApiError, ErrorBody},
    openapi::role_schema,
    pagination::UuidCursor,
    state::State,
};
use axum::{
    Json,
    extract::{Path, Query, State as Extract},
    http::{HeaderMap, StatusCode},
};
use domain::Error;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Serialize, ToSchema)]
pub struct MemberSummaryResponse {
    user_id: Uuid,
    #[schema(schema_with = role_schema)]
    role: domain::Role,
}

#[derive(Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct RemoveMemberRequest {
    proof: String,
}
#[utoipa::path(
    post,
    path="/v1/organizations/{organization}/members/{user}/remove",
    params(
        ("Origin"=String,Header,description="Exact configured public origin"),
        ("x-csrf-token"=String,Header,description="Current csrf cookie value; required with matching csrf cookie"),
        ("organization"=Uuid,Path),
        ("user"=Uuid,Path),
    ),
    request_body=RemoveMemberRequest,
    security(("session"=[])),
    responses(
        (status="default",description="Stable JSON error envelope, including transport, authentication, authorization and availability failures",body=ErrorBody),
        (status=204),
    )
)]
pub(crate) async fn remove_member(
    Extract(state): Extract<State>,
    Path((org, user)): Path<(Uuid, Uuid)>,
    headers: HeaderMap,
    Json(input): Json<RemoveMemberRequest>,
) -> Result<StatusCode, ApiError> {
    let actor = actor(&state, &headers).await?;
    state
        .memberships
        .remove(&actor, org.into(), user.into(), &input.proof)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}
#[derive(Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct ChangeMemberRoleRequest {
    #[schema(schema_with = role_schema)]
    role: String,
    proof: String,
}
pub(super) fn role(value: &str) -> Result<domain::Role, Error> {
    match value {
        "owner" => Ok(domain::Role::Owner),
        "admin" => Ok(domain::Role::Admin),
        "member" => Ok(domain::Role::Member),
        "viewer" => Ok(domain::Role::Viewer),
        _ => Err(Error::Invalid),
    }
}
#[utoipa::path(
    post,
    path="/v1/organizations/{organization}/members/{user}/role",
    params(
        ("Origin"=String,Header,description="Exact configured public origin"),
        ("x-csrf-token"=String,Header,description="Current csrf cookie value; required with matching csrf cookie"),
        ("organization"=Uuid,Path),
        ("user"=Uuid,Path),
    ),
    request_body=ChangeMemberRoleRequest,
    security(("session"=[])),
    responses(
        (status="default",description="Stable JSON error envelope, including transport, authentication, authorization and availability failures",body=ErrorBody),
        (status=204),
    )
)]
pub(crate) async fn change_role(
    Extract(state): Extract<State>,
    Path((org, user)): Path<(Uuid, Uuid)>,
    headers: HeaderMap,
    Json(input): Json<ChangeMemberRoleRequest>,
) -> Result<StatusCode, ApiError> {
    let actor = actor(&state, &headers).await?;
    state
        .memberships
        .change_role(
            &actor,
            org.into(),
            user.into(),
            role(&input.role)?,
            &input.proof,
        )
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(
    get,
    path="/v1/organizations/{organization}/members",
    params(
        ("organization"=Uuid,Path),
        ("after"=Option<Uuid>,Query,description="Exclusive user ID cursor; at most 50 rows"),
    ),
    security(("session"=[])),
    responses(
        (status="default",description="Stable JSON error envelope, including transport, authentication, authorization and availability failures",body=ErrorBody),
        (status=200,body=Vec<MemberSummaryResponse>),
    )
)]
pub(crate) async fn list_members(
    Extract(state): Extract<State>,
    Path(org): Path<Uuid>,
    Query(page): Query<UuidCursor>,
    headers: HeaderMap,
) -> Result<Json<Vec<MemberSummaryResponse>>, ApiError> {
    let actor = actor(&state, &headers).await?;
    Ok(Json(
        state
            .organizations
            .members(&actor, org.into(), page.after.map(Into::into))
            .await?
            .into_iter()
            .map(|row| MemberSummaryResponse {
                user_id: row.user_id.into_uuid(),
                role: row.role,
            })
            .collect(),
    ))
}

#[derive(Serialize, ToSchema)]
pub struct DirectoryMemberResponse {
    user_id: Uuid,
    email: String,
    #[schema(schema_with = role_schema)]
    role: domain::Role,
}

#[utoipa::path(
    get,
    path="/v1/organizations/{organization}/member-directory",
    params(
        ("organization"=Uuid,Path),
        ("after"=Option<Uuid>,Query,description="Exclusive user ID cursor. Up to 50 rows in ascending user ID order; continue with the last user ID until empty."),
    ),
    security(("session"=[])),
    responses(
        (status="default",description="Stable JSON error envelope, including transport, authentication, authorization and availability failures",body=ErrorBody),
        (status=200,body=Vec<DirectoryMemberResponse>),
    )
)]
pub(crate) async fn list_member_directory(
    Extract(state): Extract<State>,
    Path(org): Path<Uuid>,
    Query(page): Query<UuidCursor>,
    headers: HeaderMap,
) -> Result<Json<Vec<DirectoryMemberResponse>>, ApiError> {
    let actor = actor(&state, &headers).await?;
    Ok(Json(
        state
            .administration
            .member_directory(&actor, org.into(), page.after.map(Into::into))
            .await?
            .into_iter()
            .map(|row| DirectoryMemberResponse {
                user_id: row.user_id.into_uuid(),
                email: row.email,
                role: row.role,
            })
            .collect(),
    ))
}
