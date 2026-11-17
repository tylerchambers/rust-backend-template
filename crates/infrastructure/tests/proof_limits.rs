#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "Test setup and assertions must fail immediately on unexpected errors"
)]

use application::Secrets;
use application::authentication::IdentityRepository;
use application::reauthentication::ReauthenticationRepository;
#[tokio::test]
async fn concurrent_fresh_email_requests_cannot_bypass_limit() {
    let pool = sqlx::PgPool::connect(&std::env::var("TEST_DATABASE_URL").unwrap())
        .await
        .unwrap();
    infrastructure::postgres::migrations::migrate(&pool)
        .await
        .unwrap();
    let db = infrastructure::postgres::Database::new(pool.clone()).unwrap();
    let token = infrastructure::system::SystemSecrets.token();
    db.issue_login(
        &format!("proof-limit-{}@example.test", uuid::Uuid::now_v7()),
        &token,
    )
    .await
    .unwrap();
    let actor = db
        .consume_login(&token, &infrastructure::system::SystemSecrets.token())
        .await
        .unwrap();
    let organization = application::Organizations::new(
        std::sync::Arc::new(db.clone()),
        std::sync::Arc::new(infrastructure::system::SystemIds),
    )
    .create(&actor, "limits".into())
    .await
    .unwrap();
    let mut lock = pool.begin().await.unwrap();
    sqlx::query("LOCK reauthentication_proofs IN ACCESS EXCLUSIVE MODE")
        .execute(&mut *lock)
        .await
        .unwrap();
    let mut tasks = tokio::task::JoinSet::new();
    for _ in 0..8 {
        let db = db.clone();
        let actor = actor.clone();
        tasks.spawn(async move {
            db.issue_proof(
                &actor,
                "invite",
                uuid::Uuid::now_v7(),
                organization,
                &infrastructure::system::SystemSecrets.token(),
            )
            .await
        });
    }
    tokio::time::sleep(std::time::Duration::from_millis(200)).await;
    lock.commit().await.unwrap();
    let mut admitted = 0;
    while let Some(result) = tasks.join_next().await {
        if result.unwrap().is_ok() {
            admitted += 1;
        }
    }
    assert_eq!(admitted, 5);
}
