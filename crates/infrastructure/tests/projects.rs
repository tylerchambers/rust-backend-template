use application::{Actor, Organizations, Projects};
use domain::Error;
use infrastructure::Database;
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
#[tokio::test]
async fn project_policy_concurrency_and_organization_atomicity() {
    let pool = sqlx::PgPool::connect(&std::env::var("TEST_DATABASE_URL").unwrap())
        .await
        .unwrap();
    infrastructure::migrate(&pool).await.unwrap();
    let user_id = Uuid::now_v7();
    let session_id = Uuid::now_v7();
    sqlx::query("INSERT INTO users(id,email) VALUES($1,$2)")
        .bind(user_id)
        .bind(format!("{user_id}@example.test"))
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO sessions(id,user_id,token_hash) VALUES($1,$2,$3)")
        .bind(session_id)
        .bind(user_id)
        .bind(infrastructure::auth::token_hash(&user_id.to_string()))
        .execute(&pool)
        .await
        .unwrap();
    let actor = Actor {
        correlation_id: Uuid::now_v7(),
        user_id: user_id.into(),
        session_id: session_id.into(),
    };
    let db = Arc::new(Database::new(pool.clone()).unwrap());
    let ids = Arc::new(FixedIds {
        organization: Uuid::now_v7().into(),
        project: Uuid::now_v7().into(),
    });
    let invalid = Actor {
        correlation_id: Uuid::now_v7(),
        user_id: user_id.into(),
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
    let projects = Projects::new(db, ids.clone());
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
    sqlx::query("UPDATE memberships SET role='viewer' WHERE organization_id=$1")
        .bind(org.into_uuid())
        .execute(&pool)
        .await
        .unwrap();
    assert!(matches!(
        projects.create(&actor, org, "Denied".into()).await,
        Err(Error::Forbidden)
    ));
    assert_eq!(projects.list(&actor, org, None).await.unwrap().len(), 1);
    sqlx::query("DELETE FROM memberships WHERE organization_id=$1")
        .bind(org.into_uuid())
        .execute(&pool)
        .await
        .unwrap();
    assert!(matches!(
        projects.list(&actor, org, None).await,
        Err(Error::Forbidden)
    ));
}
