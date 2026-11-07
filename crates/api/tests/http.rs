#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "Test setup and assertions must fail immediately on unexpected errors"
)]

use application::{Authentication, LoginMailer, Organizations, Projects};
use application::{IdentityRepository, Secrets};
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
#[expect(
    clippy::too_many_lines,
    reason = "One ordered integration journey retains shared state and security assertions"
)]
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
            db.clone(),
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
    let login = infrastructure::auth::SystemSecrets.token();
    let session = infrastructure::auth::SystemSecrets.token();
    db.issue_login(&format!("{}@example.test", uuid::Uuid::now_v7()), &login)
        .await
        .unwrap();
    let owner = db.consume_login(&login, &session).await.unwrap();
    let org = Organizations::new(db.clone(), Arc::new(infrastructure::auth::SystemIds))
        .create(&owner, "Discoverable".into())
        .await
        .unwrap();
    for path in [
        "/v1/organizations".to_string(),
        format!("/v1/organizations/{org}/members"),
    ] {
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri(&path)
                    .header("cookie", format!("__Host-session={session}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), 200, "discovery route {path}");
        let body = axum::body::to_bytes(response.into_body(), 16384)
            .await
            .unwrap();
        let rows: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(rows.as_array().unwrap().len(), 1);
        assert_eq!(rows[0]["role"], "owner");
    }
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
    // Distinct mailboxes still consume the same trusted socket budget.
    for _ in 0..60 {
        let response = app.clone().oneshot(Request::builder().method("POST")
            .uri("/v1/auth/request").header("content-type", "application/json")
            .header("origin", "http://localhost:58039").header("cookie", format!("csrf={csrf}"))
            .header("x-csrf-token", csrf).header("x-forwarded-for", uuid::Uuid::now_v7().to_string())
            .body(Body::from(serde_json::json!({"email":format!("{}@example.test",uuid::Uuid::now_v7())}).to_string())).unwrap()).await.unwrap();
        assert_eq!(response.status(), 202);
    }
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
                    .body(Body::from(
                        serde_json::json!({"token":"f".repeat(64)}).to_string(),
                    ))
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
        .clone()
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
    let post_request = |path: &str| {
        Request::builder()
            .method("POST")
            .uri(path)
            .header("origin", "http://localhost:58039")
            .header("cookie", format!("csrf={csrf}; __Host-session={session}"))
            .header("x-csrf-token", csrf)
            .header("content-type", "application/json")
            .body(Body::from("{}"))
            .unwrap()
    };
    let limited = app
        .clone()
        .oneshot(post_request("/v1/auth/request"))
        .await
        .unwrap();
    assert_eq!(
        limited.status(),
        429,
        "initiation must share socket admission even for new addresses"
    );
    // Hold the rotated response in the test instead of delivering its cookie to
    // the browser; logout still arrives with the original cookie.
    let rotated = app
        .clone()
        .oneshot(post_request("/v1/auth/sessions/rotate"))
        .await
        .unwrap();
    assert_eq!(rotated.status(), 204);
    let replacement = rotated
        .headers()
        .get("set-cookie")
        .unwrap()
        .to_str()
        .unwrap()
        .split(';')
        .next()
        .unwrap();
    let replay = || {
        Request::builder()
            .uri("/v1/auth/sessions")
            .header("cookie", replacement)
            .body(Body::empty())
            .unwrap()
    };
    assert_eq!(
        app.clone().oneshot(replay()).await.unwrap().status(),
        200,
        "live no-logout control"
    );
    assert_eq!(
        app.clone()
            .oneshot(post_request("/v1/auth/logout"))
            .await
            .unwrap()
            .status(),
        204
    );
    assert_eq!(
        app.oneshot(replay()).await.unwrap().status(),
        401,
        "delayed replacement cookie survived logout"
    );
}
