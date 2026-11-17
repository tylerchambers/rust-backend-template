use crate::{
    authentication, invitations, memberships, metrics, middleware, openapi::openapi, organizations,
    projects, reauthentication, sessions, state::State,
};
use axum::{
    Router,
    http::{StatusCode, header},
    routing::{get, patch, post},
};

pub fn router(state: State) -> Router {
    middleware::layers(routes(), state)
}

pub(crate) fn routes() -> Router<State> {
    Router::new()
        .route("/health", get(|| async { StatusCode::OK }))
        .route("/metrics/http", get(metrics::http_metrics))
        .route(
            "/openapi.json",
            get(|| async { ([(header::CONTENT_TYPE, "application/json")], openapi()) }),
        )
        .route("/v1/auth/request", post(authentication::request_login))
        .route("/v1/auth/confirm", post(authentication::confirm))
        .route("/v1/auth/confirmation", get(authentication::confirmation))
        .route("/v1/auth/logout", post(authentication::logout))
        .route(
            "/v1/auth/reauth/request",
            post(reauthentication::request_proof),
        )
        .route(
            "/v1/auth/reauth/confirmation",
            get(reauthentication::proof_confirmation),
        )
        .route(
            "/v1/invitations/confirmation",
            get(invitations::invitation_confirmation),
        )
        .route("/v1/auth/sessions", get(sessions::sessions))
        .route(
            "/v1/auth/sessions/{session}/revoke",
            post(sessions::revoke_session),
        )
        .route("/v1/auth/sessions/rotate", post(sessions::rotate))
        .route("/v1/auth/sessions/revoke-all", post(sessions::revoke_all))
        .route(
            "/v1/organizations/{organization}/invitations",
            post(invitations::invite),
        )
        .route(
            "/v1/organizations/{organization}/invitations/{invitation}/accept",
            post(invitations::accept),
        )
        .route(
            "/v1/organizations/{organization}/members/{user}/role",
            post(memberships::change_role),
        )
        .route(
            "/v1/organizations/{organization}/invitations/{invitation}/revoke",
            post(invitations::revoke_invitation),
        )
        .route(
            "/v1/organizations/{organization}/members/{user}/remove",
            post(memberships::remove_member),
        )
        .route(
            "/v1/organizations",
            get(organizations::list_organizations).post(organizations::create_org),
        )
        .route(
            "/v1/organizations/{organization}/members",
            get(memberships::list_members),
        )
        .route(
            "/v1/organizations/{organization}/projects",
            get(projects::list_projects).post(projects::create_project),
        )
        .route(
            "/v1/organizations/{organization}/projects/{project}",
            patch(projects::rename_project),
        )
}
