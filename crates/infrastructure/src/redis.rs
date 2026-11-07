use crate::auth::token_hash;
use application::optional::{ProjectCache, SecurityLimiter};
use async_trait::async_trait;
use domain::{Error, Project};
use redis::{AsyncCommands, aio::ConnectionManager};
use std::time::Duration;
use uuid::Uuid;
#[derive(Clone)]
pub struct RedisAdapters {
    connection: ConnectionManager,
    namespace: String,
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
        })
    }
    fn key(&self, org: Uuid) -> String {
        format!("{}:tenant:{org}:projects:v1", self.namespace)
    }
}
#[async_trait]
impl ProjectCache for RedisAdapters {
    async fn get(&self, org: Uuid) -> Result<Option<Vec<Project>>, Error> {
        let mut connection = self.connection.clone();
        let value: Option<String> =
            tokio::time::timeout(Duration::from_millis(300), connection.get(self.key(org)))
                .await
                .map_err(|_| Error::Unavailable)?
                .map_err(|_| Error::Unavailable)?;
        value
            .map(|s| serde_json::from_str(&s).map_err(|_| Error::Unavailable))
            .transpose()
    }
    async fn put(&self, org: Uuid, projects: &[Project]) -> Result<(), Error> {
        let mut connection = self.connection.clone();
        let data = serde_json::to_string(projects).map_err(|_| Error::Unavailable)?;
        tokio::time::timeout(
            Duration::from_millis(300),
            connection.set_ex::<_, _, ()>(self.key(org), data, 30),
        )
        .await
        .map_err(|_| Error::Unavailable)?
        .map_err(|_| Error::Unavailable)
    }
    async fn invalidate(&self, org: Uuid) -> Result<(), Error> {
        let mut connection = self.connection.clone();
        tokio::time::timeout(
            Duration::from_millis(300),
            connection.del::<_, ()>(self.key(org)),
        )
        .await
        .map_err(|_| Error::Unavailable)?
        .map_err(|_| Error::Unavailable)
    }
}
#[async_trait]
impl SecurityLimiter for RedisAdapters {
    async fn admit(&self, key: &str) -> Result<(), Error> {
        let key = format!("{}:auth:{}", self.namespace, hex::encode(token_hash(key)));
        let mut connection = self.connection.clone();
        let count:i64=tokio::time::timeout(Duration::from_millis(300),redis::Script::new("local n=redis.call('INCR',KEYS[1]); if n==1 then redis.call('EXPIRE',KEYS[1],900) end; return n").key(key).invoke_async(&mut connection)).await.map_err(|_|Error::Unavailable)?.map_err(|_|Error::Unavailable)?;
        if count > 5 {
            Err(Error::RateLimited)
        } else {
            Ok(())
        }
    }
}
