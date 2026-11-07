use application::{Actor, Organizations, Projects};
use domain::Error;
use infrastructure::Database;
use std::sync::Arc;
use uuid::Uuid;
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
        user_id,
        session_id,
    };
    let db = Arc::new(Database::new(pool.clone()).unwrap());
    let invalid = Actor {
        user_id,
        session_id: Uuid::now_v7(),
    };
    assert!(
        Organizations::new(db.clone())
            .create(&invalid, "forged".into())
            .await
            .is_err()
    );
    let org = Organizations::new(db.clone())
        .create(&actor, "Organization".into())
        .await
        .unwrap();
    let projects = Projects::new(db);
    let p = projects
        .create(&actor, org, "Project".into())
        .await
        .unwrap();
    let updated = projects
        .rename(&actor, org, p.id, 1, "Revised".into())
        .await
        .unwrap();
    assert_eq!(updated.version, 2);
    assert!(matches!(
        projects.rename(&actor, org, p.id, 1, "Stale".into()).await,
        Err(Error::Conflict)
    ));
    assert!(matches!(
        projects.list(&actor, Uuid::now_v7(), None).await,
        Err(Error::Forbidden)
    ));
    sqlx::query("UPDATE memberships SET role='viewer' WHERE organization_id=$1")
        .bind(org)
        .execute(&pool)
        .await
        .unwrap();
    assert!(matches!(
        projects.create(&actor, org, "Denied".into()).await,
        Err(Error::Forbidden)
    ));
    assert_eq!(projects.list(&actor, org, None).await.unwrap().len(), 1);
    sqlx::query("DELETE FROM memberships WHERE organization_id=$1")
        .bind(org)
        .execute(&pool)
        .await
        .unwrap();
    assert!(matches!(
        projects.list(&actor, org, None).await,
        Err(Error::Forbidden)
    ));
}
