use crate::{
    authentication::actor,
    error::{ApiError, ErrorBody},
    pagination::UuidCursor,
    state::State,
};
use axum::{
    Json,
    extract::{Query, State as Extract},
    http::{HeaderMap, StatusCode},
};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Serialize, ToSchema)]
pub struct OrganizationResponse {
    id: Uuid,
}

#[derive(Serialize, ToSchema)]
pub struct OrganizationSummaryResponse {
    id: Uuid,
    name: String,
    is_admin: bool,
}

#[utoipa::path(
    post,
    path="/v1/organizations",
    request_body=CreateOrganizationRequest,
    security(("session"=[])),
    params(
        ("Origin"=String,Header,description="Exact configured public origin"),
        ("x-csrf-token"=String,Header,description="Current csrf cookie value; required with matching csrf cookie"),
    ),
    responses(
        (status="default",description="Stable JSON error envelope, including transport, authentication, authorization and availability failures",body=ErrorBody),
        (status=201,body=OrganizationResponse),
    )
)]
pub(crate) async fn create_org(
    Extract(state): Extract<State>,
    headers: HeaderMap,
    Json(input): Json<CreateOrganizationRequest>,
) -> Result<(StatusCode, Json<OrganizationResponse>), ApiError> {
    let actor = actor(&state, &headers).await?;
    let id = state.organizations.create(&actor, input.name).await?;
    Ok((
        StatusCode::CREATED,
        Json(OrganizationResponse { id: id.into_uuid() }),
    ))
}

#[utoipa::path(
    get,
    path="/v1/organizations",
    params(
        ("after"=Option<Uuid>,Query,description="Exclusive organization ID cursor; at most 50 rows"),
    ),
    security(("session"=[])),
    responses(
        (status="default",description="Stable JSON error envelope, including transport, authentication, authorization and availability failures",body=ErrorBody),
        (status=200,body=Vec<OrganizationSummaryResponse>),
    )
)]
pub(crate) async fn list_organizations(
    Extract(state): Extract<State>,
    Query(page): Query<UuidCursor>,
    headers: HeaderMap,
) -> Result<Json<Vec<OrganizationSummaryResponse>>, ApiError> {
    let actor = actor(&state, &headers).await?;
    Ok(Json(
        state
            .organizations
            .list(&actor, page.after.map(Into::into))
            .await?
            .into_iter()
            .map(|row| OrganizationSummaryResponse {
                id: row.id.into_uuid(),
                name: row.name,
                is_admin: row.is_admin,
            })
            .collect(),
    ))
}

#[derive(Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct CreateOrganizationRequest {
    name: String,
}
