use crate::postgres::token_hash;
use application::authentication::SecurityLimiter;
use application::projects::ProjectCache;
use async_trait::async_trait;
use domain::{Error, OrganizationId, Project};
use redis::{AsyncCommands, aio::ConnectionManager};
use std::{
    future::Future,
    sync::Arc,
    time::{Duration, Instant},
};

use prometheus_client::{
    metrics::{counter::Counter, histogram::Histogram},
    registry::Registry,
};
struct Metrics {
    requests: Counter,
    errors: Counter,
    duration: Histogram,
}
impl Default for Metrics {
    fn default() -> Self {
        Self {
            requests: Counter::default(),
            errors: Counter::default(),
            duration: Histogram::new([0.001, 0.005, 0.01, 0.025, 0.05, 0.1, 0.3, 1.0, 2.0]),
        }
    }
}
#[derive(Clone)]
pub struct RedisAdapters {
    connection: ConnectionManager,
    namespace: String,
    metrics: Arc<Metrics>,
}
impl RedisAdapters {
    /// Connect a namespaced optional cache and fail-closed limiter.
    ///
    /// # Errors
    /// Returns `Invalid` for invalid namespace/URL and `Unavailable` for connection failure or timeout.
    pub async fn connect(url: &str, environment: &str) -> Result<Self, Error> {
        if environment.is_empty()
            || !environment
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'-')
        {
            return Err(Error::Invalid);
        }
        let client = redis::Client::open(url).map_err(|_| Error::Invalid)?;
        let connection =
            tokio::time::timeout(Duration::from_secs(2), client.get_connection_manager())
                .await
                .map_err(|_| Error::Unavailable)?
                .map_err(|_| Error::Unavailable)?;
        Ok(Self {
            connection,
            namespace: format!("rust-backend-template:{environment}"),
            metrics: Arc::default(),
        })
    }
    /// Register bounded, unlabelled Redis instruments in the caller's registry.
    pub fn register_metrics(&self, registry: &mut Registry) {
        registry.register(
            "redis_requests",
            "Completed Redis operations",
            self.metrics.requests.clone(),
        );
        registry.register(
            "redis_errors",
            "Unavailable Redis operations",
            self.metrics.errors.clone(),
        );
        registry.register(
            "redis_operation_duration_seconds",
            "Redis operation latency",
            self.metrics.duration.clone(),
        );
    }
    async fn observe<T>(
        &self,
        operation: impl Future<Output = Result<T, Error>>,
    ) -> Result<T, Error> {
        let start = Instant::now();
        let result = operation.await;
        self.metrics.requests.inc();
        self.metrics.duration.observe(start.elapsed().as_secs_f64());
        if matches!(result, Err(Error::Unavailable)) {
            self.metrics.errors.inc();
        }
        result
    }
    fn key(&self, org: OrganizationId) -> String {
        format!("{}:tenant:{org}:projects:v1", self.namespace)
    }
}
#[async_trait]
impl ProjectCache for RedisAdapters {
    async fn get(&self, org: OrganizationId) -> Result<Option<Vec<Project>>, Error> {
        self.observe(async {
            let mut connection = self.connection.clone();
            let value: Option<String> =
                tokio::time::timeout(Duration::from_millis(300), connection.get(self.key(org)))
                    .await
                    .map_err(|_| Error::Unavailable)?
                    .map_err(|_| Error::Unavailable)?;
            value
                .map(|s| serde_json::from_str(&s).map_err(|_| Error::Unavailable))
                .transpose()
        })
        .await
    }
    async fn put(&self, org: OrganizationId, projects: &[Project]) -> Result<(), Error> {
        self.observe(async {
            let mut connection = self.connection.clone();
            let data = serde_json::to_string(projects).map_err(|_| Error::Unavailable)?;
            tokio::time::timeout(
                Duration::from_millis(300),
                connection.set_ex::<_, _, ()>(self.key(org), data, 30),
            )
            .await
            .map_err(|_| Error::Unavailable)?
            .map_err(|_| Error::Unavailable)
        })
        .await
    }
    async fn invalidate(&self, org: OrganizationId) -> Result<(), Error> {
        self.observe(async {
            let mut connection = self.connection.clone();
            tokio::time::timeout(
                Duration::from_millis(300),
                connection.del::<_, ()>(self.key(org)),
            )
            .await
            .map_err(|_| Error::Unavailable)?
            .map_err(|_| Error::Unavailable)
        })
        .await
    }
}
#[async_trait]
impl SecurityLimiter for RedisAdapters {
    async fn admit(&self, key: &str) -> Result<(), Error> {
        self.observe(async {
            let key = format!("{}:auth:{}", self.namespace, hex::encode(token_hash(key)));
            let mut connection = self.connection.clone();
            let count: i64 = tokio::time::timeout(Duration::from_millis(300), redis::Script::new("local n=redis.call('INCR',KEYS[1]); if n==1 then redis.call('EXPIRE',KEYS[1],900) end; return n").key(key).invoke_async(&mut connection))
                .await.map_err(|_| Error::Unavailable)?.map_err(|_| Error::Unavailable)?;
            if count > 5 { Err(Error::RateLimited) } else { Ok(()) }
        }).await
    }
}
