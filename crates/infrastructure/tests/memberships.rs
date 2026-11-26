#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "Test setup and assertions must fail immediately on unexpected errors"
)]

use application::Actor;
use application::Organizations;
use application::Secrets;
use application::authentication::IdentityRepository;
use application::memberships::Memberships;
use domain::Error;
use infrastructure::postgres::Database;
use infrastructure::system::{SystemIds, SystemSecrets};
use std::sync::Arc;
use uuid::Uuid;
#[tokio::test]
async fn membership_changes_require_a_live_admin_and_only_real_changes_retire_sessions() {
    let pool = sqlx::PgPool::connect(&std::env::var("TEST_DATABASE_URL").unwrap())
        .await
        .unwrap();
    infrastructure::postgres::migrations::migrate(&pool)
        .await
        .unwrap();
    let db = Arc::new(Database::new(pool.clone()).unwrap());
    let secret = SystemSecrets;
    let link = secret.token();
    let session = secret.token();
    db.issue_login(&format!("{}@example.test", Uuid::now_v7()), &link)
        .await
        .unwrap();
    let actor = db.consume_login(&link, &session).await.unwrap();
    let organizations = Organizations::new(db.clone(), Arc::new(SystemIds));
    let org = organizations
        .create(&actor, "Memberships".into())
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
    sqlx::query("INSERT INTO memberships(organization_id,user_id,is_admin) VALUES($1,$2,false)")
        .bind(org.into_uuid())
        .bind(target)
        .execute(&pool)
        .await
        .unwrap();
    let target_login = secret.token();
    let target_session = secret.token();
    db.issue_login(&format!("{target}@example.test"), &target_login)
        .await
        .unwrap();
    let member = db
        .consume_login(&target_login, &target_session)
        .await
        .unwrap();
    let other_org = organizations
        .create(&member, "Other tenant".into())
        .await
        .unwrap();
    let wrong = Actor {
        correlation_id: Uuid::now_v7(),
        user_id: actor.user_id,
        session_id: member.session_id,
    };
    for (unauthorized, tenant) in [(&wrong, org), (&member, org), (&actor, other_org)] {
        assert_eq!(
            service
                .change_role(unauthorized, tenant, member.user_id, true)
                .await,
            Err(Error::Forbidden)
        );
        assert_eq!(
            service.remove(unauthorized, tenant, member.user_id).await,
            Err(Error::Forbidden)
        );
    }
    assert_eq!(
        service.remove(&actor, org, actor.user_id).await,
        Err(Error::Conflict)
    );
    assert_eq!(
        service.change_role(&actor, org, actor.user_id, false).await,
        Err(Error::Conflict)
    );
    service
        .change_role(&actor, org, member.user_id, false)
        .await
        .unwrap();
    assert!(db.authenticate(&target_session).await.is_ok());
    service
        .change_role(&actor, org, member.user_id, true)
        .await
        .unwrap();
    assert!(matches!(
        db.authenticate(&target_session).await,
        Err(Error::Unauthenticated)
    ));
    assert!(db.authenticate(&session).await.is_ok());
    assert_eq!(
        service.remove(&member, org, actor.user_id).await,
        Err(Error::Forbidden),
        "a promotion does not authorize the now-retired session"
    );
}
