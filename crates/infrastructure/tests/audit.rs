#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "Test setup and assertions must fail immediately on unexpected errors"
)]

use application::Secrets;
use application::authentication::IdentityRepository;
#[tokio::test]
async fn identity_lifecycle_is_audited_without_credentials() {
    let pool = sqlx::PgPool::connect(&std::env::var("TEST_DATABASE_URL").unwrap())
        .await
        .unwrap();
    infrastructure::postgres::migrations::migrate(&pool)
        .await
        .unwrap();
    let db = infrastructure::postgres::Database::new(pool.clone()).unwrap();
    let email = format!("audit-{}@example.test", uuid::Uuid::now_v7());
    let login = infrastructure::system::SystemSecrets.token();
    let session = infrastructure::system::SystemSecrets.token();
    db.issue_login(&email, &login).await.unwrap();
    let actor = db.consume_login(&login, &session).await.unwrap();
    db.revoke(&session).await.unwrap();
    let events: Vec<String> = sqlx::query_scalar(
        "SELECT action FROM identity_audit WHERE user_id=$1 ORDER BY created_at",
    )
    .bind(actor.user_id.into_uuid())
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(
        events,
        vec!["identity.created", "session.created", "session.revoked"]
    );
    let stored: String =
        sqlx::query_scalar("SELECT json_agg(a)::text FROM identity_audit a WHERE user_id=$1")
            .bind(actor.user_id.into_uuid())
            .fetch_one(&pool)
            .await
            .unwrap();
    assert!(!stored.contains(&email));
    assert!(!stored.contains(&session));
    assert!(!stored.contains(&login));
}
