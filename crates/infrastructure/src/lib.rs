pub mod auth;
pub mod jobs;
pub mod mail;
mod memberships;
mod projects;
pub mod redis;
mod storage;
use riverqueue::{Client, JobArgs};
use serde::{Deserialize, Serialize};
use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

pub type InfraResult<T> = Result<T, Box<dyn std::error::Error + Send + Sync>>;

/// Application migrations and River migrations have independent ownership.
/// # Errors
/// Propagates migration, checksum and database errors from the owning engines.
pub async fn migrate(pool: &PgPool) -> InfraResult<()> {
    riverqueue::migrate::PostgresMigrator::new(pool.clone())
        .migrate_up()
        .await?;
    sqlx::migrate!("../../migrations").run(pool).await?;
    Ok(())
}

#[derive(Clone)]
pub struct Database {
    pub pool: PgPool,
    pub jobs: Client,
}
impl Database {
    pub async fn ready(&self) -> bool {
        matches!(
            tokio::time::timeout(
                std::time::Duration::from_secs(2),
                sqlx::query("SELECT 1").execute(&self.pool)
            )
            .await,
            Ok(Ok(_))
        )
    }
    /// Read pool and durable queue metrics.
    ///
    /// # Errors
    /// Returns `Unavailable` when queue metrics cannot be queried.
    pub async fn operational_metrics(&self) -> Result<String, domain::Error> {
        let (queued,retry,dead):(i64,i64,i64)=sqlx::query_as("SELECT count(*) FILTER(WHERE state IN ('available','scheduled')),count(*) FILTER(WHERE state='retryable'),count(*) FILTER(WHERE state='discarded') FROM river_job").fetch_one(&self.pool).await.map_err(|_|domain::Error::Unavailable)?;
        let (lag, retries): (f64, i64) = sqlx::query_as("SELECT coalesce(max(greatest(0,extract(epoch FROM clock_timestamp()-scheduled_at))) FILTER(WHERE state IN ('available','retryable')),0)::float8, coalesce(sum(greatest(attempt-1,0)),0)::bigint FROM river_job").fetch_one(&self.pool).await.map_err(|_|domain::Error::Unavailable)?;
        Ok(format!(
            "river_queue_lag_seconds {lag}\nriver_attempt_retries {retries}\nriver_jobs_queued {queued}\nriver_jobs_retryable {retry}\nriver_jobs_discarded {dead}\ndb_pool_connections {}\ndb_pool_idle {}\n",
            self.pool.size(),
            self.pool.num_idle()
        ))
    }
    /// Construct database adapters and a River client over the supplied pool.
    ///
    /// # Errors
    /// Propagates River client configuration errors.
    pub fn new(pool: PgPool) -> InfraResult<Self> {
        let jobs = Client::builder(pool.clone()).build()?;
        Ok(Self { pool, jobs })
    }
    /// Begin a cancellation-safe transaction with transaction-local tenant context.
    /// The caller must validate actor authority before performing tenant work.
    ///
    /// # Errors
    /// Propagates database begin/context-setting errors.
    pub async fn begin_tenant(
        &self,
        organization: Uuid,
    ) -> Result<Transaction<'static, Postgres>, sqlx::Error> {
        let mut tx = riverqueue::database::begin_postgres(&self.pool).await?;
        sqlx::query("SELECT set_config('app.organization_id',$1,true)")
            .bind(organization.to_string())
            .execute(&mut *tx)
            .await?;
        Ok(tx)
    }
    /// Enqueue a project change in the caller-owned transaction.
    ///
    /// # Errors
    /// Propagates River insertion errors; the caller owns commit or rollback.
    pub async fn enqueue_project(
        &self,
        tx: &mut Transaction<'_, Postgres>,
        organization_id: Uuid,
        project_id: Uuid,
    ) -> InfraResult<()> {
        self.enqueue_project_correlated(tx, organization_id, project_id, Uuid::now_v7())
            .await
    }
    /// Enqueue a project change carrying the invocation correlation ID.
    ///
    /// # Errors
    /// Propagates River insertion errors; the caller owns commit or rollback.
    pub async fn enqueue_project_correlated(
        &self,
        tx: &mut Transaction<'_, Postgres>,
        organization_id: Uuid,
        project_id: Uuid,
        correlation_id: Uuid,
    ) -> InfraResult<()> {
        self.jobs
            .insert(ProjectChanged {
                correlation_id,
                organization_id,
                project_id,
            })
            .tx(tx)
            .await?;
        Ok(())
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, JobArgs)]
#[river(kind = "project_changed_v1")]
pub struct ProjectChanged {
    #[serde(default)]
    pub correlation_id: Uuid,
    pub organization_id: Uuid,
    pub project_id: Uuid,
}
