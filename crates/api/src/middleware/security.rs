use crate::{
    cookies::cookie,
    error::{ApiError, ErrorBody},
    state::State,
};
use axum::{
    Json,
    extract::{Request, State as Extract},
    http::{HeaderMap, StatusCode, header},
    middleware::Next,
    response::{IntoResponse, Response},
};
use domain::Error;

fn single_header<'a>(headers: &'a HeaderMap, name: &str) -> Option<&'a str> {
    let mut values = headers.get_all(name).iter();
    let value = values.next()?.to_str().ok()?;
    values.next().is_none().then_some(value)
}
pub(super) async fn security(
    Extract(state): Extract<State>,
    request: Request,
    next: Next,
) -> Response {
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
        && matches!(route.as_str(), "/v1/auth/request" | "/v1/auth/confirm")
    {
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
