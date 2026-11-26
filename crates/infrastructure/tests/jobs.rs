#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "Test setup and assertions must fail immediately on unexpected errors"
)]

use infrastructure::postgres::Database;
use infrastructure::river::jobs::ProjectChanged;
use riverqueue::{EventKind, JobArgs};
use uuid::Uuid;
#[tokio::test]
async fn river_worker_updates_projection_and_runs_scheduled_cleanup() {
    let pool = sqlx::PgPool::connect(&std::env::var("TEST_DATABASE_URL").unwrap())
        .await
        .unwrap();
    infrastructure::postgres::migrations::migrate(&pool)
        .await
        .unwrap();
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
    let client = infrastructure::river::client::worker_client(
        db,
        std::sync::Arc::new(
            infrastructure::smtp::SmtpMailer::local_capture(
                "127.0.0.1",
                std::env::var("SMTP_PORT").unwrap().parse().unwrap(),
                "http://localhost:58039",
            )
            .unwrap(),
        ),
        std::sync::Arc::new(infrastructure::system::SystemSecrets),
    )
    .unwrap();
    let mut events = client.subscribe(&[EventKind::JobCompleted]).unwrap();
    let mut run = client.start().unwrap();
    let queued = client
        .insert(ProjectChanged {
            correlation_id: Uuid::now_v7(),
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
                cleanup |= event.job.kind == infrastructure::river::jobs::Cleanup::KIND;
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
    crash_and_recover(&pool, org, project).await;
    retries_reach_terminal_state(&pool, org).await;
}

/// Runs in a separate OS process so SIGKILL cannot execute River shutdown.
#[tokio::test]
async fn crash_worker_process() {
    if std::env::var_os("RBT_CRASH_WORKER").is_none() {
        return;
    }
    let pool = sqlx::PgPool::connect(&std::env::var("TEST_DATABASE_URL").unwrap())
        .await
        .unwrap();
    let client = infrastructure::river::client::worker_client(
        Database::new(pool).unwrap(),
        std::sync::Arc::new(
            infrastructure::smtp::SmtpMailer::local_capture(
                "127.0.0.1",
                std::env::var("SMTP_PORT").unwrap().parse().unwrap(),
                "http://localhost:58039",
            )
            .unwrap(),
        ),
        std::sync::Arc::new(infrastructure::system::SystemSecrets),
    )
    .unwrap();
    client.start().unwrap().wait().await.unwrap();
}

struct Child(std::process::Child);
impl Drop for Child {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}
fn start_child() -> Child {
    Child(
        std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--exact", "crash_worker_process", "--nocapture"])
            .env("RBT_CRASH_WORKER", "1")
            .spawn()
            .unwrap(),
    )
}
async fn crash_and_recover(pool: &sqlx::PgPool, organization_id: Uuid, project_id: Uuid) {
    let mut lock = pool.begin().await.unwrap();
    sqlx::query("LOCK TABLE project_index IN ACCESS EXCLUSIVE MODE")
        .execute(&mut *lock)
        .await
        .unwrap();
    let client = riverqueue::Client::builder(pool.clone()).build().unwrap();
    let job = client
        .insert(ProjectChanged {
            organization_id,
            project_id,
            correlation_id: Uuid::now_v7(),
        })
        .await
        .unwrap();
    let mut child = start_child();
    tokio::time::timeout(std::time::Duration::from_secs(15), async {
        loop {
            let blocked: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM pg_stat_activity WHERE cardinality(pg_blocking_pids(pid))>0 AND query LIKE 'INSERT INTO project_index%')").fetch_one(pool).await.unwrap();
            let running: bool = sqlx::query_scalar("SELECT state='running' FROM river_job WHERE id=$1").bind(job.id()).fetch_one(pool).await.unwrap();
            if blocked && running { break; }
            tokio::time::sleep(std::time::Duration::from_millis(20)).await;
        }
    }).await.unwrap();
    child.0.kill().unwrap();
    assert!(!child.0.wait().unwrap().success());
    lock.rollback().await.unwrap();
    // Advance only the crashed job's age; production rescue timing is unchanged.
    sqlx::query("UPDATE river_job SET attempted_at=clock_timestamp()-interval '2 hours' WHERE id=$1 AND state='running'").bind(job.id()).execute(pool).await.unwrap();
    let _replacement = start_child();
    tokio::time::timeout(std::time::Duration::from_secs(75), async {
        loop {
            let completed: bool =
                sqlx::query_scalar("SELECT state='completed' FROM river_job WHERE id=$1")
                    .bind(job.id())
                    .fetch_one(pool)
                    .await
                    .unwrap();
            if completed {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        }
    })
    .await
    .unwrap();
    let (attempt, rescued): (i16, bool) = sqlx::query_as("SELECT attempt, EXISTS(SELECT 1 FROM unnest(errors) e WHERE e->>'error'='Stuck job rescued by JobRescuer') FROM river_job WHERE id=$1").bind(job.id()).fetch_one(pool).await.unwrap();
    assert_eq!(attempt, 2);
    assert!(rescued);
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM project_index WHERE project_id=$1")
        .bind(project_id)
        .fetch_one(pool)
        .await
        .unwrap();
    assert_eq!(count, 1);
}

struct FailingMail;
#[async_trait::async_trait]
impl application::invitations::InvitationMailer for FailingMail {
    async fn send_invitation(
        &self,
        _: &str,
        _: domain::OrganizationId,
        _: domain::InvitationId,
        _: &str,
    ) -> Result<(), domain::Error> {
        Err(domain::Error::Unavailable)
    }
}
async fn retries_reach_terminal_state(pool: &sqlx::PgPool, organization_id: Uuid) {
    let invitation_id = Uuid::now_v7();
    sqlx::query("INSERT INTO invitations(id,organization_id,email,is_admin,token_hash,expires_at) VALUES($1,$2,'poison@example.test',false,$3,clock_timestamp()+interval '1 hour')")
        .bind(invitation_id).bind(organization_id).bind(vec![0_u8;32]).execute(pool).await.unwrap();
    let client = infrastructure::river::client::worker_client(
        Database::new(pool.clone()).unwrap(),
        std::sync::Arc::new(FailingMail),
        std::sync::Arc::new(infrastructure::system::SystemSecrets),
    )
    .unwrap();
    let job = client
        .insert(infrastructure::river::jobs::InvitationEmail {
            organization_id,
            invitation_id,
            correlation_id: Uuid::now_v7(),
        })
        .opts(riverqueue::InsertOpts::default().with_max_attempts(5))
        .await
        .unwrap();
    let mut run = client.start().unwrap();
    tokio::time::timeout(std::time::Duration::from_secs(60), async {
        loop {
            let state: String = sqlx::query_scalar("SELECT state::text FROM river_job WHERE id=$1").bind(job.id()).fetch_one(pool).await.unwrap();
            if state == "discarded" { break; }
            // Advance only this fixture's retry schedule; River chooses and executes every retry.
            if state == "retryable" {
                sqlx::query("UPDATE river_job SET scheduled_at=clock_timestamp() WHERE id=$1 AND state='retryable'").bind(job.id()).execute(pool).await.unwrap();
            }
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        }
    }).await.unwrap();
    run.stop().await.unwrap();
    let (attempts, errors): (i16, i32) =
        sqlx::query_as("SELECT attempt, cardinality(errors) FROM river_job WHERE id=$1")
            .bind(job.id())
            .fetch_one(pool)
            .await
            .unwrap();
    assert_eq!((attempts, errors), (5, 5));
}
