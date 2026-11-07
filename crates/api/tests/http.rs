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
        Arc::new(Organizations::new(db.clone())),
        Arc::new(Projects::new(db.clone())),
        secrets.clone(),
        "http://localhost:58039".into(),
        Arc::new(application::memberships::Memberships::new(db.clone())),
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
    let app = api::router(state);
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
        .oneshot(
            Request::builder()
                .uri("/v1/organizations/00000000-0000-0000-0000-000000000001/projects")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), 401);
}
