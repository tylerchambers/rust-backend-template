use application::{IdentityRepository, Organizations, Projects, Secrets};
use infrastructure::{
    Database,
    auth::{SystemIds, SystemSecrets},
};
use std::sync::Arc;
use uuid::Uuid;
#[path = "../../../tests/project_contract.rs"]
mod contract;
#[tokio::test]
async fn postgres_obeys_shared_project_contract() {
    let pool = sqlx::PgPool::connect(&std::env::var("TEST_DATABASE_URL").unwrap())
        .await
        .unwrap();
    infrastructure::migrate(&pool).await.unwrap();
    let db = Arc::new(Database::new(pool).unwrap());
    let mut actors = Vec::new();
    for _ in 0..2 {
        let link = SystemSecrets.token();
        let session = SystemSecrets.token();
        db.issue_login(&format!("{}@example.test", Uuid::now_v7()), &link)
            .await
            .unwrap();
        actors.push(db.consume_login(&link, &session).await.unwrap());
    }
    let org = Organizations::new(db.clone(), Arc::new(SystemIds))
        .create(&actors[0], "Contract".into())
        .await
        .unwrap();
    contract::project_contract(
        &Projects::new(db, Arc::new(SystemIds)),
        &actors[0],
        &actors[1],
        org,
    )
    .await;
}
