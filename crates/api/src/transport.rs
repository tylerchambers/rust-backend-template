use axum::{
    Router,
    body::Body,
    extract::Request,
    http::{HeaderValue, header},
    middleware::Next,
    response::Response,
};
use tower_http::{set_header::SetResponseHeaderLayer, trace::TraceLayer};
use uuid::Uuid;

pub(crate) fn layers(router: Router) -> Router {
    router
        .layer(
            TraceLayer::new_for_http()
                .make_span_with(|request: &Request| {
                    let request_id = request
                        .headers()
                        .get("x-request-id")
                        .and_then(|v| v.to_str().ok())
                        .unwrap_or("missing");
                    let route = request
                        .extensions()
                        .get::<axum::extract::MatchedPath>()
                        .map_or("unmatched", axum::extract::MatchedPath::as_str);
                    tracing::info_span!("http", request_id, route)
                })
                .on_request(())
                .on_response(
                    |response: &axum::http::Response<Body>,
                     latency: std::time::Duration,
                     _span: &tracing::Span| {
                        tracing::info!(
                            operation = "http.request",
                            status = response.status().as_u16(),
                            duration_ms = u64::try_from(latency.as_millis()).unwrap_or(u64::MAX)
                        );
                    },
                )
                .on_failure(()),
        )
        .layer(SetResponseHeaderLayer::overriding(
            header::CACHE_CONTROL,
            HeaderValue::from_static("no-store"),
        ))
        .layer(SetResponseHeaderLayer::overriding(
            header::HeaderName::from_static("referrer-policy"),
            HeaderValue::from_static("no-referrer"),
        ))
        .layer(SetResponseHeaderLayer::overriding(
            header::HeaderName::from_static("x-content-type-options"),
            HeaderValue::from_static("nosniff"),
        ))
        .layer(axum::middleware::from_fn(identify))
}

// Tower's request-ID layer preserves incoming IDs; these are deliberately replaced.
#[expect(
    clippy::expect_used,
    reason = "UUID display is always an ASCII header value"
)]
async fn identify(mut request: Request, next: Next) -> Response {
    let id = HeaderValue::from_str(&Uuid::now_v7().to_string()).expect("UUID header");
    request.headers_mut().insert("x-request-id", id.clone());
    let mut response = next.run(request).await;
    response.headers_mut().insert("x-request-id", id);
    response
}
