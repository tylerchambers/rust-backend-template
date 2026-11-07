use application::{
    Actor, IdentityRepository, Organizations, Secrets,
    memberships::{Memberships, ReauthenticationRepository},
};
use domain::{Error, Role};
use infrastructure::{Database, auth::SystemSecrets};
use std::sync::Arc;
use uuid::Uuid;
#[tokio::test]
async fn bound_proof_is_single_use_and_last_owner_is_guarded() {
    let pool = sqlx::PgPool::connect(&std::env::var("TEST_DATABASE_URL").unwrap())
        .await
        .unwrap();
    infrastructure::migrate(&pool).await.unwrap();
    let db = Arc::new(Database::new(pool.clone()).unwrap());
    let secret = SystemSecrets;
    let link = secret.token();
    let session = secret.token();
    db.issue_login(&format!("{}@example.test", Uuid::now_v7()), &link)
        .await
        .unwrap();
    let actor = db.consume_login(&link, &session).await.unwrap();
    let org = Organizations::new(db.clone())
        .create(&actor, "Proof tenant".into())
        .await
        .unwrap();
    let service = Memberships::new(db.clone());
    let target = Uuid::now_v7();
    sqlx::query("INSERT INTO users(id,email) VALUES($1,$2)")
        .bind(target)
        .bind(format!("{target}@example.test"))
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO memberships(organization_id,user_id,role) VALUES($1,$2,'member')")
        .bind(org)
        .bind(target)
        .execute(&pool)
        .await
        .unwrap();
    let proof = secret.token();
    db.issue_proof(&actor, "change_role", target, org, &proof)
        .await
        .unwrap();
    let wrong = Actor {
        user_id: actor.user_id,
        session_id: Uuid::now_v7(),
    };
    assert!(
        service
            .change_role(&wrong, org, target, Role::Admin, &proof)
            .await
            .is_err()
    );
    service
        .change_role(&actor, org, target, Role::Admin, &proof)
        .await
        .unwrap();
    assert!(
        service
            .change_role(&actor, org, target, Role::Member, &proof)
            .await
            .is_err()
    );
    let proof = secret.token();
    db.issue_proof(&actor, "change_role", actor.user_id, org, &proof)
        .await
        .unwrap();
    assert!(matches!(
        service
            .change_role(&actor, org, actor.user_id, Role::Member, &proof)
            .await,
        Err(Error::Conflict)
    ));
}
