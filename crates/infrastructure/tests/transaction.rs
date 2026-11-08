#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "Test setup and assertions must fail immediately on unexpected errors"
)]

use infrastructure::{Database, migrate};
use sqlx::postgres::PgPoolOptions;
use uuid::Uuid;

#[tokio::test]
async fn project_and_river_enqueue_share_commit_and_rollback() {
    let url = std::env::var("TEST_DATABASE_URL").expect("disposable postgres required");
    let pool = PgPoolOptions::new()
        .max_connections(2)
        .connect(&url)
        .await
        .unwrap();
    migrate(&pool).await.unwrap();
    let db = Database::new(pool.clone()).unwrap();
    let id = Uuid::now_v7();
    let mut tx = db.begin_tenant(id).await.unwrap();
    sqlx::query("INSERT INTO organizations(id,name) VALUES ($1,'test')")
        .bind(id)
        .execute(&mut *tx)
        .await
        .unwrap();
    db.enqueue_project(&mut tx, id, id).await.unwrap();
    tx.rollback().await.unwrap();
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM organizations WHERE id=$1")
            .bind(id)
            .fetch_one(&pool)
            .await
            .unwrap(),
        0
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM river_job WHERE args->>'organization_id'=$1"
        )
        .bind(id.to_string())
        .fetch_one(&pool)
        .await
        .unwrap(),
        0
    );
    let mut tx = db.begin_tenant(id).await.unwrap();
    sqlx::query("INSERT INTO organizations(id,name) VALUES ($1,'test')")
        .bind(id)
        .execute(&mut *tx)
        .await
        .unwrap();
    db.enqueue_project(&mut tx, id, id).await.unwrap();
    tx.commit().await.unwrap();
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM river_job WHERE args->>'organization_id'=$1"
        )
        .bind(id.to_string())
        .fetch_one(&pool)
        .await
        .unwrap(),
        1
    );
}
