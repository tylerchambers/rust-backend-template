use crate::InfraResult;
use riverqueue::Client;
use sqlx::PgPool;

#[derive(Clone)]
pub struct Database {
    pub pool: PgPool,
    pub jobs: Client,
}
impl Database {
    /// Construct database adapters and a River client over the supplied pool.
    ///
    /// # Errors
    /// Propagates River client configuration errors.
    pub fn new(pool: PgPool) -> InfraResult<Self> {
        let jobs = Client::builder(pool.clone()).build()?;
        Ok(Self { pool, jobs })
    }
}
