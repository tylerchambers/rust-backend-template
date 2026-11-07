use application::{
    IdentityRepository, Organizations, Secrets,
    memberships::{Memberships, ReauthenticationRepository},
};
use domain::Role;
use infrastructure::{Database, auth::SystemSecrets};
use std::sync::Arc;
use uuid::Uuid;
#[tokio::test]
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
    let org = Organizations::new(db.clone())
        .create(&actors[0], "Invitations".into())
        .await
        .unwrap();
    let service = Memberships::new(db.clone());
    let proof = secrets.token();
    db.issue_proof(&actors[0], "invite", org, org, &proof)
        .await
        .unwrap();
    let invitation = service
        .invite(&actors[0], org, &emails[1], Role::Member, &proof)
        .await
        .unwrap();
    let token = secrets.token();
    db.prepare_invitation_delivery(org, invitation, &token)
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
}
