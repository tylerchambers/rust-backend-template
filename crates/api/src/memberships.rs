#[derive(Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct PrivilegedRequest {
    proof: String,
}
#[utoipa::path(post,path="/v1/organizations/{organization}/invitations/{invitation}/revoke",params(("organization"=Uuid,Path),("invitation"=Uuid,Path)),request_body=PrivilegedRequest,responses((status=204)))]
pub async fn revoke_invitation(
    Extract(state): Extract<State>,
    Path((org, id)): Path<(Uuid, Uuid)>,
    headers: HeaderMap,
    Json(input): Json<PrivilegedRequest>,
) -> Result<StatusCode, ApiError> {
    let actor = actor(&state, &headers).await?;
    state
        .memberships
        .revoke_invitation(&actor, org.into(), id.into(), &input.proof)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}
#[utoipa::path(post,path="/v1/organizations/{organization}/members/{user}/remove",params(("organization"=Uuid,Path),("user"=Uuid,Path)),request_body=PrivilegedRequest,responses((status=204)))]
pub async fn remove_member(
    Extract(state): Extract<State>,
    Path((org, user)): Path<(Uuid, Uuid)>,
    headers: HeaderMap,
    Json(input): Json<PrivilegedRequest>,
) -> Result<StatusCode, ApiError> {
    let actor = actor(&state, &headers).await?;
    state
        .memberships
        .remove(&actor, org.into(), user.into(), &input.proof)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}
use super::{
    ApiError, ConfirmRequest, Deserialize, Error, ErrorBody, Extract, HeaderMap, Html,
    IntoResponse, Json, OrganizationResponse, Path, Response, State, StatusCode, ToSchema, Uuid,
    actor, cookie, header,
};
#[derive(Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct ProofRequest {
    action: String,
    target: Uuid,
    organization_id: Uuid,
}
#[derive(Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct InvitationRequest {
    email: String,
    role: String,
    proof: String,
}
#[derive(Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct RoleRequest {
    role: String,
    proof: String,
}
fn role(value: &str) -> Result<domain::Role, Error> {
    match value {
        "owner" => Ok(domain::Role::Owner),
        "admin" => Ok(domain::Role::Admin),
        "member" => Ok(domain::Role::Member),
        "viewer" => Ok(domain::Role::Viewer),
        _ => Err(Error::Invalid),
    }
}
#[utoipa::path(post,path="/v1/auth/reauth/request",request_body=ProofRequest,responses((status=202)))]
pub async fn request_proof(
    Extract(state): Extract<State>,
    headers: HeaderMap,
    Json(input): Json<ProofRequest>,
) -> Result<StatusCode, ApiError> {
    let actor = actor(&state, &headers).await?;
    state
        .reauthentication
        .request(
            &actor,
            &input.action,
            input.target,
            input.organization_id.into(),
        )
        .await?;
    Ok(StatusCode::ACCEPTED)
}
#[utoipa::path(post,path="/v1/organizations/{organization}/invitations",params(("organization"=Uuid,Path)),request_body=InvitationRequest,responses((status=201,body=OrganizationResponse)))]
pub async fn invite(
    Extract(state): Extract<State>,
    Path(org): Path<Uuid>,
    headers: HeaderMap,
    Json(input): Json<InvitationRequest>,
) -> Result<(StatusCode, Json<OrganizationResponse>), ApiError> {
    let actor = actor(&state, &headers).await?;
    let id = state
        .memberships
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
        Json(OrganizationResponse { id: id.into_uuid() }),
    ))
}
#[utoipa::path(post,path="/v1/organizations/{organization}/invitations/{invitation}/accept",params(("organization"=Uuid,Path),("invitation"=Uuid,Path)),request_body=ConfirmRequest,responses((status=204)))]
pub async fn accept(
    Extract(state): Extract<State>,
    Path((org, id)): Path<(Uuid, Uuid)>,
    headers: HeaderMap,
    Json(input): Json<ConfirmRequest>,
) -> Result<StatusCode, ApiError> {
    let actor = actor(&state, &headers).await?;
    state
        .memberships
        .accept(&actor, org.into(), id.into(), &input.token)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}
#[utoipa::path(post,path="/v1/organizations/{organization}/members/{user}/role",params(("organization"=Uuid,Path),("user"=Uuid,Path)),request_body=RoleRequest,responses((status=204)))]
pub async fn change_role(
    Extract(state): Extract<State>,
    Path((org, user)): Path<(Uuid, Uuid)>,
    headers: HeaderMap,
    Json(input): Json<RoleRequest>,
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
#[utoipa::path(get,path="/v1/auth/sessions",responses((status=200,body=Vec<Uuid>)))]
pub async fn sessions(
    Extract(state): Extract<State>,
    headers: HeaderMap,
) -> Result<Json<Vec<Uuid>>, ApiError> {
    Ok(Json(
        state
            .auth
            .sessions(
                cookie(&headers, "__Host-session")
                    .as_deref()
                    .ok_or(Error::Unauthenticated)?,
            )
            .await?
            .into_iter()
            .map(domain::SessionId::into_uuid)
            .collect(),
    ))
}
#[utoipa::path(post,path="/v1/auth/sessions/{session}/revoke",params(("session"=Uuid,Path)),responses((status=204),(status=404,body=ErrorBody)))]
pub async fn revoke_session(
    Extract(state): Extract<State>,
    Path(session): Path<Uuid>,
    headers: HeaderMap,
) -> Result<StatusCode, ApiError> {
    state
        .auth
        .revoke_session(
            cookie(&headers, "__Host-session")
                .as_deref()
                .ok_or(Error::Unauthenticated)?,
            session.into(),
        )
        .await?;
    Ok(StatusCode::NO_CONTENT)
}
#[utoipa::path(post,path="/v1/auth/sessions/revoke-all",responses((status=204)))]
pub async fn revoke_all(
    Extract(state): Extract<State>,
    headers: HeaderMap,
) -> Result<StatusCode, ApiError> {
    state
        .auth
        .revoke_all(
            cookie(&headers, "__Host-session")
                .as_deref()
                .ok_or(Error::Unauthenticated)?,
        )
        .await?;
    Ok(StatusCode::NO_CONTENT)
}
#[utoipa::path(post,path="/v1/auth/sessions/rotate",responses((status=204)))]
pub async fn rotate(
    Extract(state): Extract<State>,
    headers: HeaderMap,
) -> Result<Response, ApiError> {
    let session = state
        .auth
        .rotate(
            cookie(&headers, "__Host-session")
                .as_deref()
                .ok_or(Error::Unauthenticated)?,
        )
        .await?;
    Ok((
        StatusCode::NO_CONTENT,
        [(
            header::SET_COOKIE,
            crate::session_cookie(session.token).to_string(),
        )],
    )
        .into_response())
}
#[utoipa::path(get,path="/v1/auth/reauth/confirmation",responses((status=200)))]
pub async fn proof_confirmation() -> Html<&'static str> {
    Html(
        r#"<!doctype html><meta charset="utf-8"><title>Privileged operation confirmation</title><p>Return this proof to the requesting client. Only its CSRF-protected privileged POST can consume it. Check the action and target in your email before proceeding.</p><button id="show">Confirm and reveal proof</button><pre id="proof"></pre><script>const proof=location.hash.slice(1);history.replaceState(null,'',location.pathname);document.getElementById('show').onclick=()=>document.getElementById('proof').textContent=proof;</script>"#,
    )
}
#[utoipa::path(get,path="/v1/invitations/confirmation",responses((status=200)))]
pub async fn invitation_confirmation(Extract(state): Extract<State>) -> Response {
    let csrf = state.secrets.token();
    let html = format!(
        r#"<!doctype html><meta charset="utf-8"><title>Accept invitation</title><p>Sign in as the invited email address first.</p><button id="accept">Accept invitation</button><pre id="result"></pre><script>const [org,id,token]=location.hash.slice(1).split(':');history.replaceState(null,'',location.pathname);document.getElementById('accept').onclick=async()=>{{const r=await fetch('/v1/organizations/'+encodeURIComponent(org)+'/invitations/'+encodeURIComponent(id)+'/accept',{{method:'POST',headers:{{'content-type':'application/json','x-csrf-token':'{csrf}'}},body:JSON.stringify({{token}})}});document.getElementById('result').textContent=r.ok?'Accepted':'Sign in or request a new invitation';}};</script>"#
    );
    (
        [(header::SET_COOKIE, crate::csrf_cookie(csrf).to_string())],
        Html(html),
    )
        .into_response()
}
