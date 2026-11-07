use application::{
    Actor, Authentication, Organizations, Projects, Secrets,
    memberships::{Memberships, Reauthentication},
};
mod memberships;
use axum::{
    Json, Router,
    extract::Request,
    extract::{DefaultBodyLimit, Path, State as Extract},
    http::{HeaderMap, StatusCode, header},
    middleware::{self, Next},
    response::{Html, IntoResponse, Response},
    routing::{get, patch, post},
};
use domain::Error;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use utoipa::{OpenApi, ToSchema};
use uuid::Uuid;

#[derive(Clone)]
pub struct State {
    auth: Arc<Authentication>,
    organizations: Arc<Organizations>,
    projects: Arc<Projects>,
    secrets: Arc<dyn Secrets>,
    origin: String,
    memberships: Arc<Memberships>,
    reauthentication: Arc<Reauthentication>,
}
impl State {
    pub fn new(
        auth: Arc<Authentication>,
        organizations: Arc<Organizations>,
        projects: Arc<Projects>,
        secrets: Arc<dyn Secrets>,
        origin: String,
        memberships: Arc<Memberships>,
        reauthentication: Arc<Reauthentication>,
    ) -> Self {
        Self {
            auth,
            organizations,
            projects,
            secrets,
            origin,
            memberships,
            reauthentication,
        }
    }
}

#[derive(Serialize, ToSchema)]
pub struct ErrorBody {
    code: String,
}
pub struct ApiError(Error);
impl From<Error> for ApiError {
    fn from(value: Error) -> Self {
        Self(value)
    }
}
impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let status = match self.0 {
            Error::Invalid => StatusCode::BAD_REQUEST,
            Error::Unauthenticated => StatusCode::UNAUTHORIZED,
            Error::Forbidden => StatusCode::FORBIDDEN,
            Error::NotFound => StatusCode::NOT_FOUND,
            Error::Conflict => StatusCode::CONFLICT,
            Error::RateLimited => StatusCode::TOO_MANY_REQUESTS,
            Error::Unavailable => StatusCode::SERVICE_UNAVAILABLE,
        };
        (
            status,
            Json(ErrorBody {
                code: self.0.to_string(),
            }),
        )
            .into_response()
    }
}
fn cookie<'a>(headers: &'a HeaderMap, name: &str) -> Option<&'a str> {
    headers
        .get(header::COOKIE)?
        .to_str()
        .ok()?
        .split(';')
        .filter_map(|p| p.trim().split_once('='))
        .find_map(|(k, v)| (k == name).then_some(v))
}
async fn actor(state: &State, headers: &HeaderMap) -> Result<Actor, ApiError> {
    Ok(state
        .auth
        .authenticate(cookie(headers, "__Host-session").ok_or(Error::Unauthenticated)?)
        .await?)
}
async fn security(Extract(state): Extract<State>, request: Request, next: Next) -> Response {
    if !matches!(
        *request.method(),
        axum::http::Method::GET | axum::http::Method::HEAD | axum::http::Method::OPTIONS
    ) {
        let headers = request.headers();
        let csrf = headers.get("x-csrf-token").and_then(|v| v.to_str().ok());
        if headers.get(header::ORIGIN).and_then(|v| v.to_str().ok()) != Some(state.origin.as_str())
            || csrf.is_none()
            || csrf.map(str::len) != Some(64)
            || csrf != cookie(headers, "csrf")
        {
            return ApiError(Error::Forbidden).into_response();
        }
    }
    let mut response = next.run(request).await;
    response
        .headers_mut()
        .insert(header::CACHE_CONTROL, "no-store".parse().unwrap());
    response
        .headers_mut()
        .insert("referrer-policy", "no-referrer".parse().unwrap());
    response
        .headers_mut()
        .insert("x-content-type-options", "nosniff".parse().unwrap());
    response
}
#[derive(Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct LoginRequest {
    email: String,
}
#[derive(Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct ConfirmRequest {
    token: String,
}
#[derive(Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct NameRequest {
    name: String,
}
#[derive(Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct RenameRequest {
    name: String,
    version: i64,
}
#[derive(Serialize, ToSchema)]
pub struct IdentityResponse {
    user_id: Uuid,
}
#[derive(Serialize, ToSchema)]
pub struct OrganizationResponse {
    id: Uuid,
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
            id: p.id,
            organization_id: p.organization_id,
            name: p.name,
            version: p.version,
            archived: p.archived,
        }
    }
}

#[utoipa::path(post,path="/v1/auth/request",request_body=LoginRequest,responses((status=202),(status=429,body=ErrorBody)))]
async fn request_login(
    Extract(state): Extract<State>,
    Json(input): Json<LoginRequest>,
) -> Result<StatusCode, ApiError> {
    state.auth.request(&input.email).await?;
    Ok(StatusCode::ACCEPTED)
}
#[utoipa::path(post,path="/v1/auth/confirm",request_body=ConfirmRequest,responses((status=200,body=IdentityResponse),(status=401,body=ErrorBody)))]
async fn confirm(
    Extract(state): Extract<State>,
    Json(input): Json<ConfirmRequest>,
) -> Result<Response, ApiError> {
    let session = state.auth.confirm(&input.token).await?;
    let value = format!(
        "__Host-session={}; Secure; HttpOnly; SameSite=Lax; Path=/; Max-Age=604800",
        session.token
    );
    Ok((
        [(header::SET_COOKIE, value)],
        Json(IdentityResponse {
            user_id: session.user_id,
        }),
    )
        .into_response())
}
#[utoipa::path(get,path="/v1/auth/confirmation",responses((status=200,description="Scanner-safe confirmation page; token remains in URL fragment")))]
async fn confirmation(Extract(state): Extract<State>) -> Response {
    let csrf = state.secrets.token();
    let html = format!(
        r#"<!doctype html><meta charset="utf-8"><title>Confirm sign in</title><button id="confirm">Confirm sign in</button><pre id="result"></pre><script>const token=location.hash.slice(1);history.replaceState(null,'',location.pathname);document.getElementById('confirm').onclick=async()=>{{const r=await fetch('/v1/auth/confirm',{{method:'POST',headers:{{'content-type':'application/json','x-csrf-token':'{csrf}'}},body:JSON.stringify({{token}})}});document.getElementById('result').textContent=r.ok?'Signed in':'Link invalid or expired';}};</script>"#
    );
    (
        [(
            header::SET_COOKIE,
            format!("csrf={csrf}; Secure; SameSite=Strict; Path=/"),
        )],
        Html(html),
    )
        .into_response()
}
#[utoipa::path(post,path="/v1/auth/logout",responses((status=204)))]
async fn logout(Extract(state): Extract<State>, headers: HeaderMap) -> Result<Response, ApiError> {
    state
        .auth
        .logout(cookie(&headers, "__Host-session").ok_or(Error::Unauthenticated)?)
        .await?;
    Ok((
        StatusCode::NO_CONTENT,
        [(
            header::SET_COOKIE,
            "__Host-session=; Secure; HttpOnly; SameSite=Lax; Path=/; Max-Age=0",
        )],
    )
        .into_response())
}
#[utoipa::path(post,path="/v1/organizations",request_body=NameRequest,responses((status=201,body=OrganizationResponse)))]
async fn create_org(
    Extract(state): Extract<State>,
    headers: HeaderMap,
    Json(input): Json<NameRequest>,
) -> Result<(StatusCode, Json<OrganizationResponse>), ApiError> {
    let actor = actor(&state, &headers).await?;
    let id = state.organizations.create(&actor, input.name).await?;
    Ok((StatusCode::CREATED, Json(OrganizationResponse { id })))
}
#[utoipa::path(post,path="/v1/organizations/{organization}/projects",params(("organization"=Uuid,Path)),request_body=NameRequest,responses((status=201,body=ProjectResponse)))]
async fn create_project(
    Extract(state): Extract<State>,
    Path(org): Path<Uuid>,
    headers: HeaderMap,
    Json(input): Json<NameRequest>,
) -> Result<(StatusCode, Json<ProjectResponse>), ApiError> {
    let actor = actor(&state, &headers).await?;
    Ok((
        StatusCode::CREATED,
        Json(state.projects.create(&actor, org, input.name).await?.into()),
    ))
}
#[utoipa::path(get,path="/v1/organizations/{organization}/projects",params(("organization"=Uuid,Path)),responses((status=200,body=Vec<ProjectResponse>)))]
async fn list_projects(
    Extract(state): Extract<State>,
    Path(org): Path<Uuid>,
    headers: HeaderMap,
) -> Result<Json<Vec<ProjectResponse>>, ApiError> {
    let actor = actor(&state, &headers).await?;
    Ok(Json(
        state
            .projects
            .list(&actor, org, None)
            .await?
            .into_iter()
            .map(Into::into)
            .collect(),
    ))
}
#[utoipa::path(patch,path="/v1/organizations/{organization}/projects/{project}",params(("organization"=Uuid,Path),("project"=Uuid,Path)),request_body=RenameRequest,responses((status=200,body=ProjectResponse),(status=409,body=ErrorBody)))]
async fn rename_project(
    Extract(state): Extract<State>,
    Path((org, id)): Path<(Uuid, Uuid)>,
    headers: HeaderMap,
    Json(input): Json<RenameRequest>,
) -> Result<Json<ProjectResponse>, ApiError> {
    let actor = actor(&state, &headers).await?;
    Ok(Json(
        state
            .projects
            .rename(&actor, org, id, input.version, input.name)
            .await?
            .into(),
    ))
}
#[derive(OpenApi)]
#[openapi(
    paths(
        request_login,
        confirm,
        confirmation,
        logout,
        create_org,
        create_project,
        list_projects,
        rename_project,
        memberships::request_proof,
        memberships::invite,
        memberships::accept,
        memberships::change_role,
        memberships::sessions,
        memberships::rotate,
        memberships::revoke_all,
        memberships::proof_confirmation,
        memberships::invitation_confirmation
    ),
    components(schemas(
        LoginRequest,
        ConfirmRequest,
        NameRequest,
        RenameRequest,
        IdentityResponse,
        OrganizationResponse,
        ProjectResponse,
        ErrorBody
    ))
)]
pub struct Contract;
pub fn openapi() -> String {
    Contract::openapi()
        .to_pretty_json()
        .expect("serializable contract")
}
pub fn router(state: State) -> Router {
    Router::new()
        .route("/health", get(|| async { StatusCode::OK }))
        .route(
            "/openapi.json",
            get(|| async { ([(header::CONTENT_TYPE, "application/json")], openapi()) }),
        )
        .route("/v1/auth/request", post(request_login))
        .route("/v1/auth/confirm", post(confirm))
        .route("/v1/auth/confirmation", get(confirmation))
        .route("/v1/auth/logout", post(logout))
        .route("/v1/auth/reauth/request", post(memberships::request_proof))
        .route(
            "/v1/auth/reauth/confirmation",
            get(memberships::proof_confirmation),
        )
        .route(
            "/v1/invitations/confirmation",
            get(memberships::invitation_confirmation),
        )
        .route("/v1/auth/sessions", get(memberships::sessions))
        .route("/v1/auth/sessions/rotate", post(memberships::rotate))
        .route(
            "/v1/auth/sessions/revoke-all",
            post(memberships::revoke_all),
        )
        .route(
            "/v1/organizations/{organization}/invitations",
            post(memberships::invite),
        )
        .route(
            "/v1/organizations/{organization}/invitations/{invitation}/accept",
            post(memberships::accept),
        )
        .route(
            "/v1/organizations/{organization}/members/{user}/role",
            post(memberships::change_role),
        )
        .route("/v1/organizations", post(create_org))
        .route(
            "/v1/organizations/{organization}/projects",
            get(list_projects).post(create_project),
        )
        .route(
            "/v1/organizations/{organization}/projects/{project}",
            patch(rename_project),
        )
        .layer(DefaultBodyLimit::max(16384))
        .layer(middleware::from_fn_with_state(state.clone(), security))
        .with_state(state)
}
