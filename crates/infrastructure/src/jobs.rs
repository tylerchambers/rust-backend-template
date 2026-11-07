use crate::{Database, InfraResult, ProjectChanged};
use riverqueue::{
    Client, IntervalSchedule, Job, JobArgs, PeriodicJob, PeriodicJobOpts, QueueConfig, WorkContext,
    WorkOutcome, Workers,
};
use serde::{Deserialize, Serialize};
use std::{sync::Arc, time::Duration};
use tracing::Instrument;

#[derive(Clone, Debug, Serialize, Deserialize, JobArgs)]
#[river(kind = "identity_cleanup_v1")]
pub struct Cleanup {}

#[derive(Clone, Debug, Serialize, Deserialize, JobArgs)]
#[river(kind = "invitation_email_v1")]
pub struct InvitationEmail {
    // Old enqueued payloads remain decodable during a rolling upgrade.
    #[serde(default)]
    pub correlation_id: uuid::Uuid,
    pub organization_id: uuid::Uuid,
    pub invitation_id: uuid::Uuid,
}

/// River owns execution, election, retry, scheduling and stuck-job rescue.
pub fn worker_client(
    db: Database,
    mail: Arc<dyn application::memberships::InvitationMailer>,
) -> InfraResult<Client> {
    let mut workers = Workers::new();
    let projection = db.clone();
    workers.add_fn(move |_context: WorkContext, job: Job<ProjectChanged>| {
        let db = projection.clone();
        let span = tracing::info_span!("job", request_id=%job.args.correlation_id, job_id=job.row.id, kind="project_changed_v1", attempt=job.row.attempt);
        async move {
            let mut tx = db.begin_tenant(job.args.organization_id).await.map_err(|_| domain::Error::Unavailable)?;
            sqlx::query("INSERT INTO project_index(organization_id,project_id,name,version) SELECT organization_id,id,name,version FROM projects WHERE organization_id=$1 AND id=$2 ON CONFLICT(project_id) DO UPDATE SET name=EXCLUDED.name,version=EXCLUDED.version WHERE project_index.version<=EXCLUDED.version")
                .bind(job.args.organization_id).bind(job.args.project_id)
                .execute(&mut *tx).await.map_err(|_| domain::Error::Unavailable)?;
            tx.commit().await.map_err(|_| domain::Error::Unavailable)?;
            tracing::info!(operation="job.completed");
            Ok::<_, domain::Error>(WorkOutcome::Complete)
        }.instrument(span)
    })?;
    let delivery = db.clone();
    workers.add_fn(move |_context: WorkContext, job: Job<InvitationEmail>| {
        let db = delivery.clone();
        let mail = mail.clone();
        let span = tracing::info_span!("job", request_id=%job.args.correlation_id, job_id=job.row.id, kind="invitation_email_v1", attempt=job.row.attempt);
        async move {
            use application::Secrets;
            let token = crate::auth::SystemSecrets.token();
            if let Some(email) = db.prepare_invitation_delivery(job.args.organization_id, job.args.invitation_id, &token).await? {
                mail.send_invitation(&email, job.args.organization_id.into(), job.args.invitation_id.into(), &token).await?;
            }
            tracing::info!(operation="job.completed");
            Ok::<_, domain::Error>(WorkOutcome::Complete)
        }.instrument(span)
    })?;
    let cleanup = db.clone();
    workers.add_fn(move |_context: WorkContext, job: Job<Cleanup>| {
        let db = cleanup.clone();
        let span = tracing::info_span!("job", job_id=job.row.id, kind="identity_cleanup_v1", attempt=job.row.attempt);
        async move {
            // Reviewed global path: credential garbage collection only, no tenant bypass.
            sqlx::query("DELETE FROM magic_links WHERE expires_at<clock_timestamp()-interval '1 day'").execute(&db.pool).await.map_err(|_| domain::Error::Unavailable)?;
            sqlx::query("DELETE FROM reauthentication_proofs WHERE expires_at<clock_timestamp()-interval '1 day'").execute(&db.pool).await.map_err(|_| domain::Error::Unavailable)?;
            sqlx::query("DELETE FROM auth_limits WHERE window_start<clock_timestamp()-interval '1 day'").execute(&db.pool).await.map_err(|_| domain::Error::Unavailable)?;
            tracing::info!(operation="job.completed");
            Ok::<_, domain::Error>(WorkOutcome::Complete)
        }.instrument(span)
    })?;
    Ok(Client::builder(db.pool)
        .workers(workers)
        .queue("default", QueueConfig::new(4))
        .periodic_job(PeriodicJob::with_options(
            IntervalSchedule::new(Duration::from_secs(3600))?,
            || Cleanup {},
            PeriodicJobOpts::new()
                .with_id("identity_cleanup_v1")
                .with_run_on_start(true),
        ))
        .build()?)
}
