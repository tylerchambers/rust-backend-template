#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "Test setup and assertions must fail immediately on unexpected errors"
)]

use application::{
    IdentityRepository, Organizations, Secrets,
    memberships::{Memberships, ReauthenticationRepository},
};
use domain::{Error, Role};
use infrastructure::{
    Database,
    auth::{SystemClock, SystemIds, SystemSecrets},
};
use std::sync::Arc;
use uuid::Uuid;

#[tokio::test]
#[expect(
    clippy::too_many_lines,
    reason = "One ordered integration journey retains shared state and security assertions"
)]
async fn fresh_proof_rejects_live_wrong_user_session_action_target_tenant_expiry_and_replay() {
    let pool = sqlx::PgPool::connect(&std::env::var("TEST_DATABASE_URL").unwrap())
        .await
        .unwrap();
    infrastructure::migrate(&pool).await.unwrap();
    let db = Arc::new(Database::new(pool.clone()).unwrap());
    let email = format!("{}@example.test", Uuid::now_v7());
    let mut actors = Vec::new();
    for address in [
        email.clone(),
        email,
        format!("{}@example.test", Uuid::now_v7()),
    ] {
        let token = SystemSecrets.token();
        db.issue_login(&address, &token).await.unwrap();
        actors.push(
            db.consume_login(&token, &SystemSecrets.token())
                .await
                .unwrap(),
        );
    }
    let owner = &actors[0];
    let organizations = Organizations::new(db.clone(), Arc::new(SystemIds));
    let org = organizations
        .create(owner, "Proof bindings".into())
        .await
        .unwrap();
    let other = organizations
        .create(owner, "Other proof tenant".into())
        .await
        .unwrap();
    let targets = [Uuid::now_v7(), Uuid::now_v7()];
    for target in targets {
        sqlx::query("INSERT INTO users(id,email) VALUES($1,$2)")
            .bind(target)
            .bind(format!("{target}@example.test"))
            .execute(&pool)
            .await
            .unwrap();
        for organization in [org, other] {
            sqlx::query(
                "INSERT INTO memberships(organization_id,user_id,role) VALUES($1,$2,'member')",
            )
            .bind(organization.into_uuid())
            .bind(target)
            .execute(&pool)
            .await
            .unwrap();
        }
    }
    sqlx::query("INSERT INTO memberships(organization_id,user_id,role) VALUES($1,$2,'owner')")
        .bind(org.into_uuid())
        .bind(actors[2].user_id.into_uuid())
        .execute(&pool)
        .await
        .unwrap();
    let memberships = Memberships::new(db.clone(), Arc::new(SystemIds), Arc::new(SystemClock));
    let proof = SystemSecrets.token();
    db.issue_proof(owner, "change_role", targets[0], org, &proof)
        .await
        .unwrap();
    // Both alternate actors have live sessions and adequate authority, so failure must bind the proof.
    for actor in [&actors[1], &actors[2]] {
        assert!(matches!(
            memberships
                .change_role(actor, org, targets[0].into(), Role::Admin, &proof)
                .await,
            Err(Error::Forbidden)
        ));
    }
    assert!(matches!(
        memberships
            .remove(owner, org, targets[0].into(), &proof)
            .await,
        Err(Error::Forbidden)
    ));
    assert!(matches!(
        memberships
            .change_role(owner, org, targets[1].into(), Role::Admin, &proof)
            .await,
        Err(Error::Forbidden)
    ));
    assert!(matches!(
        memberships
            .change_role(owner, other, targets[0].into(), Role::Admin, &proof)
            .await,
        Err(Error::Forbidden)
    ));
    let role: String =
        sqlx::query_scalar("SELECT role FROM memberships WHERE organization_id=$1 AND user_id=$2")
            .bind(org.into_uuid())
            .bind(targets[0])
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(role, "member");
    memberships
        .change_role(owner, org, targets[0].into(), Role::Admin, &proof)
        .await
        .unwrap();
    assert!(matches!(
        memberships
            .change_role(owner, org, targets[0].into(), Role::Member, &proof)
            .await,
        Err(Error::Forbidden)
    ));
    let expired = SystemSecrets.token();
    db.issue_proof(owner, "change_role", targets[1], org, &expired)
        .await
        .unwrap();
    sqlx::query("UPDATE reauthentication_proofs SET expires_at=clock_timestamp()-interval '1 second' WHERE token_hash=$1").bind(infrastructure::auth::token_hash(&expired)).execute(&pool).await.unwrap();
    assert!(matches!(
        memberships
            .change_role(owner, org, targets[1].into(), Role::Admin, &expired)
            .await,
        Err(Error::Forbidden)
    ));
    let role: String =
        sqlx::query_scalar("SELECT role FROM memberships WHERE organization_id=$1 AND user_id=$2")
            .bind(org.into_uuid())
            .bind(targets[1])
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(role, "member");
}
