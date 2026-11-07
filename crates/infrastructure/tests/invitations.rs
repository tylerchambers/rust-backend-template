#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "Test setup and assertions must fail immediately on unexpected errors"
)]

use application::{
    IdentityRepository, Organizations, Secrets,
    memberships::{Memberships, ReauthenticationRepository},
};
use domain::Role;
use infrastructure::{Database, auth::SystemSecrets};
use std::sync::Arc;
use uuid::Uuid;
struct FixedClock(chrono::DateTime<chrono::Utc>);
impl application::Clock for FixedClock {
    fn now(&self) -> chrono::DateTime<chrono::Utc> {
        self.0
    }
}
#[tokio::test]
#[expect(
    clippy::too_many_lines,
    reason = "One ordered integration journey retains shared state and security assertions"
)]
async fn concurrent_invitation_acceptance_has_one_winner_and_is_email_bound() {
    let pool = sqlx::PgPool::connect(&std::env::var("TEST_DATABASE_URL").unwrap())
        .await
        .unwrap();
    infrastructure::migrate(&pool).await.unwrap();
    let db = Arc::new(Database::new(pool).unwrap());
    let secrets = SystemSecrets;
    let mut actors = Vec::new();
    let mut emails = Vec::new();
    for _ in 0..3 {
        let email = format!("{}@example.test", Uuid::now_v7());
        let token = secrets.token();
        db.issue_login(&email, &token).await.unwrap();
        actors.push(db.consume_login(&token, &secrets.token()).await.unwrap());
        emails.push(email);
    }
    let org = Organizations::new(
        db.clone(),
        std::sync::Arc::new(infrastructure::auth::SystemIds),
    )
    .create(&actors[0], "Invitations".into())
    .await
    .unwrap();
    let fixed_now =
        chrono::DateTime::from_timestamp(chrono::Utc::now().timestamp() + 300, 0).unwrap();
    let service = Memberships::new(
        db.clone(),
        std::sync::Arc::new(infrastructure::auth::SystemIds),
        std::sync::Arc::new(FixedClock(fixed_now)),
    );
    let proof = secrets.token();
    db.issue_proof(&actors[0], "invite", org.into_uuid(), org, &proof)
        .await
        .unwrap();
    let invitation = service
        .invite(&actors[0], org, &emails[1], Role::Member, &proof)
        .await
        .unwrap();
    let expires_at: chrono::DateTime<chrono::Utc> =
        sqlx::query_scalar("SELECT expires_at FROM invitations WHERE id=$1")
            .bind(invitation.into_uuid())
            .fetch_one(&db.pool)
            .await
            .unwrap();
    assert_eq!(expires_at, fixed_now + chrono::Duration::hours(24));
    let token = secrets.token();
    db.prepare_invitation_delivery(org.into_uuid(), invitation.into_uuid(), &token)
        .await
        .unwrap();
    assert!(
        service
            .accept(&actors[2], org, invitation, &token)
            .await
            .is_err()
    );
    let (a, b) = tokio::join!(
        service.accept(&actors[1], org, invitation, &token),
        service.accept(&actors[1], org, invitation, &token)
    );
    assert_eq!(usize::from(a.is_ok()) + usize::from(b.is_ok()), 1);
    // An invitation is a join grant, not an alternate role-change operation.
    let proof = secrets.token();
    db.issue_proof(&actors[0], "invite", org.into_uuid(), org, &proof)
        .await
        .unwrap();
    let duplicate = service
        .invite(&actors[0], org, &emails[1], Role::Owner, &proof)
        .await
        .unwrap();
    let token = secrets.token();
    db.prepare_invitation_delivery(org.into_uuid(), duplicate.into_uuid(), &token)
        .await
        .unwrap();
    service
        .accept(&actors[1], org, duplicate, &token)
        .await
        .unwrap();
    let role: String =
        sqlx::query_scalar("SELECT role FROM memberships WHERE organization_id=$1 AND user_id=$2")
            .bind(org.into_uuid())
            .bind(actors[1].user_id.into_uuid())
            .fetch_one(&db.pool)
            .await
            .unwrap();
    assert_eq!(role, "member");
    assert!(
        service
            .accept(&actors[1], org, duplicate, &token)
            .await
            .is_err()
    );
    let proof = secrets.token();
    db.issue_proof(&actors[0], "invite", org.into_uuid(), org, &proof)
        .await
        .unwrap();
    let cancelled = service
        .invite(&actors[0], org, &emails[2], Role::Member, &proof)
        .await
        .unwrap();
    let proof = secrets.token();
    db.issue_proof(
        &actors[0],
        "revoke_invitation",
        cancelled.into_uuid(),
        org,
        &proof,
    )
    .await
    .unwrap();
    service
        .revoke_invitation(&actors[0], org, cancelled, &proof)
        .await
        .unwrap();
    assert!(
        db.prepare_invitation_delivery(org.into_uuid(), cancelled.into_uuid(), &secrets.token())
            .await
            .unwrap()
            .is_none()
    );
}
