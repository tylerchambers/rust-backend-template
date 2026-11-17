use crate::{
    Failure,
    config::Config,
    services::enforce_runtime_role,
    telemetry::{logging, shutdown},
};
use infrastructure::postgres::Database;
use std::sync::Arc;
/// Compose and run River workers until shutdown.
///
/// # Errors
/// Propagates configuration, dependency, runtime-role and worker lifecycle failures.
pub async fn worker() -> Result<(), Failure> {
    logging();
    let config = Config::load().map_err(|_| Failure::Configuration)?;
    let pool = config.pool().await.map_err(|_| Failure::DatabaseConnect)?;
    enforce_runtime_role(&pool)
        .await
        .map_err(|_| Failure::RuntimeRole)?;
    let client = infrastructure::river::client::worker_client(
        Database::new(pool.clone()).map_err(|_| Failure::Dependency)?,
        config.mail().map_err(|_| Failure::Configuration)?,
        Arc::new(infrastructure::system::SystemSecrets),
    )
    .map_err(|_| Failure::WorkerStart)?;
    let mut run = client
        .start_with_graceful_stop(shutdown())
        .map_err(|_| Failure::WorkerStart)?;
    tracing::info!(operation = "worker.ready");
    run.wait().await.map_err(|_| Failure::WorkerRun)?;
    pool.close().await;
    Ok(())
}
