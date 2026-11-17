use crate::{
    authentication::actor,
    error::{ApiError, ErrorBody},
    pagination::UuidCursor,
    state::State,
};
use axum::{
    Json,
    extract::{Path, Query, State as Extract},
    http::{HeaderMap, StatusCode},
};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct RenameProjectRequest {
    name: String,
    version: i64,
}

#[derive(Serialize, ToSchema)]
pub struct ProjectResponse {
    id: Uuid,
    organization_id: Uuid,
    name: String,
    version: i64,
    archived: bool,
}
impl From<domain::Project> for ProjectResponse {
    fn from(p: domain::Project) -> Self {
        Self {
            id: p.id.into_uuid(),
            organization_id: p.organization_id.into_uuid(),
            name: p.name,
            version: p.version,
            archived: p.archived,
        }
    }
}

#[utoipa::path(
    post,
    path="/v1/organizations/{organization}/projects",
    params(
        ("Origin"=String,Header,description="Exact configured public origin"),
        ("x-csrf-token"=String,Header,description="Current csrf cookie value; required with matching csrf cookie"),
        ("organization"=Uuid,Path),
    ),
    request_body=CreateProjectRequest,
    security(("session"=[])),
    responses(
        (status="default",description="Stable JSON error envelope, including transport, authentication, authorization and availability failures",body=ErrorBody),
        (status=201,body=ProjectResponse),
    )
)]
pub(crate) async fn create_project(
    Extract(state): Extract<State>,
    Path(org): Path<Uuid>,
    headers: HeaderMap,
    Json(input): Json<CreateProjectRequest>,
) -> Result<(StatusCode, Json<ProjectResponse>), ApiError> {
    let actor = actor(&state, &headers).await?;
    Ok((
        StatusCode::CREATED,
        Json(
            state
                .projects
                .create(&actor, org.into(), input.name)
                .await?
                .into(),
        ),
    ))
}

#[utoipa::path(
    get,
    path="/v1/organizations/{organization}/projects",
    params(
        ("organization"=Uuid,Path),
        ("after"=Option<Uuid>,Query,description="Exclusive project ID cursor; at most 50 rows"),
    ),
    security(("session"=[])),
    responses(
        (status="default",description="Stable JSON error envelope, including transport, authentication, authorization and availability failures",body=ErrorBody),
        (status=200,body=Vec<ProjectResponse>),
    )
)]
pub(crate) async fn list_projects(
    Extract(state): Extract<State>,
    Path(org): Path<Uuid>,
    Query(page): Query<UuidCursor>,
    headers: HeaderMap,
) -> Result<Json<Vec<ProjectResponse>>, ApiError> {
    let actor = actor(&state, &headers).await?;
    Ok(Json(
        state
            .projects
            .list(&actor, org.into(), page.after.map(Into::into))
            .await?
            .into_iter()
            .map(Into::into)
            .collect(),
    ))
}

#[utoipa::path(
    patch,
    path="/v1/organizations/{organization}/projects/{project}",
    params(
        ("Origin"=String,Header,description="Exact configured public origin"),
        ("x-csrf-token"=String,Header,description="Current csrf cookie value; required with matching csrf cookie"),
        ("organization"=Uuid,Path),
        ("project"=Uuid,Path),
    ),
    request_body=RenameProjectRequest,
    security(("session"=[])),
    responses(
        (status="default",description="Stable JSON error envelope, including transport, authentication, authorization and availability failures",body=ErrorBody),
        (status=200,body=ProjectResponse),
        (status=409,body=ErrorBody),
    )
)]
pub(crate) async fn rename_project(
    Extract(state): Extract<State>,
    Path((org, id)): Path<(Uuid, Uuid)>,
    headers: HeaderMap,
    Json(input): Json<RenameProjectRequest>,
) -> Result<Json<ProjectResponse>, ApiError> {
    let actor = actor(&state, &headers).await?;
    Ok(Json(
        state
            .projects
            .rename(&actor, org.into(), id.into(), input.version, input.name)
            .await?
            .into(),
    ))
}

#[derive(Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct CreateProjectRequest {
    name: String,
}
