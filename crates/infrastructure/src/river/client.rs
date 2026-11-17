use super::{jobs::Cleanup, workers};
use crate::{InfraResult, postgres::Database};
use riverqueue::{Client, IntervalSchedule, PeriodicJob, PeriodicJobOpts, QueueConfig};
use std::{sync::Arc, time::Duration};

/// River owns execution, election, retry, scheduling and stuck-job rescue.
/// # Errors
/// Returns River worker-registration, schedule or client-configuration errors.
pub fn worker_client(
    db: Database,
    mail: Arc<dyn application::invitations::InvitationMailer>,
    secrets: Arc<dyn application::Secrets>,
) -> InfraResult<Client> {
    let workers = workers::register(db.clone(), mail, secrets)?;
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
