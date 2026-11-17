use sqlx::postgres::PgPoolOptions;
use std::time::Duration;
#[expect(
    clippy::print_stdout,
    reason = "The openapi CLI command writes its requested document to stdout"
)]
/// Run the migration or contract-export command.
///
/// # Errors
/// Rejects unknown commands; propagates database configuration and migration failures.
pub async fn cli() -> Result<(), Failure> {
    match std::env::args().nth(1).as_deref() {
        Some("openapi") => {
            println!("{}", api::openapi());
            Ok(())
        }
        Some("migrate") => {
            let pool = PgPoolOptions::new()
                .max_connections(1)
                .acquire_timeout(Duration::from_secs(5))
                .connect(&std::env::var("DATABASE_URL").map_err(|_| Failure::Configuration)?)
                .await
                .map_err(|_| Failure::DatabaseConnect)?;
            infrastructure::postgres::migrations::migrate(&pool)
                .await
                .map_err(|_| Failure::Migration)?;
            pool.close().await;
            Ok(())
        }
        _ => Err(Failure::InvalidCommand),
    }
}

/// Closed executable-boundary diagnostics; never retains dependency messages.
#[derive(Debug, Clone, Copy)]
pub enum Failure {
    Configuration,
    DatabaseConnect,
    RuntimeRole,
    Dependency,
    ListenerBind,
    Serve,
    WorkerStart,
    WorkerRun,
    Migration,
    InvalidCommand,
}
impl Failure {
    /// Stable safe category for structured process diagnostics.
    #[must_use]
    pub const fn category(self) -> &'static str {
        match self {
            Self::Configuration => "configuration",
            Self::DatabaseConnect => "database_connect",
            Self::RuntimeRole => "runtime_role",
            Self::Dependency => "dependency",
            Self::ListenerBind => "listener_bind",
            Self::Serve => "serve",
            Self::WorkerStart => "worker_start",
            Self::WorkerRun => "worker_run",
            Self::Migration => "migration",
            Self::InvalidCommand => "invalid_command",
        }
    }
}
