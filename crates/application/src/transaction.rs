use crate::Actor;
use async_trait::async_trait;
use domain::Error;
use uuid::Uuid;

/// Shared atomic publication boundary. Dropping without commit rolls back.
#[async_trait]
pub trait TenantTransaction: Send {
    async fn audit(&mut self, actor: &Actor, action: &str, subject: Uuid) -> Result<(), Error>;
    async fn commit(self: Box<Self>) -> Result<(), Error>;
}
