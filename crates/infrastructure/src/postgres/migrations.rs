use crate::InfraResult;
use sqlx::PgPool;
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
