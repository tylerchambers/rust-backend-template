use crate::{
    authentication::actor,
    error::{ApiError, ErrorBody},
    state::State,
};
use application::reauthentication::ProofPurpose;
use axum::{
    Json,
    extract::State as Extract,
    http::{HeaderMap, StatusCode},
    response::Html,
};
use serde::Deserialize;
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct ProofRequest {
    #[schema(schema_with = action_schema)]
    action: String,
    /// For `invite`, use `organization_id`; for `revoke_invitation`, use the invitation ID.
    /// For `change_role` and `remove_member`, use the affected user's ID.
    target: Uuid,
    organization_id: Uuid,
}

fn action_schema() -> utoipa::openapi::schema::Object {
    utoipa::openapi::schema::ObjectBuilder::new()
        .schema_type(utoipa::openapi::schema::Type::String)
        .enum_values(Some([
            "change_role",
            "invite",
            "revoke_invitation",
            "remove_member",
        ]))
        .build()
}

#[utoipa::path(
    post,
    path="/v1/auth/reauth/request",
    request_body=ProofRequest,
    security(("session"=[])),
    params(
        ("Origin"=String,Header,description="Exact configured public origin"),
        ("x-csrf-token"=String,Header,description="Current csrf cookie value; required with matching csrf cookie"),
    ),
    responses(
        (status="default",description="Stable JSON error envelope, including transport, authentication, authorization and availability failures",body=ErrorBody),
        (status=202),
    )
)]
pub(crate) async fn request_proof(
    Extract(state): Extract<State>,
    headers: HeaderMap,
    Json(input): Json<ProofRequest>,
) -> Result<StatusCode, ApiError> {
    let actor = actor(&state, &headers).await?;
    let purpose = match input.action.as_str() {
        "change_role" => ProofPurpose::ChangeRole(input.target.into()),
        "invite" if input.target == input.organization_id => ProofPurpose::Invite,
        "revoke_invitation" => ProofPurpose::RevokeInvitation(input.target.into()),
        "remove_member" => ProofPurpose::RemoveMember(input.target.into()),
        _ => return Err(ApiError(domain::Error::Invalid)),
    };
    state
        .reauthentication
        .request(&actor, purpose, input.organization_id.into())
        .await?;
    Ok(StatusCode::ACCEPTED)
}

#[utoipa::path(
    get,
    path="/v1/auth/reauth/confirmation",
    responses(
        (status="default",description="Stable JSON error envelope, including transport, authentication, authorization and availability failures",body=ErrorBody),
        (status=200,body=String,content_type="text/html"),
    )
)]
pub(crate) async fn proof_confirmation() -> Html<&'static str> {
    Html(include_str!("pages/reauthentication.html"))
}
