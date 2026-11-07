use crate::auth::token_hash;
use application::optional::{ProjectCache, SecurityLimiter};
use async_trait::async_trait;
use domain::{Error, OrganizationId, Project};
use redis::{AsyncCommands, aio::ConnectionManager};
use std::{
    future::Future,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
    time::{Duration, Instant},
};

#[derive(Default)]
struct Metrics {
    requests: AtomicU64,
    errors: AtomicU64,
    duration_micros: AtomicU64,
}
#[derive(Clone)]
pub struct RedisAdapters {
    connection: ConnectionManager,
    namespace: String,
    metrics: Arc<Metrics>,
}
impl RedisAdapters {
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
    pub fn operational_metrics(&self) -> String {
        format!(
            "redis_requests_total {}\nredis_errors_total {}\nredis_operation_duration_seconds_sum {}\n",
            self.metrics.requests.load(Ordering::Relaxed),
            self.metrics.errors.load(Ordering::Relaxed),
            self.metrics.duration_micros.load(Ordering::Relaxed) as f64 / 1_000_000.0
        )
    }
    async fn observe<T>(
        &self,
        operation: impl Future<Output = Result<T, Error>>,
    ) -> Result<T, Error> {
        let start = Instant::now();
        let result = operation.await;
        self.metrics.requests.fetch_add(1, Ordering::Relaxed);
        self.metrics.duration_micros.fetch_add(
            u64::try_from(start.elapsed().as_micros()).unwrap_or(u64::MAX),
            Ordering::Relaxed,
        );
        if matches!(result, Err(Error::Unavailable)) {
            self.metrics.errors.fetch_add(1, Ordering::Relaxed);
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
