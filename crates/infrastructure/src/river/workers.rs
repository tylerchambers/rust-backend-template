use super::jobs::{Cleanup, InvitationEmail, ProjectChanged};
use crate::{
    InfraResult,
    postgres::{Database, invitations::deliver_invitation},
};
use riverqueue::{Job, WorkContext, WorkOutcome, Workers};
use std::sync::Arc;
use tracing::Instrument;

pub(super) fn register(
    db: Database,
    mail: Arc<dyn application::invitations::InvitationMailer>,
    secrets: Arc<dyn application::Secrets>,
) -> InfraResult<Workers> {
    let mut workers = Workers::new();
    let projection = db.clone();
    workers.add_fn(move |_context: WorkContext, job: Job<ProjectChanged>| {
        let db = projection.clone();
        let span = tracing::info_span!("job", request_id=%job.args.correlation_id, job_id=job.row.id, kind="project_changed_v1", attempt=job.row.attempt);
        async move {
            db.refresh_project_index(job.args.organization_id, job.args.project_id).await?;
            tracing::info!(operation="job.completed");
            Ok::<_, domain::Error>(WorkOutcome::Complete)
        }.instrument(span)
    })?;
    let delivery = db.clone();
    workers.add_fn(move |_context: WorkContext, job: Job<InvitationEmail>| {
        let db = delivery.clone();
        let mail = mail.clone();
        let secrets = secrets.clone();
        let span = tracing::info_span!("job", request_id=%job.args.correlation_id, job_id=job.row.id, kind="invitation_email_v1", attempt=job.row.attempt);
        async move {
            deliver_invitation(&db, secrets.as_ref(), mail.as_ref(), job.args.organization_id, job.args.invitation_id).await?;
            tracing::info!(operation="job.completed");
            Ok::<_, domain::Error>(WorkOutcome::Complete)
        }.instrument(span)
    })?;
    workers.add_fn(move |_context: WorkContext, job: Job<Cleanup>| {
        let db = db.clone();
        let span = tracing::info_span!(
            "job",
            job_id = job.row.id,
            kind = "identity_cleanup_v1",
            attempt = job.row.attempt
        );
        async move {
            db.cleanup_identity_records().await?;
            tracing::info!(operation = "job.completed");
            Ok::<_, domain::Error>(WorkOutcome::Complete)
        }
        .instrument(span)
    })?;
    Ok(workers)
}
