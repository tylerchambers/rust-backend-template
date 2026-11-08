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
async fn concurrent_owner_demotions_leave_one_owner_and_retire_only_changed_credentials() {
    let pool = sqlx::PgPool::connect(&std::env::var("TEST_DATABASE_URL").unwrap())
        .await
        .unwrap();
    infrastructure::migrate(&pool).await.unwrap();
    let db = Arc::new(Database::new(pool.clone()).unwrap());
    let mut actors = Vec::new();
    let mut tokens = Vec::new();
    for _ in 0..2 {
        let link = SystemSecrets.token();
        let token = SystemSecrets.token();
        db.issue_login(&format!("{}@example.test", Uuid::now_v7()), &link)
            .await
            .unwrap();
        actors.push(db.consume_login(&link, &token).await.unwrap());
        tokens.push(token);
    }
    let org = Organizations::new(db.clone(), Arc::new(SystemIds))
        .create(&actors[0], "Owners".into())
        .await
        .unwrap();
    sqlx::query("INSERT INTO memberships(organization_id,user_id,role) VALUES($1,$2,'owner')")
        .bind(org.into_uuid())
        .bind(actors[1].user_id.into_uuid())
        .execute(&pool)
        .await
        .unwrap();
    let proofs = [SystemSecrets.token(), SystemSecrets.token()];
    for i in 0..2 {
        db.issue_proof(
            &actors[i],
            "change_role",
            actors[i].user_id.into_uuid(),
            org,
            &proofs[i],
        )
        .await
        .unwrap();
    }
    let service = Memberships::new(db.clone(), Arc::new(SystemIds), Arc::new(SystemClock));
    let (a, b) = tokio::join!(
        service.change_role(&actors[0], org, actors[0].user_id, Role::Member, &proofs[0]),
        service.change_role(&actors[1], org, actors[1].user_id, Role::Member, &proofs[1])
    );
    assert_eq!(usize::from(a.is_ok()) + usize::from(b.is_ok()), 1);
    assert!(matches!(
        (&a, &b),
        (Ok(()), Err(Error::Conflict)) | (Err(Error::Conflict), Ok(()))
    ));
    for (i, result) in [a, b].iter().enumerate() {
        assert_eq!(db.authenticate(&tokens[i]).await.is_ok(), result.is_err());
    }
    let count: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM memberships WHERE organization_id=$1 AND role='owner'",
    )
    .bind(org.into_uuid())
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(count, 1);
}
