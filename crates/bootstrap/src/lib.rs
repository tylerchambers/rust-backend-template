use application::{
    Authentication, Organizations, Projects,
    memberships::{Memberships, Reauthentication},
};
use infrastructure::{Database, InfraResult, auth::SystemSecrets, mail::SmtpMailer};
use sqlx::{PgPool, postgres::PgPoolOptions};
use std::{sync::Arc, time::Duration};

pub struct Config {
    database: String,
    origin: String,
    bind: String,
    capture: bool,
}
impl Config {
    /// Validate the canonical public origin and mail mode.
    ///
    /// # Errors
    /// Returns `Invalid` for unsupported or insecure configuration.
    pub fn validate(origin: &str, mail: &str, development: bool) -> Result<(), domain::Error> {
        let url = url::Url::parse(origin).map_err(|_| domain::Error::Invalid)?;
        if !matches!(mail, "smtp" | "capture")
            || !matches!(url.scheme(), "http" | "https")
            || url.origin().ascii_serialization() != origin
            || !url.username().is_empty()
            || url.password().is_some()
            || url.query().is_some()
            || url.fragment().is_some()
            || (!development && (mail == "capture" || url.scheme() != "https"))
            || (url.scheme() == "http"
                && !matches!(url.host_str(), Some("localhost" | "127.0.0.1")))
        {
            return Err(domain::Error::Invalid);
        }
        Ok(())
    }
    fn load() -> InfraResult<Self> {
        let origin = std::env::var("PUBLIC_ORIGIN")?;
        let mail = std::env::var("MAIL_MODE")?;
        Self::validate(
            &origin,
            &mail,
            std::env::var("ENVIRONMENT").as_deref() == Ok("development"),
        )?;
        Ok(Self {
            database: std::env::var("DATABASE_URL")?,
            origin,
            bind: std::env::var("BIND_ADDRESS").unwrap_or_else(|_| "127.0.0.1:8080".into()),
            capture: mail == "capture",
        })
    }
    async fn pool(&self) -> InfraResult<PgPool> {
        Ok(PgPoolOptions::new()
            .max_connections(10)
            .acquire_timeout(Duration::from_secs(5))
            .after_connect(|connection,_metadata|Box::pin(async move {
                sqlx::raw_sql("SET statement_timeout='5s'; SET lock_timeout='3s'; SET idle_in_transaction_session_timeout='10s'").execute(connection).await?;Ok(())
            }))
            .connect(&self.database)
            .await?)
    }
    fn mail(&self) -> InfraResult<Arc<SmtpMailer>> {
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
                std::env::var("SMTP_FROM")?,
                self.origin.clone(),
            )?))
        }
    }
}
fn logging() {
    let _ = tracing_subscriber::fmt()
        .json()
        .with_writer(std::io::stderr)
        .with_env_filter("bootstrap=info,api=info,infrastructure=info")
        .try_init();
}
#[expect(
    clippy::expect_used,
    reason = "Failure to install the OS shutdown handler is a fatal runtime defect"
)]
async fn shutdown() {
    #[cfg(unix)]
    {
        let mut term = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("signal handler");
        tokio::select! {_ = tokio::signal::ctrl_c()=>{},_ = term.recv()=>{}}
    }
    #[cfg(not(unix))]
    {
        let _ = tokio::signal::ctrl_c().await;
    }
}
async fn enforce_runtime_role(pool: &PgPool) -> InfraResult<()> {
    let unsafe_role:bool=sqlx::query_scalar("SELECT rolsuper OR rolbypassrls OR EXISTS(SELECT 1 FROM pg_tables WHERE schemaname='public' AND tableowner=current_user) FROM pg_roles WHERE rolname=current_user").fetch_one(pool).await?;
    if unsafe_role {
        return Err("runtime role must be nonowner and NOBYPASSRLS".into());
    }
    Ok(())
}
/// Compose and serve the HTTP application until shutdown.
///
/// # Errors
/// Propagates configuration, dependency, runtime-role, bind and serving failures.
pub async fn server() -> InfraResult<()> {
    logging();
    let config = Config::load()?;
    let pool = config.pool().await?;
    enforce_runtime_role(&pool).await?;
    let db = Arc::new(Database::new(pool.clone())?);
    let secrets = Arc::new(SystemSecrets);
    let mut auth = Authentication::new(db.clone(), config.mail()?, secrets.clone());
    let mut projects = Projects::new(
        db.clone(),
        std::sync::Arc::new(infrastructure::auth::SystemIds),
    );
    let mut redis_metrics = None;
    if let Ok(url) = std::env::var("REDIS_URL") {
        let redis = Arc::new(
            infrastructure::redis::RedisAdapters::connect(&url, &std::env::var("ENVIRONMENT")?)
                .await?,
        );
        auth = auth.with_limiter(redis.clone());
        redis_metrics = Some(redis.clone());
        projects = projects.with_cache(redis);
    }
    let state = api::State::new(
        Arc::new(auth),
        Arc::new(Organizations::new(
            db.clone(),
            std::sync::Arc::new(infrastructure::auth::SystemIds),
        )),
        Arc::new(projects),
        secrets.clone(),
        config.origin.clone(),
        Arc::new(Memberships::new(
            db.clone(),
            std::sync::Arc::new(infrastructure::auth::SystemIds),
            std::sync::Arc::new(infrastructure::auth::SystemClock),
        )),
        Arc::new(Reauthentication::new(db.clone(), config.mail()?, secrets)),
    );
    let listener = tokio::net::TcpListener::bind(config.bind).await?;
    tracing::info!(operation = "server.ready");
    let readiness = db.clone();
    let metrics = db;
    let http_metrics = state.clone();
    let router = api::router(state)
        .route(
            "/ready",
            axum::routing::get(move || {
                let db = readiness.clone();
                async move {
                    if db.ready().await {
                        axum::http::StatusCode::OK
                    } else {
                        axum::http::StatusCode::SERVICE_UNAVAILABLE
                    }
                }
            }),
        )
        .route(
            "/metrics",
            axum::routing::get(move || {
                let db = metrics.clone();
                let redis = redis_metrics.clone();
                let http = http_metrics.clone();
                async move {
                    let mut registry = prometheus_client::registry::Registry::default();
                    db.collect_metrics(&mut registry)
                        .await
                        .map_err(|_| axum::http::StatusCode::SERVICE_UNAVAILABLE)?;
                    http.register_metrics(&mut registry);
                    if let Some(redis) = redis {
                        redis.register_metrics(&mut registry);
                    }
                    let mut output = String::new();
                    prometheus_client::encoding::text::encode(&mut output, &registry)
                        .map_err(|_| axum::http::StatusCode::SERVICE_UNAVAILABLE)?;
                    Ok::<_, axum::http::StatusCode>((
                        [(
                            axum::http::header::CONTENT_TYPE,
                            "application/openmetrics-text; version=1.0.0; charset=utf-8",
                        )],
                        output,
                    ))
                }
            }),
        );
    axum::serve(
        listener,
        router.into_make_service_with_connect_info::<std::net::SocketAddr>(),
    )
    .with_graceful_shutdown(shutdown())
    .await?;
    pool.close().await;
    Ok(())
}
/// Compose and run River workers until shutdown.
///
/// # Errors
/// Propagates configuration, dependency, runtime-role and worker lifecycle failures.
pub async fn worker() -> InfraResult<()> {
    logging();
    let config = Config::load()?;
    let pool = config.pool().await?;
    enforce_runtime_role(&pool).await?;
    let client = infrastructure::jobs::worker_client(Database::new(pool.clone())?, config.mail()?)?;
    let mut run = client.start_with_graceful_stop(shutdown())?;
    tracing::info!(operation = "worker.ready");
    run.wait().await?;
    pool.close().await;
    Ok(())
}
#[expect(
    clippy::print_stdout,
    reason = "The openapi CLI command writes its requested document to stdout"
)]
/// Run the migration or contract-export command.
///
/// # Errors
/// Rejects unknown commands; propagates database configuration and migration failures.
pub async fn cli() -> InfraResult<()> {
    match std::env::args().nth(1).as_deref() {
        Some("openapi") => {
            println!("{}", api::openapi());
            Ok(())
        }
        Some("migrate") => {
            let pool = PgPoolOptions::new()
                .max_connections(1)
                .connect(&std::env::var("DATABASE_URL")?)
                .await?;
            infrastructure::migrate(&pool).await?;
            pool.close().await;
            Ok(())
        }
        _ => Err("usage: cli migrate|openapi".into()),
    }
}
