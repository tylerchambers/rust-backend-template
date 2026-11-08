use async_trait::async_trait;
use domain::OrganizationId;
use domain::{Error, Project};
#[async_trait]
pub trait SecurityLimiter: Send + Sync {
    async fn admit(&self, key: &str) -> Result<(), Error>;
}
#[async_trait]
pub trait ProjectCache: Send + Sync {
    async fn get(&self, organization: OrganizationId) -> Result<Option<Vec<Project>>, Error>;
    async fn put(&self, organization: OrganizationId, projects: &[Project]) -> Result<(), Error>;
    async fn invalidate(&self, organization: OrganizationId) -> Result<(), Error>;
}
