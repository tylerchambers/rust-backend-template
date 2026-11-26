#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "Test setup and assertions must fail immediately on unexpected errors"
)]

use application::Actor;
use application::Organizations;
use application::authentication::IdentityRepository;
use application::{Projects, Secrets};
use domain::Error;
use infrastructure::postgres::Database;
use infrastructure::system::{SystemIds, SystemSecrets};
use std::sync::Arc;
use uuid::Uuid;
struct FixedIds {
    organization: domain::OrganizationId,
    project: domain::ProjectId,
}
impl application::IdGenerator for FixedIds {
    fn organization(&self) -> domain::OrganizationId {
        self.organization
    }
    fn project(&self) -> domain::ProjectId {
        self.project
    }
    fn invitation(&self) -> domain::InvitationId {
        Uuid::now_v7().into()
    }
}

async fn sign_in(db: &Database) -> Actor {
    let link = SystemSecrets.token();
    db.issue_login(&format!("{}@example.test", Uuid::now_v7()), &link)
        .await
        .unwrap();
    db.consume_login(&link, &SystemSecrets.token())
        .await
        .unwrap()
}

#[tokio::test]
async fn project_policy_concurrency_and_organization_atomicity() {
    let pool = sqlx::PgPool::connect(&std::env::var("TEST_DATABASE_URL").unwrap())
        .await
        .unwrap();
    infrastructure::postgres::migrations::migrate(&pool)
        .await
        .unwrap();
    let db = Arc::new(Database::new(pool.clone()).unwrap());
    let actor = sign_in(&db).await;
    let ids = Arc::new(FixedIds {
        organization: Uuid::now_v7().into(),
        project: Uuid::now_v7().into(),
    });
    let invalid = Actor {
        correlation_id: Uuid::now_v7(),
        user_id: actor.user_id,
        session_id: Uuid::now_v7().into(),
    };
    assert!(
        Organizations::new(db.clone(), ids.clone())
            .create(&invalid, "forged".into())
            .await
            .is_err()
    );
    let org = Organizations::new(db.clone(), ids.clone())
        .create(&actor, "Organization".into())
        .await
        .unwrap();
    let projects = Projects::new(db.clone(), ids.clone());
    let p = projects
        .create(&actor, org, "Project".into())
        .await
        .unwrap();
    assert_eq!(org, ids.organization);
    assert_eq!(p.id, ids.project);
    let updated = projects
        .rename(&actor, org, p.id, 1, "Revised".into())
        .await
        .unwrap();
    assert_eq!(updated.version, 2);
    let correlated: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM audit_events a JOIN river_job j ON j.args->>'project_id'=a.subject::text AND j.args->>'correlation_id'=a.correlation_id::text WHERE a.subject=$1 AND a.correlation_id IS NOT NULL)").bind(p.id.into_uuid()).fetch_one(&pool).await.unwrap();
    assert!(
        correlated,
        "audit and durable job must share request correlation"
    );
    assert!(matches!(
        projects.rename(&actor, org, p.id, 1, "Stale".into()).await,
        Err(Error::Conflict)
    ));
    assert!(matches!(
        projects.list(&actor, Uuid::now_v7().into(), None).await,
        Err(Error::Forbidden)
    ));
    let member = sign_in(&db).await;
    sqlx::query("INSERT INTO memberships(organization_id,user_id,is_admin) VALUES($1,$2,false)")
        .bind(org.into_uuid())
        .bind(member.user_id.into_uuid())
        .execute(&pool)
        .await
        .unwrap();
    let projects = Projects::new(db, Arc::new(SystemIds));
    let created = projects
        .create(&member, org, "Member project".into())
        .await
        .unwrap();
    assert_eq!(
        projects.get(&actor, org, created.id).await.unwrap().id,
        created.id
    );
    let renamed = projects
        .rename(&member, org, p.id, updated.version, "Shared".into())
        .await
        .unwrap();
    assert_eq!(renamed.version, 3);
    assert_eq!(
        projects.get(&member, org, p.id).await.unwrap().name,
        "Shared"
    );
    assert_eq!(projects.list(&member, org, None).await.unwrap().len(), 2);
    sqlx::query("DELETE FROM memberships WHERE organization_id=$1 AND user_id=$2")
        .bind(org.into_uuid())
        .bind(member.user_id.into_uuid())
        .execute(&pool)
        .await
        .unwrap();
    assert!(matches!(
        projects.list(&member, org, None).await,
        Err(Error::Forbidden)
    ));
    assert!(matches!(
        projects
            .rename(&member, org, p.id, 3, "Denied".into())
            .await,
        Err(Error::Forbidden)
    ));
}
