#![allow(
    clippy::unwrap_used,
    reason = "Test setup and assertions fail immediately"
)]
use super::*;
use tower::ServiceExt;

#[tokio::test(start_paused = true)]
async fn timeout_retains_json_and_outer_transport_policy() {
    // GET /pending never touches these adapters. No socket is opened.
    let pool = sqlx::postgres::PgPoolOptions::new()
        .connect_lazy("postgres://unused:unused@127.0.0.1:1/unused")
        .unwrap();
    let db = Arc::new(infrastructure::Database::new(pool).unwrap());
    let secrets = Arc::new(infrastructure::auth::SystemSecrets);
    let ids = Arc::new(infrastructure::auth::SystemIds);
    let mail = Arc::new(
        infrastructure::mail::SmtpMailer::local_capture("127.0.0.1", 1, "http://localhost")
            .unwrap(),
    );
    let state = State::new(
        Arc::new(Authentication::new(
            db.clone(),
            mail.clone(),
            secrets.clone(),
        )),
        Arc::new(Organizations::new(db.clone(), ids.clone())),
        Arc::new(Projects::new(db.clone(), ids.clone())),
        secrets.clone(),
        "http://localhost".into(),
        Arc::new(Memberships::new(
            db.clone(),
            ids,
            Arc::new(infrastructure::auth::SystemClock),
        )),
        Arc::new(Reauthentication::new(db, mail, secrets)),
    );
    let app = transport::layers(
        Router::new()
            .route("/pending", get(std::future::pending::<Response>))
            .layer(middleware::from_fn_with_state(state.clone(), security)),
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
