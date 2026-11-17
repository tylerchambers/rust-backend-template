use crate::{
    cookies::{cookie, session_cookie},
    error::{ApiError, ErrorBody},
    pagination::UuidCursor,
    state::State,
};
use axum::{
    Json,
    extract::{Path, Query, State as Extract},
    http::{HeaderMap, StatusCode, header},
    response::{IntoResponse, Response},
};
use domain::Error;
use uuid::Uuid;

#[utoipa::path(
    get,
    path="/v1/auth/sessions",
    params(
        ("after"=Option<Uuid>,Query,description="Exclusive session ID cursor; at most 50 live owned sessions in ascending ID order. Continue with the last ID until an empty page."),
    ),
    security(("session"=[])),
    responses(
        (status="default",description="Stable JSON error envelope, including transport, authentication, authorization and availability failures",body=ErrorBody),
        (status=200,body=Vec<Uuid>),
    )
)]
pub(crate) async fn sessions(
    Extract(state): Extract<State>,
    Query(page): Query<UuidCursor>,
    headers: HeaderMap,
) -> Result<Json<Vec<Uuid>>, ApiError> {
    Ok(Json(
        state
            .auth
            .sessions(
                cookie(&headers, "__Host-session")
                    .as_deref()
                    .ok_or(Error::Unauthenticated)?,
                page.after.map(Into::into),
            )
            .await?
            .into_iter()
            .map(domain::SessionId::into_uuid)
            .collect(),
    ))
}
#[utoipa::path(
    post,
    path="/v1/auth/sessions/{session}/revoke",
    params(
        ("Origin"=String,Header,description="Exact configured public origin"),
        ("x-csrf-token"=String,Header,description="Current csrf cookie value; required with matching csrf cookie"),
        ("session"=Uuid,Path),
    ),
    security(("session"=[])),
    responses(
        (status="default",description="Stable JSON error envelope, including transport, authentication, authorization and availability failures",body=ErrorBody),
        (status=204),
        (status=404,body=ErrorBody),
    )
)]
pub(crate) async fn revoke_session(
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
#[utoipa::path(
    post,
    path="/v1/auth/sessions/revoke-all",
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
pub(crate) async fn revoke_all(
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
#[utoipa::path(
    post,
    path="/v1/auth/sessions/rotate",
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
pub(crate) async fn rotate(
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
            session_cookie(session.token).to_string(),
        )],
    )
        .into_response())
}
