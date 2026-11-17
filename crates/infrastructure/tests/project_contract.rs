#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "Test setup and assertions must fail immediately on unexpected errors"
)]

use application::Organizations;
use application::Projects;
use application::Secrets;
use application::authentication::IdentityRepository;
use infrastructure::postgres::Database;
use infrastructure::system::SystemIds;
use infrastructure::system::SystemSecrets;
use std::sync::Arc;
use uuid::Uuid;
#[path = "../../application/tests/support/project_contract.rs"]
mod contract;
#[tokio::test]
async fn postgres_obeys_shared_project_contract() {
    let pool = sqlx::PgPool::connect(&std::env::var("TEST_DATABASE_URL").unwrap())
        .await
        .unwrap();
    infrastructure::postgres::migrations::migrate(&pool)
        .await
        .unwrap();
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
