use async_trait::async_trait;
use domain::{Error, Project};
use uuid::Uuid;
#[async_trait]
pub trait SecurityLimiter: Send + Sync {
    async fn admit(&self, key: &str) -> Result<(), Error>;
}
#[async_trait]
pub trait ProjectCache: Send + Sync {
    async fn get(&self, organization: Uuid) -> Result<Option<Vec<Project>>, Error>;
    async fn put(&self, organization: Uuid, projects: &[Project]) -> Result<(), Error>;
    async fn invalidate(&self, organization: Uuid) -> Result<(), Error>;
}
