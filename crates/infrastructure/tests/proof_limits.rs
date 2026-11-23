#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "Test setup and assertions must fail immediately on unexpected errors"
)]

use application::Secrets;
use application::authentication::IdentityRepository;
use application::memberships::Memberships;
use application::reauthentication::{ProofPurpose, ReauthenticationRepository};
use domain::{Error, Role};
#[tokio::test]
async fn concurrent_requests_cap_outstanding_proofs_and_consumption_releases_one_slot() {
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
            let token = infrastructure::system::SystemSecrets.token();
            db.issue_proof(
                &actor,
                ProofPurpose::ChangeRole(actor.user_id),
                organization,
                &token,
            )
            .await?;
            Ok::<_, Error>(token)
        });
    }
    tokio::time::sleep(std::time::Duration::from_millis(200)).await;
    lock.commit().await.unwrap();
    let mut admitted = Vec::new();
    let mut limited = 0;
    while let Some(result) = tasks.join_next().await {
        match result.unwrap() {
            Ok(token) => admitted.push(token),
            Err(Error::RateLimited) => limited += 1,
            Err(error) => panic!("unexpected proof issuance error: {error:?}"),
        }
    }
    assert_eq!((admitted.len(), limited), (5, 3));

    Memberships::new(std::sync::Arc::new(db.clone()))
        .change_role(
            &actor,
            organization,
            actor.user_id,
            Role::Owner,
            &admitted[0],
        )
        .await
        .unwrap();
    db.issue_proof(
        &actor,
        ProofPurpose::Invite,
        organization,
        &infrastructure::system::SystemSecrets.token(),
    )
    .await
    .unwrap();
    assert!(matches!(
        db.issue_proof(
            &actor,
            ProofPurpose::Invite,
            organization,
            &infrastructure::system::SystemSecrets.token(),
        )
        .await,
        Err(Error::RateLimited)
    ));
}
