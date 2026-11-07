use application::{IdentityRepository, Secrets};
use domain::Error;
use infrastructure::{Database, auth::SystemSecrets};
use uuid::Uuid;
#[tokio::test]
async fn expiry_limits_and_hashed_storage_are_enforced() {
    let pool = sqlx::PgPool::connect(&std::env::var("TEST_DATABASE_URL").unwrap())
        .await
        .unwrap();
    infrastructure::migrate(&pool).await.unwrap();
    let db = Database::new(pool.clone()).unwrap();
    let secret = SystemSecrets;
    let email = format!("{}@example.test", Uuid::now_v7());
    let expired = secret.token();
    db.issue_login(&email, &expired).await.unwrap();
    sqlx::query(
        "UPDATE magic_links SET expires_at=clock_timestamp()-interval '1 second' WHERE email=$1",
    )
    .bind(&email)
    .execute(&pool)
    .await
    .unwrap();
    assert!(db.consume_login(&expired, &secret.token()).await.is_err());
    for _ in 0..4 {
        db.issue_login(&email, &secret.token()).await.unwrap();
    }
    assert!(matches!(
        db.issue_login(&email, &secret.token()).await,
        Err(Error::RateLimited)
    ));
    let email = format!("{}@example.test", Uuid::now_v7());
    let token = secret.token();
    let session = secret.token();
    db.issue_login(&email, &token).await.unwrap();
    let actor = db.consume_login(&token, &session).await.unwrap();
    let hash: Vec<u8> = sqlx::query_scalar("SELECT token_hash FROM sessions WHERE id=$1")
        .bind(actor.session_id.into_uuid())
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(hash.len(), 32);
    assert_ne!(hash, session.as_bytes());
    sqlx::query(
        "UPDATE sessions SET last_seen_at=clock_timestamp()-interval '31 minutes' WHERE id=$1",
    )
    .bind(actor.session_id.into_uuid())
    .execute(&pool)
    .await
    .unwrap();
    assert!(db.authenticate(&session).await.is_err());
    sqlx::query("UPDATE sessions SET last_seen_at=clock_timestamp(),expires_at=clock_timestamp()-interval '1 second' WHERE id=$1").bind(actor.session_id.into_uuid()).execute(&pool).await.unwrap();
    assert!(db.authenticate(&session).await.is_err());
}
