use crate::{
    authentication::actor,
    cookies::csrf_cookie,
    error::{ApiError, ErrorBody},
    pagination::UuidCursor,
    state::State,
};
use application::administration::InvitationStatus;
use axum::{
    Json,
    extract::{Path, Query, State as Extract},
    http::{HeaderMap, StatusCode, header},
    response::{Html, IntoResponse, Response},
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum InvitationStatusResponse {
    Pending,
    Accepted,
    Revoked,
    Expired,
}

impl From<InvitationStatus> for InvitationStatusResponse {
    fn from(status: InvitationStatus) -> Self {
        match status {
            InvitationStatus::Pending => Self::Pending,
            InvitationStatus::Accepted => Self::Accepted,
            InvitationStatus::Revoked => Self::Revoked,
            InvitationStatus::Expired => Self::Expired,
        }
    }
}

#[derive(Serialize, ToSchema)]
pub struct InvitationSummaryResponse {
    id: Uuid,
    email: String,
    is_admin: bool,
    status: InvitationStatusResponse,
    expires_at: DateTime<Utc>,
}

#[utoipa::path(
    get,
    path="/v1/organizations/{organization}/invitations",
    params(
        ("organization"=Uuid,Path),
        ("after"=Option<Uuid>,Query,description="Exclusive invitation ID cursor. Up to 50 rows in ascending ID order; continue with the last ID until empty."),
    ),
    security(("session"=[])),
    responses(
        (status="default",description="Stable JSON error envelope, including transport, authentication, authorization and availability failures",body=ErrorBody),
        (status=200,body=Vec<InvitationSummaryResponse>),
    )
)]
pub(crate) async fn list_invitations(
    Extract(state): Extract<State>,
    Path(org): Path<Uuid>,
    Query(page): Query<UuidCursor>,
    headers: HeaderMap,
) -> Result<Json<Vec<InvitationSummaryResponse>>, ApiError> {
    let actor = actor(&state, &headers).await?;
    Ok(Json(
        state
            .administration
            .invitations(&actor, org.into(), page.after.map(Into::into))
            .await?
            .into_iter()
            .map(|row| InvitationSummaryResponse {
                id: row.id.into_uuid(),
                email: row.email,
                is_admin: row.is_admin,
                status: row.status.into(),
                expires_at: row.expires_at,
            })
            .collect(),
    ))
}

#[derive(Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct InvitationRequest {
    email: String,
    is_admin: bool,
}

#[utoipa::path(
    post,
    path="/v1/organizations/{organization}/invitations/{invitation}/revoke",
    params(
        ("Origin"=String,Header,description="Exact configured public origin"),
        ("x-csrf-token"=String,Header,description="Current csrf cookie value; required with matching csrf cookie"),
        ("organization"=Uuid,Path),
        ("invitation"=Uuid,Path),
    ),
    security(("session"=[])),
    responses(
        (status="default",description="Stable JSON error envelope, including transport, authentication, authorization and availability failures",body=ErrorBody),
        (status=204),
    )
)]
pub(crate) async fn revoke_invitation(
    Extract(state): Extract<State>,
    Path((org, id)): Path<(Uuid, Uuid)>,
    headers: HeaderMap,
) -> Result<StatusCode, ApiError> {
    let actor = actor(&state, &headers).await?;
    state
        .invitations
        .revoke_invitation(&actor, org.into(), id.into())
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(
    post,
    path="/v1/organizations/{organization}/invitations",
    params(
        ("Origin"=String,Header,description="Exact configured public origin"),
        ("x-csrf-token"=String,Header,description="Current csrf cookie value; required with matching csrf cookie"),
        ("organization"=Uuid,Path),
    ),
    request_body=InvitationRequest,
    security(("session"=[])),
    responses(
        (status="default",description="Stable JSON error envelope, including transport, authentication, authorization and availability failures",body=ErrorBody),
        (status=201,body=InvitationResponse),
    )
)]
pub(crate) async fn invite(
    Extract(state): Extract<State>,
    Path(org): Path<Uuid>,
    headers: HeaderMap,
    Json(input): Json<InvitationRequest>,
) -> Result<(StatusCode, Json<InvitationResponse>), ApiError> {
    let actor = actor(&state, &headers).await?;
    let id = state
        .invitations
        .invite(&actor, org.into(), &input.email, input.is_admin)
        .await?;
    Ok((
        StatusCode::CREATED,
        Json(InvitationResponse { id: id.into_uuid() }),
    ))
}

#[utoipa::path(
    post,
    path="/v1/organizations/{organization}/invitations/{invitation}/accept",
    params(
        ("Origin"=String,Header,description="Exact configured public origin"),
        ("x-csrf-token"=String,Header,description="Current csrf cookie value; required with matching csrf cookie"),
        ("organization"=Uuid,Path),
        ("invitation"=Uuid,Path),
    ),
    request_body=AcceptInvitationRequest,
    security(("session"=[])),
    responses(
        (status="default",description="Stable JSON error envelope, including transport, authentication, authorization and availability failures",body=ErrorBody),
        (status=204),
    )
)]
pub(crate) async fn accept(
    Extract(state): Extract<State>,
    Path((org, id)): Path<(Uuid, Uuid)>,
    headers: HeaderMap,
    Json(input): Json<AcceptInvitationRequest>,
) -> Result<StatusCode, ApiError> {
    let actor = actor(&state, &headers).await?;
    state
        .invitations
        .accept(&actor, org.into(), id.into(), &input.token)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(
    get,
    path="/v1/invitations/confirmation",
    responses(
        (status="default",description="Stable JSON error envelope, including transport, authentication, authorization and availability failures",body=ErrorBody),
        (status=200,body=String,content_type="text/html"),
    )
)]
pub(crate) async fn invitation_confirmation(Extract(state): Extract<State>) -> Response {
    let csrf = state.secrets.token();
    let html = include_str!("pages/invitation.html");
    (
        [(header::SET_COOKIE, csrf_cookie(csrf).to_string())],
        Html(html),
    )
        .into_response()
}

#[derive(Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct AcceptInvitationRequest {
    token: String,
}

#[derive(Serialize, ToSchema)]
pub struct InvitationResponse {
    id: Uuid,
}
