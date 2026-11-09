use application::{
    Actor, Authentication, Organizations, Projects, Secrets,
    memberships::{Memberships, Reauthentication},
};
mod memberships;
use axum::{
    Json, Router,
    extract::Request,
    extract::{DefaultBodyLimit, Path, Query, State as Extract},
    http::{HeaderMap, StatusCode, header},
    middleware::{self, Next},
    response::{Html, IntoResponse, Response},
    routing::{get, patch, post},
};
use domain::Error;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
mod metrics;
use metrics::HttpMetrics;
use prometheus_client::{encoding::text::encode, registry::Registry};

mod transport;
#[cfg(test)]
mod transport_tests;
use utoipa::{OpenApi, ToSchema};
use uuid::Uuid;

#[derive(Clone)]
pub struct State {
    metrics: Arc<HttpMetrics>,
    auth: Arc<Authentication>,
    organizations: Arc<Organizations>,
    projects: Arc<Projects>,
    secrets: Arc<dyn Secrets>,
    origin: String,
    memberships: Arc<Memberships>,
    reauthentication: Arc<Reauthentication>,
}
impl State {
    /// Register this application's instruments in the composition root's registry.
    pub fn register_metrics(&self, registry: &mut Registry) {
        self.metrics.register(registry);
    }

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
            metrics: Arc::new(HttpMetrics::default()),
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
mod cookies;
use cookies::{cookie, csrf_cookie, remove_session_cookie, session_cookie};
fn single_header<'a>(headers: &'a HeaderMap, name: &str) -> Option<&'a str> {
    let mut values = headers.get_all(name).iter();
    let value = values.next()?.to_str().ok()?;
    values.next().is_none().then_some(value)
}
async fn actor(state: &State, headers: &HeaderMap) -> Result<Actor, ApiError> {
    let mut actor = state
        .auth
        .authenticate(
            cookie(headers, "__Host-session")
                .as_deref()
                .ok_or(Error::Unauthenticated)?,
        )
        .await?;
    actor.correlation_id = headers
        .get("x-request-id")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.parse().ok())
        .ok_or(Error::Unavailable)?;
    Ok(actor)
}
async fn security(Extract(state): Extract<State>, request: Request, next: Next) -> Response {
    let mut csrf_denied = false;
    if !matches!(
        *request.method(),
        axum::http::Method::GET | axum::http::Method::HEAD | axum::http::Method::OPTIONS
    ) {
        let headers = request.headers();
        let csrf = single_header(headers, "x-csrf-token");
        if single_header(headers, "origin") != Some(state.origin.as_str())
            || csrf.is_none()
            || csrf.map(str::len) != Some(64)
            || csrf != cookie(headers, "csrf").as_deref()
        {
            csrf_denied = true;
        }
    }
    let route = request
        .extensions()
        .get::<axum::extract::MatchedPath>()
        .map_or("unmatched", axum::extract::MatchedPath::as_str)
        .to_owned();
    let start = std::time::Instant::now();
    let admission = if !csrf_denied
        && request.method() == axum::http::Method::POST
        && matches!(
            route.as_str(),
            "/v1/auth/request" | "/v1/auth/confirm" | "/v1/auth/reauth/request"
        ) {
        match request
            .extensions()
            .get::<axum::extract::ConnectInfo<std::net::SocketAddr>>()
        {
            Some(peer) => state.auth.admit_source(&peer.ip().to_string()).await,
            None => Err(Error::Unavailable),
        }
    } else {
        Ok(())
    };
    let mut response = if csrf_denied {
        ApiError(Error::Forbidden).into_response()
    } else if let Err(error) = admission {
        ApiError(error).into_response()
    } else {
        match tokio::time::timeout(std::time::Duration::from_secs(15), next.run(request)).await {
            Ok(response) => response,
            Err(_) => ApiError(Error::Unavailable).into_response(),
        }
    };
    if response.status().is_client_error()
        && response
            .headers()
            .get(header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            != Some("application/json")
    {
        let status = response.status();
        response = (
            status,
            Json(ErrorBody {
                code: if status == StatusCode::NOT_FOUND {
                    "not_found"
                } else {
                    "invalid_input"
                }
                .into(),
            }),
        )
            .into_response();
    }
    state.metrics.requests.inc();
    state
        .metrics
        .duration
        .observe(start.elapsed().as_secs_f64());
    if response.status() == StatusCode::TOO_MANY_REQUESTS {
        state.metrics.rate_limited.inc();
    }
    if response.status() == StatusCode::UNAUTHORIZED {
        state.metrics.authentication_denials.inc();
    }
    if response.status() == StatusCode::FORBIDDEN {
        state.metrics.authorization_denials.inc();
    }

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
            id: p.id.into_uuid(),
            organization_id: p.organization_id.into_uuid(),
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
    let value = session_cookie(session.token).to_string();
    Ok((
        [(header::SET_COOKIE, value)],
        Json(IdentityResponse {
            user_id: session.user_id.into_uuid(),
        }),
    )
        .into_response())
}
#[utoipa::path(get,path="/v1/auth/confirmation",responses((status=200,description="Scanner-safe confirmation page; token remains in URL fragment")))]
async fn confirmation(Extract(state): Extract<State>) -> Response {
    let csrf = state.secrets.token();
    let html = format!(
        r#"<!doctype html><meta charset="utf-8"><title>Confirm sign in</title><button id="confirm">Confirm sign in</button><pre id="result"></pre><script>let token='';function capture(){{if(location.hash){{token=location.hash.slice(1);history.replaceState(null,'',location.pathname);}}}}capture();addEventListener('hashchange',capture);document.getElementById('confirm').onclick=async()=>{{const r=await fetch('/v1/auth/confirm',{{method:'POST',headers:{{'content-type':'application/json','x-csrf-token':'{csrf}'}},body:JSON.stringify({{token}})}});document.getElementById('result').textContent=r.ok?'Signed in':'Link invalid or expired';}};</script>"#
    );
    (
        [(header::SET_COOKIE, crate::csrf_cookie(csrf).to_string())],
        Html(html),
    )
        .into_response()
}
#[utoipa::path(post,path="/v1/auth/logout",responses((status=204)))]
async fn logout(Extract(state): Extract<State>, headers: HeaderMap) -> Result<Response, ApiError> {
    state
        .auth
        .logout(
            cookie(&headers, "__Host-session")
                .as_deref()
                .ok_or(Error::Unauthenticated)?,
        )
        .await?;
    Ok((
        StatusCode::NO_CONTENT,
        [(header::SET_COOKIE, remove_session_cookie().to_string())],
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
    Ok((
        StatusCode::CREATED,
        Json(OrganizationResponse { id: id.into_uuid() }),
    ))
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
        Json(
            state
                .projects
                .create(&actor, org.into(), input.name)
                .await?
                .into(),
        ),
    ))
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProjectPage {
    after: Option<Uuid>,
}
#[utoipa::path(get,path="/v1/organizations/{organization}/projects",params(("organization"=Uuid,Path),("after"=Option<Uuid>,Query,description="Exclusive project ID cursor; at most 50 rows")),responses((status=200,body=Vec<ProjectResponse>)))]
async fn list_projects(
    Extract(state): Extract<State>,
    Path(org): Path<Uuid>,
    Query(page): Query<ProjectPage>,
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
            .rename(&actor, org.into(), id.into(), input.version, input.name)
            .await?
            .into(),
    ))
}
#[derive(Serialize, ToSchema)]
pub struct OrganizationSummaryResponse {
    id: Uuid,
    name: String,
    #[schema(value_type = String)]
    role: domain::Role,
}
#[derive(Serialize, ToSchema)]
pub struct MemberSummaryResponse {
    user_id: Uuid,
    #[schema(value_type = String)]
    role: domain::Role,
}
#[utoipa::path(get,path="/v1/organizations",params(("after"=Option<Uuid>,Query,description="Exclusive organization ID cursor; at most 50 rows")),responses((status=200,body=Vec<OrganizationSummaryResponse>)))]
async fn list_organizations(
    Extract(state): Extract<State>,
    Query(page): Query<ProjectPage>,
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
                role: row.role,
            })
            .collect(),
    ))
}
#[utoipa::path(get,path="/v1/organizations/{organization}/members",params(("organization"=Uuid,Path),("after"=Option<Uuid>,Query,description="Exclusive user ID cursor; at most 50 rows")),responses((status=200,body=Vec<MemberSummaryResponse>)))]
async fn list_members(
    Extract(state): Extract<State>,
    Path(org): Path<Uuid>,
    Query(page): Query<ProjectPage>,
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

#[derive(OpenApi)]
#[openapi(
    paths(
        request_login,
        confirm,
        confirmation,
        logout,
        create_org,
        list_organizations,
        list_members,
        create_project,
        list_projects,
        rename_project,
        memberships::request_proof,
        memberships::revoke_invitation,
        memberships::remove_member,
        memberships::invite,
        memberships::accept,
        memberships::change_role,
        memberships::sessions,
        memberships::rotate,
        memberships::revoke_all,
        memberships::revoke_session,
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
#[must_use]
/// Serialize the statically derived API contract.
///
/// # Panics
/// Panics if the derived contract cannot be serialized, indicating a schema defect.
#[expect(
    clippy::expect_used,
    reason = "Serialization failure of the static contract is a programmer defect"
)]
pub fn openapi() -> String {
    Contract::openapi()
        .to_pretty_json()
        .expect("serializable contract")
}
async fn http_metrics(Extract(state): Extract<State>) -> Result<Response, ApiError> {
    let mut registry = Registry::default();
    state.register_metrics(&mut registry);
    let mut output = String::new();
    encode(&mut output, &registry).map_err(|_| Error::Unavailable)?;
    Ok((
        [(
            header::CONTENT_TYPE,
            "application/openmetrics-text; version=1.0.0; charset=utf-8",
        )],
        output,
    )
        .into_response())
}
pub fn router(state: State) -> Router {
    let router = Router::new()
        .route("/health", get(|| async { StatusCode::OK }))
        .route("/metrics/http", get(http_metrics))
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
        .route(
            "/v1/auth/sessions/{session}/revoke",
            post(memberships::revoke_session),
        )
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
        .route(
            "/v1/organizations/{organization}/invitations/{invitation}/revoke",
            post(memberships::revoke_invitation),
        )
        .route(
            "/v1/organizations/{organization}/members/{user}/remove",
            post(memberships::remove_member),
        )
        .route(
            "/v1/organizations",
            get(list_organizations).post(create_org),
        )
        .route(
            "/v1/organizations/{organization}/members",
            get(list_members),
        )
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
        .with_state(state);
    transport::layers(router)
}
