use super::Database;
use prometheus_client::metrics::gauge::Gauge;
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
    pub async fn collect_metrics(
        &self,
        registry: &mut prometheus_client::registry::Registry,
    ) -> Result<(), domain::Error> {
        let (queued,retry,dead):(i64,i64,i64)=sqlx::query_as("SELECT count(*) FILTER(WHERE state IN ('available','scheduled')),count(*) FILTER(WHERE state='retryable'),count(*) FILTER(WHERE state='discarded') FROM river_job").fetch_one(&self.pool).await.map_err(|_|domain::Error::Unavailable)?;
        let (lag, retries): (f64, i64) = sqlx::query_as("SELECT coalesce(max(greatest(0,extract(epoch FROM clock_timestamp()-scheduled_at))) FILTER(WHERE state IN ('available','retryable')),0)::float8, coalesce(sum(greatest(attempt-1,0)),0)::bigint FROM river_job").fetch_one(&self.pool).await.map_err(|_|domain::Error::Unavailable)?;
        for (name, help, value) in [
            (
                "river_attempt_retries",
                "Retained job retry attempts (decreases after cleanup)",
                retries,
            ),
            ("river_jobs_queued", "Available and scheduled jobs", queued),
            ("river_jobs_retryable", "Retryable jobs", retry),
            ("river_jobs_discarded", "Discarded jobs", dead),
            (
                "db_pool_connections",
                "Open pool connections",
                i64::from(self.pool.size()),
            ),
            (
                "db_pool_idle",
                "Idle pool connections",
                i64::try_from(self.pool.num_idle()).map_err(|_| domain::Error::Unavailable)?,
            ),
        ] {
            let gauge = Gauge::<i64>::default();
            gauge.set(value);
            registry.register(name, help, gauge);
        }
        let gauge = Gauge::<f64, std::sync::atomic::AtomicU64>::default();
        gauge.set(lag);
        registry.register("river_queue_lag_seconds", "Oldest runnable job lag", gauge);
        Ok(())
    }
}
