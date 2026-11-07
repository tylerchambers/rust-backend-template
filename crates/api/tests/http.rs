use application::{Authentication, LoginMailer, Organizations, Projects};
use async_trait::async_trait;
use axum::{body::Body, http::Request};
use domain::Error;
use std::sync::Arc;
use tower::ServiceExt;
struct Mail;
#[async_trait]
impl LoginMailer for Mail {
    async fn send_login(&self, _: &str, _: &str) -> Result<(), Error> {
        Ok(())
    }
}
#[tokio::test]
async fn scanner_get_does_not_authenticate_and_mutations_require_csrf() {
    let pool = sqlx::PgPool::connect(&std::env::var("TEST_DATABASE_URL").unwrap())
        .await
        .unwrap();
    infrastructure::migrate(&pool).await.unwrap();
    let db = Arc::new(infrastructure::Database::new(pool).unwrap());
    let secrets = Arc::new(infrastructure::auth::SystemSecrets);
    let state = api::State::new(
        Arc::new(Authentication::new(
            db.clone(),
            Arc::new(Mail),
            secrets.clone(),
        )),
        Arc::new(Organizations::new(
            db.clone(),
            std::sync::Arc::new(infrastructure::auth::SystemIds),
        )),
        Arc::new(Projects::new(
            db.clone(),
            std::sync::Arc::new(infrastructure::auth::SystemIds),
        )),
        secrets.clone(),
        "http://localhost:58039".into(),
        Arc::new(application::memberships::Memberships::new(
            db.clone(),
            std::sync::Arc::new(infrastructure::auth::SystemIds),
            std::sync::Arc::new(infrastructure::auth::SystemClock),
        )),
        Arc::new(application::memberships::Reauthentication::new(
            db,
            Arc::new(
                infrastructure::mail::SmtpMailer::local_capture(
                    "127.0.0.1",
                    51029,
                    "http://localhost:58039",
                )
                .unwrap(),
            ),
            secrets,
        )),
    );
    let app = api::router(state).layer(axum::Extension(axum::extract::ConnectInfo(
        std::net::SocketAddr::new(
            std::net::IpAddr::V6(std::net::Ipv6Addr::from(uuid::Uuid::now_v7().as_u128())),
            1234,
        ),
    )));
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/v1/auth/confirmation?token=scanner")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), 200);
    let cookie = response
        .headers()
        .get("set-cookie")
        .unwrap()
        .to_str()
        .unwrap();
    assert!(cookie.starts_with("csrf="));
    assert!(!cookie.contains("session="));
    let csrf = cookie
        .split(';')
        .next()
        .unwrap()
        .strip_prefix("csrf=")
        .unwrap();
    let malformed = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/auth/confirm")
                .header("content-type", "application/json")
                .header("origin", "http://localhost:58039")
                .header("cookie", format!("csrf={csrf}"))
                .header("x-csrf-token", csrf)
                .body(Body::from("{"))
                .unwrap(),
        )
        .await
        .unwrap();
    let body = axum::body::to_bytes(malformed.into_body(), 4096)
        .await
        .unwrap();
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&body).unwrap()["code"],
        "invalid_input"
    );
    let mut limited = false;
    for _ in 0..125 {
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/auth/confirm")
                    .header("content-type", "application/json")
                    .header("origin", "http://localhost:58039")
                    .header("cookie", format!("csrf={csrf}"))
                    .header("x-csrf-token", csrf)
                    .header("x-forwarded-for", uuid::Uuid::now_v7().to_string())
                    .body(Body::from(r#"{"token":"invalid"}"#))
                    .unwrap(),
            )
            .await
            .unwrap();
        if response.status() == 429 {
            limited = true;
            break;
        }
        assert_eq!(response.status(), 401);
    }
    assert!(
        limited,
        "confirmation guessing must be limited by socket IP, not forwarded headers"
    );
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/auth/confirm")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"token":"invalid"}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), 403);
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/v1/organizations/00000000-0000-0000-0000-000000000001/projects")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), 401);
    let response = app
        .oneshot(
            Request::builder()
                .uri("/metrics/http")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let body = axum::body::to_bytes(response.into_body(), 4096)
        .await
        .unwrap();
    let metrics = std::str::from_utf8(&body).unwrap();
    assert!(metrics.contains("http_request_duration_seconds_sum "));
    assert!(metrics.contains("authentication_rate_limited_total 1"));
}
