use crate::{
    authentication::actor,
    cookies::csrf_cookie,
    error::{ApiError, ErrorBody},
    memberships::role,
    openapi::role_schema,
    state::State,
};
use axum::{
    Json,
    extract::{Path, State as Extract},
    http::{HeaderMap, StatusCode, header},
    response::{Html, IntoResponse, Response},
};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct InvitationRequest {
    email: String,
    #[schema(schema_with = role_schema)]
    role: String,
    proof: String,
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
    request_body=RevokeInvitationRequest,
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
    Json(input): Json<RevokeInvitationRequest>,
) -> Result<StatusCode, ApiError> {
    let actor = actor(&state, &headers).await?;
    state
        .invitations
        .revoke_invitation(&actor, org.into(), id.into(), &input.proof)
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
        .invite(
            &actor,
            org.into(),
            &input.email,
            role(&input.role)?,
            &input.proof,
        )
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
pub struct RevokeInvitationRequest {
    proof: String,
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
