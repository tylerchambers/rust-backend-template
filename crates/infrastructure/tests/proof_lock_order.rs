#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "Real PostgreSQL regression setup and assertions"
)]
use application::Organizations;
use application::Secrets;
use application::authentication::IdentityRepository;
use application::memberships::MembershipUnitOfWork;
use application::reauthentication::ReauthenticationRepository;
use infrastructure::postgres::Database;
use infrastructure::system::SystemIds;
use infrastructure::system::SystemSecrets;
use std::sync::Arc;
use uuid::Uuid;

#[tokio::test]
async fn proof_issuance_and_session_retirement_do_not_deadlock() {
    let admin = sqlx::PgPool::connect(&std::env::var("TEST_DATABASE_URL").unwrap())
        .await
        .unwrap();
    infrastructure::postgres::migrations::migrate(&admin)
        .await
        .unwrap();
    let runtime =
        sqlx::PgPool::connect("postgres://template_app:local-app-test@127.0.0.1:55439/template")
            .await
            .unwrap();
    let db = Arc::new(Database::new(runtime).unwrap());
    let mut actors = Vec::new();
    for _ in 0..2 {
        let link = SystemSecrets.token();
        db.issue_login(&format!("{}@example.test", Uuid::now_v7()), &link)
            .await
            .unwrap();
        actors.push(
            db.consume_login(&link, &SystemSecrets.token())
                .await
                .unwrap(),
        );
    }
    let org = Organizations::new(db.clone(), Arc::new(SystemIds))
        .create(&actors[0], "Lock regression".into())
        .await
        .unwrap();
    sqlx::query("INSERT INTO memberships(organization_id,user_id,role) VALUES($1,$2,'member')")
        .bind(org.into_uuid())
        .bind(actors[1].user_id.into_uuid())
        .execute(&admin)
        .await
        .unwrap();
    let mut change = db.begin_memberships(org).await.unwrap();
    change.lock_organization().await.unwrap();
    change
        .lock_users(&[actors[0].user_id, actors[1].user_id])
        .await
        .unwrap();
    change.current_role(&actors[0], org).await.unwrap();
    change
        .set_role(actors[1].user_id, domain::Role::Viewer)
        .await
        .unwrap();
    let proof_db = db.clone();
    let actor = actors[1].clone();
    let issuing = tokio::spawn(async move {
        proof_db
            .issue_proof(
                &actor,
                "invite",
                org.into_uuid(),
                org,
                &SystemSecrets.token(),
            )
            .await
    });
    tokio::time::timeout(std::time::Duration::from_secs(5), async {
        loop {
            let waiting: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM pg_stat_activity WHERE usename='template_app' AND query LIKE '%SELECT id FROM users%' AND cardinality(pg_blocking_pids(pid))>0)")
                .fetch_one(&admin).await.unwrap();
            if waiting { break; }
            tokio::task::yield_now().await;
        }
    }).await.expect("proof reached the identity lock barrier");
    let retired = change.retire_sessions(actors[1].user_id).await;
    if retired.is_ok() {
        change.commit().await.unwrap();
    } else {
        drop(change);
    }
    let issued = issuing.await.unwrap();
    assert!(retired.is_ok(), "retirement aborted: {retired:?}");
    assert!(
        !matches!(issued, Err(domain::Error::Unavailable)),
        "proof aborted with storage failure: {issued:?}"
    );
}
