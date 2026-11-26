#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "Test setup and assertions must fail immediately on unexpected errors"
)]

use application::authentication::IdentityRepository;
use application::invitations::Invitations;
use application::{Actor, Organizations, Secrets};
use domain::{Error, InvitationId, OrganizationId};
use infrastructure::postgres::Database;
use infrastructure::system::{SystemIds, SystemSecrets};
use std::sync::Arc;
use uuid::Uuid;
struct FixedClock(chrono::DateTime<chrono::Utc>);
impl application::Clock for FixedClock {
    fn now(&self) -> chrono::DateTime<chrono::Utc> {
        self.0
    }
}

async fn login(db: &Database) -> (Actor, String) {
    let email = format!("{}@example.test", Uuid::now_v7());
    let link = SystemSecrets.token();
    db.issue_login(&email, &link).await.unwrap();
    let actor = db
        .consume_login(&link, &SystemSecrets.token())
        .await
        .unwrap();
    (actor, email)
}

async fn deliver(db: &Database, org: OrganizationId, id: InvitationId) -> String {
    let token = SystemSecrets.token();
    assert!(
        db.prepare_invitation_delivery(org.into_uuid(), id.into_uuid(), &token)
            .await
            .unwrap()
            .is_some()
    );
    token
}

#[tokio::test]
async fn concurrent_invitation_acceptance_is_mailbox_request_and_token_bound() {
    let pool = sqlx::PgPool::connect(&std::env::var("TEST_DATABASE_URL").unwrap())
        .await
        .unwrap();
    infrastructure::postgres::migrations::migrate(&pool)
        .await
        .unwrap();
    let db = Arc::new(Database::new(pool).unwrap());
    let (admin, _) = login(&db).await;
    let (member, email) = login(&db).await;
    let organizations = Organizations::new(db.clone(), Arc::new(SystemIds));
    let org = organizations
        .create(&admin, "Invitations".into())
        .await
        .unwrap();
    let fixed_now =
        chrono::DateTime::from_timestamp(chrono::Utc::now().timestamp() + 300, 0).unwrap();
    let service = Invitations::new(
        db.clone(),
        Arc::new(SystemIds),
        Arc::new(FixedClock(fixed_now)),
    );
    let invitation = service
        .invite(
            &admin,
            org,
            &format!(" {} ", email.to_ascii_uppercase()),
            false,
        )
        .await
        .unwrap();
    let expires_at: chrono::DateTime<chrono::Utc> =
        sqlx::query_scalar("SELECT expires_at FROM invitations WHERE id=$1")
            .bind(invitation.into_uuid())
            .fetch_one(&db.pool)
            .await
            .unwrap();
    assert_eq!(expires_at, fixed_now + chrono::Duration::hours(24));
    let token = deliver(&db, org, invitation).await;
    let other_org = organizations
        .create(&admin, "Elsewhere".into())
        .await
        .unwrap();
    let wrong_token = SystemSecrets.token();
    for (actor, requested_org, id, secret) in [
        (&admin, org, invitation, &token),
        (&member, other_org, invitation, &token),
        (&member, org, Uuid::now_v7().into(), &token),
        (&member, org, invitation, &wrong_token),
    ] {
        assert_eq!(
            service.accept(actor, requested_org, id, secret).await,
            Err(Error::Forbidden)
        );
    }
    let (a, b) = tokio::join!(
        service.accept(&member, org, invitation, &token),
        service.accept(&member, org, invitation, &token)
    );
    assert!(matches!(
        (a, b),
        (Ok(()), Err(Error::Forbidden)) | (Err(Error::Forbidden), Ok(()))
    ));

    // Accepting a second invitation must not elevate an existing member.
    let duplicate = service.invite(&admin, org, &email, true).await.unwrap();
    let token = deliver(&db, org, duplicate).await;
    service
        .accept(&member, org, duplicate, &token)
        .await
        .unwrap();
    let is_admin: bool = sqlx::query_scalar(
        "SELECT is_admin FROM memberships WHERE organization_id=$1 AND user_id=$2",
    )
    .bind(org.into_uuid())
    .bind(member.user_id.into_uuid())
    .fetch_one(&db.pool)
    .await
    .unwrap();
    assert!(!is_admin);

    assert_invitation_controls(&db, &service, org, &admin, &member, &email).await;
}

async fn assert_invitation_controls(
    db: &Database,
    service: &Invitations,
    org: OrganizationId,
    admin: &Actor,
    member: &Actor,
    member_email: &str,
) {
    let (other, other_email) = login(db).await;
    assert_eq!(
        service.invite(member, org, &other_email, true).await,
        Err(Error::Forbidden)
    );

    let grant = service
        .invite(admin, org, &other_email, true)
        .await
        .unwrap();
    assert_eq!(
        service.revoke_invitation(member, org, grant).await,
        Err(Error::Forbidden)
    );
    let token = deliver(db, org, grant).await;
    service.accept(&other, org, grant, &token).await.unwrap();

    // An invited admin can perform administrative actions immediately.
    let cancelled = service
        .invite(&other, org, member_email, false)
        .await
        .unwrap();
    let token = deliver(db, org, cancelled).await;
    service
        .revoke_invitation(&other, org, cancelled)
        .await
        .unwrap();
    assert_eq!(
        service.accept(member, org, cancelled, &token).await,
        Err(Error::Forbidden)
    );
    assert!(
        db.prepare_invitation_delivery(
            org.into_uuid(),
            cancelled.into_uuid(),
            &SystemSecrets.token()
        )
        .await
        .unwrap()
        .is_none()
    );
}
