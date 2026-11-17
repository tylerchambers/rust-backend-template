use crate::{
    cookies::{cookie, csrf_cookie, remove_session_cookie, session_cookie},
    error::{ApiError, ErrorBody},
    state::State,
};
use application::Actor;
use axum::{
    Json,
    extract::State as Extract,
    http::{HeaderMap, StatusCode, header},
    response::{Html, IntoResponse, Response},
};
use domain::Error;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct LoginRequest {
    email: String,
}

#[derive(Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct LoginConfirmationRequest {
    token: String,
}

#[derive(Serialize, ToSchema)]
pub struct IdentityResponse {
    user_id: Uuid,
}

#[utoipa::path(
    post,
    path="/v1/auth/request",
    request_body=LoginRequest,
    params(
        ("Origin"=String,Header,description="Exact configured public origin"),
        ("x-csrf-token"=String,Header,description="Current csrf cookie value; required with matching csrf cookie"),
    ),
    responses(
        (status="default",description="Stable JSON error envelope, including transport, authentication, authorization and availability failures",body=ErrorBody),
        (status=202),
        (status=429,body=ErrorBody),
    )
)]
pub(crate) async fn request_login(
    Extract(state): Extract<State>,
    Json(input): Json<LoginRequest>,
) -> Result<StatusCode, ApiError> {
    state.auth.request(&input.email).await?;
    Ok(StatusCode::ACCEPTED)
}

#[utoipa::path(
    post,
    path="/v1/auth/confirm",
    request_body=LoginConfirmationRequest,
    params(
        ("Origin"=String,Header,description="Exact configured public origin"),
        ("x-csrf-token"=String,Header,description="Current csrf cookie value; required with matching csrf cookie"),
    ),
    responses(
        (status="default",description="Stable JSON error envelope, including transport, authentication, authorization and availability failures",body=ErrorBody),
        (status=200,body=IdentityResponse),
        (status=401,body=ErrorBody),
    )
)]
pub(crate) async fn confirm(
    Extract(state): Extract<State>,
    Json(input): Json<LoginConfirmationRequest>,
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

#[utoipa::path(
    get,
    path="/v1/auth/confirmation",
    responses(
        (status="default",description="Stable JSON error envelope, including transport, authentication, authorization and availability failures",body=ErrorBody),
        (status=200,body=String,content_type="text/html",description="Scanner-safe page; use SDK parseAs: text"),
    )
)]
pub(crate) async fn confirmation(Extract(state): Extract<State>) -> Response {
    let csrf = state.secrets.token();
    let html = include_str!("pages/authentication.html");
    (
        [(header::SET_COOKIE, csrf_cookie(csrf).to_string())],
        Html(html),
    )
        .into_response()
}

#[utoipa::path(
    post,
    path="/v1/auth/logout",
    security(("session"=[])),
    params(
        ("Origin"=String,Header,description="Exact configured public origin"),
        ("x-csrf-token"=String,Header,description="Current csrf cookie value; required with matching csrf cookie"),
    ),
    responses(
        (status="default",description="Stable JSON error envelope, including transport, authentication, authorization and availability failures",body=ErrorBody),
        (status=204),
    )
)]
pub(crate) async fn logout(
    Extract(state): Extract<State>,
    headers: HeaderMap,
) -> Result<Response, ApiError> {
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

pub(crate) async fn actor(state: &State, headers: &HeaderMap) -> Result<Actor, ApiError> {
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
