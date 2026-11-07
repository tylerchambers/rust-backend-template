use infrastructure::{Database, ProjectChanged};
use riverqueue::{EventKind, JobArgs};
use uuid::Uuid;
#[tokio::test]
async fn river_worker_updates_projection_and_runs_scheduled_cleanup() {
    let pool = sqlx::PgPool::connect(&std::env::var("TEST_DATABASE_URL").unwrap())
        .await
        .unwrap();
    infrastructure::migrate(&pool).await.unwrap();
    let org = Uuid::now_v7();
    let project = Uuid::now_v7();
    sqlx::query("INSERT INTO organizations(id,name) VALUES($1,'worker')")
        .bind(org)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO projects(id,organization_id,name) VALUES($1,$2,'indexed')")
        .bind(project)
        .bind(org)
        .execute(&pool)
        .await
        .unwrap();
    let db = Database::new(pool.clone()).unwrap();
    let client = infrastructure::jobs::worker_client(
        db,
        std::sync::Arc::new(
            infrastructure::mail::SmtpMailer::local_capture(
                "127.0.0.1",
                51029,
                "http://localhost:58039",
            )
            .unwrap(),
        ),
    )
    .unwrap();
    let mut events = client.subscribe(&[EventKind::JobCompleted]).unwrap();
    let mut run = client.start().unwrap();
    let queued = client
        .insert(ProjectChanged {
            organization_id: org,
            project_id: project,
        })
        .await
        .unwrap();
    tokio::time::timeout(std::time::Duration::from_secs(15), async {
        let mut projection = false;
        let mut cleanup = false;
        while !projection || !cleanup {
            let event = events.recv().await.unwrap();
            if let Some(event) = event.as_job() {
                projection |= event.job.id == queued.id();
                cleanup |= event.job.kind == infrastructure::jobs::Cleanup::KIND;
            }
        }
    })
    .await
    .unwrap();
    run.stop().await.unwrap();
    assert_eq!(
        sqlx::query_scalar::<_, String>("SELECT name FROM project_index WHERE project_id=$1")
            .bind(project)
            .fetch_one(&pool)
            .await
            .unwrap(),
        "indexed"
    );
}
