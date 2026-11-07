pub mod auth;
pub mod jobs;
pub mod mail;
mod memberships;
mod projects;
pub mod redis;
use riverqueue::{Client, JobArgs};
use serde::{Deserialize, Serialize};
use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

pub type InfraResult<T> = Result<T, Box<dyn std::error::Error + Send + Sync>>;

/// Application migrations and River migrations have independent ownership.
pub async fn migrate(pool: &PgPool) -> InfraResult<()> {
    sqlx::migrate!("../../migrations").run(pool).await?;
    riverqueue::migrate::PostgresMigrator::new(pool.clone())
        .migrate_up()
        .await?;
    Ok(())
}

#[derive(Clone)]
pub struct Database {
    pub pool: PgPool,
    pub jobs: Client,
}
impl Database {
    pub fn new(pool: PgPool) -> InfraResult<Self> {
        let jobs = Client::builder(pool.clone()).build()?;
        Ok(Self { pool, jobs })
    }
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
    pub async fn enqueue_project(
        &self,
        tx: &mut Transaction<'_, Postgres>,
        organization_id: Uuid,
        project_id: Uuid,
    ) -> InfraResult<()> {
        self.jobs
            .insert(ProjectChanged {
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
    pub organization_id: Uuid,
    pub project_id: Uuid,
}
