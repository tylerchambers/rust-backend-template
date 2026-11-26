use crate::{Failure, config::Config};
use application::Authentication;
use application::Organizations;
use application::Projects;
use application::administration::Administration;
use application::invitations::Invitations;
use application::memberships::Memberships;
use infrastructure::InfraResult;
use infrastructure::postgres::Database;
use infrastructure::smtp::SmtpMailer;
use infrastructure::system::SystemClock;
use infrastructure::system::SystemIds;
use infrastructure::system::SystemSecrets;
use sqlx::{PgPool, postgres::PgPoolOptions};
use std::{sync::Arc, time::Duration};

impl Config {
    pub(crate) async fn pool(&self) -> InfraResult<PgPool> {
        Ok(PgPoolOptions::new()
            .max_connections(10)
            .acquire_timeout(Duration::from_secs(5))
            .after_connect(|connection,_metadata|Box::pin(async move {
                sqlx::raw_sql("SET statement_timeout='5s'; SET lock_timeout='3s'; SET idle_in_transaction_session_timeout='10s'").execute(connection).await?;Ok(())
            }))
            .connect(&self.database)
            .await?)
    }
    pub(crate) fn mail(&self) -> InfraResult<Arc<SmtpMailer>> {
        if self.capture {
            Ok(Arc::new(SmtpMailer::local_capture(
                "127.0.0.1",
                std::env::var("SMTP_PORT")
                    .unwrap_or_else(|_| "1025".into())
                    .parse()?,
                &self.origin,
            )?))
        } else {
            Ok(Arc::new(SmtpMailer::production(
                &std::env::var("SMTP_HOST")?,
                std::env::var("SMTP_USERNAME")?,
                std::env::var("SMTP_PASSWORD")?,
                &std::env::var("SMTP_FROM")?,
                self.origin.clone(),
            )?))
        }
    }
}
pub(crate) async fn enforce_runtime_role(pool: &PgPool) -> InfraResult<()> {
    let unsafe_role:bool=sqlx::query_scalar("SELECT rolsuper OR rolbypassrls OR EXISTS(SELECT 1 FROM pg_tables WHERE schemaname='public' AND tableowner=current_user) FROM pg_roles WHERE rolname=current_user").fetch_one(pool).await?;
    if unsafe_role {
        return Err("runtime role must be nonowner and NOBYPASSRLS".into());
    }
    Ok(())
}

/// Immutable construction result, passed directly to the server composition root.
/// Application code never looks up dependencies through this bundle.
pub(crate) struct ServerServices {
    pub(crate) db: Arc<Database>,
    pub(crate) state: api::State,
    pub(crate) redis: Option<Arc<infrastructure::redis::RedisAdapters>>,
}
impl ServerServices {
    pub(crate) async fn build(config: &Config, pool: &PgPool) -> Result<Self, Failure> {
        let db = Arc::new(Database::new(pool.clone()).map_err(|_| Failure::Dependency)?);
        let secrets = Arc::new(SystemSecrets);
        let mail = config.mail().map_err(|_| Failure::Configuration)?;
        let mut auth = Authentication::new(db.clone(), mail, secrets.clone());
        let mut projects = Projects::new(db.clone(), Arc::new(SystemIds));
        let mut redis_metrics = None;
        if let Ok(url) = std::env::var("REDIS_URL") {
            let redis = Arc::new(
                infrastructure::redis::RedisAdapters::connect(
                    &url,
                    &std::env::var("ENVIRONMENT").map_err(|_| Failure::Configuration)?,
                )
                .await
                .map_err(|_| Failure::Dependency)?,
            );
            auth = auth.with_limiter(redis.clone());
            redis_metrics = Some(redis.clone());
            projects = projects.with_cache(redis);
        }
        let state = api::State::new(
            Arc::new(auth),
            Arc::new(Organizations::new(db.clone(), Arc::new(SystemIds))),
            Arc::new(projects),
            secrets,
            config.origin.clone(),
            Arc::new(Memberships::new(db.clone())),
            Arc::new(Invitations::new(
                db.clone(),
                Arc::new(SystemIds),
                Arc::new(SystemClock),
            )),
            Arc::new(Administration::new(db.clone())),
        );
        Ok(Self {
            db,
            state,
            redis: redis_metrics,
        })
    }
}
