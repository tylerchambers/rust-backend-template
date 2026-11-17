#![allow(
    clippy::unwrap_used,
    reason = "Test setup and assertions fail immediately"
)]
use crate::State;
use application::{
    Authentication, Organizations, Projects, invitations::Invitations, memberships::Memberships,
    reauthentication::Reauthentication,
};
use axum::{
    Router,
    extract::Request,
    http::{StatusCode, header},
    response::Response,
    routing::get,
};
use std::sync::Arc;
use tower::ServiceExt;
use uuid::Uuid;

#[tokio::test(start_paused = true)]
async fn timeout_retains_json_and_outer_transport_policy() {
    let state = test_state();
    let app = super::layers(
        Router::new().route("/pending", get(std::future::pending::<Response>)),
        state.clone(),
    );
    let response = app
        .oneshot(
            Request::builder()
                .uri("/pending")
                .header("x-request-id", "untrusted")
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(response.headers()[header::CONTENT_TYPE], "application/json");
    assert_eq!(response.headers()[header::CACHE_CONTROL], "no-store");
    assert_eq!(response.headers()["referrer-policy"], "no-referrer");
    assert_eq!(response.headers()["x-content-type-options"], "nosniff");
    assert_eq!(
        Uuid::parse_str(response.headers()["x-request-id"].to_str().unwrap())
            .unwrap()
            .get_version_num(),
        7
    );
    let bytes = axum::body::to_bytes(response.into_body(), 4096)
        .await
        .unwrap();
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&bytes).unwrap(),
        serde_json::json!({"code":"unavailable"})
    );
    assert_eq!(state.metrics.requests.get(), 1);
}

fn test_state() -> State {
    // Tested routes never invoke these adapters. The lazy pool opens no socket.
    let pool = sqlx::postgres::PgPoolOptions::new()
        .connect_lazy("postgres://unused:unused@127.0.0.1:1/unused")
        .unwrap();
    let db = Arc::new(infrastructure::postgres::Database::new(pool).unwrap());
    let secrets = Arc::new(infrastructure::system::SystemSecrets);
    let ids = Arc::new(infrastructure::system::SystemIds);
    let mail = Arc::new(
        infrastructure::smtp::SmtpMailer::local_capture("127.0.0.1", 1, "http://localhost")
            .unwrap(),
    );
    State::new(
        Arc::new(Authentication::new(
            db.clone(),
            mail.clone(),
            secrets.clone(),
        )),
        Arc::new(Organizations::new(db.clone(), ids.clone())),
        Arc::new(Projects::new(db.clone(), ids.clone())),
        secrets.clone(),
        "http://localhost".into(),
        Arc::new(Memberships::new(db.clone())),
        Arc::new(Invitations::new(
            db.clone(),
            ids,
            Arc::new(infrastructure::system::SystemClock),
        )),
        Arc::new(Reauthentication::new(db, mail, secrets)),
    )
}

#[tokio::test]
async fn every_documented_method_is_mounted_on_the_router() {
    // Inspect the production route table before security normalizes 405 rejections.
    let app = crate::router::routes().with_state(test_state());
    let contract: serde_json::Value = serde_json::from_str(&crate::openapi()).unwrap();
    for (path, item) in contract["paths"].as_object().unwrap() {
        let concrete_path = path
            .split('/')
            .map(|segment| {
                if segment.starts_with('{') {
                    "00000000-0000-0000-0000-000000000001"
                } else {
                    segment
                }
            })
            .collect::<Vec<_>>()
            .join("/");
        // OPTIONS discovers Axum's method table without authenticating or mutating.
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("OPTIONS")
                    .uri(concrete_path)
                    .body(axum::body::Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::METHOD_NOT_ALLOWED, "{path}");
        let allowed = response.headers()[header::ALLOW].to_str().unwrap();
        for method in item.as_object().unwrap().keys() {
            assert!(
                allowed
                    .split(',')
                    .any(|value| value.trim().eq_ignore_ascii_case(method)),
                "{method} {path}: {allowed}"
            );
        }
    }
}

#[tokio::test]
async fn confirmation_pages_render_assets_without_issuing_a_session() {
    let app = crate::router(test_state());
    for (path, expected, csrf) in [
        (
            "/v1/auth/confirmation",
            include_str!("../pages/authentication.html"),
            true,
        ),
        (
            "/v1/auth/reauth/confirmation",
            include_str!("../pages/reauthentication.html"),
            false,
        ),
        (
            "/v1/invitations/confirmation",
            include_str!("../pages/invitation.html"),
            true,
        ),
    ] {
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri(path)
                    .body(axum::body::Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK, "{path}");
        assert_eq!(
            response.headers()[header::CONTENT_TYPE],
            "text/html; charset=utf-8"
        );
        assert_eq!(response.headers()[header::CACHE_CONTROL], "no-store");
        let cookies = response
            .headers()
            .get_all(header::SET_COOKIE)
            .iter()
            .map(|value| value.to_str().unwrap())
            .collect::<Vec<_>>();
        assert!(
            !cookies
                .iter()
                .any(|value| value.starts_with("__Host-session=")),
            "{path}"
        );
        assert_eq!(
            cookies.iter().any(|value| value.starts_with("csrf=")),
            csrf,
            "{path}"
        );
        let body = axum::body::to_bytes(response.into_body(), 4096)
            .await
            .unwrap();
        assert_eq!(std::str::from_utf8(&body).unwrap(), expected, "{path}");
    }
}

#[tokio::test]
async fn login_confirmation_posts_to_the_actual_route() {
    let response = crate::router(test_state())
        .oneshot(
            Request::builder()
                .uri("/v1/auth/confirmation")
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert!(
        response.headers()[header::SET_COOKIE]
            .to_str()
            .unwrap()
            .starts_with("csrf=")
    );
    let body = axum::body::to_bytes(response.into_body(), 4096)
        .await
        .unwrap();
    let html = std::str::from_utf8(&body).unwrap();
    assert!(html.contains("fetch('/v1/auth/confirm',"));
    assert!(html.contains("getElementById('confirm')"));
    assert!(html.contains("history.replaceState(null,'',location.pathname)"));
    assert!(html.contains("document.cookie.split('; ')"));
}
