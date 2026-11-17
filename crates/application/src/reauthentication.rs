use crate::Actor;
use async_trait::async_trait;
use domain::{Error, OrganizationId};
use std::sync::Arc;
use uuid::Uuid;

#[async_trait]
pub trait ReauthenticationMailer: Send + Sync {
    async fn send_proof(
        &self,
        email: &str,
        action: &str,
        target: Uuid,
        organization: OrganizationId,
        token: &str,
    ) -> Result<(), Error>;
}
pub struct Reauthentication {
    repository: Arc<dyn ReauthenticationRepository>,
    mail: Arc<dyn ReauthenticationMailer>,
    secrets: Arc<dyn crate::Secrets>,
}
impl Reauthentication {
    pub fn new(
        repository: Arc<dyn ReauthenticationRepository>,
        mail: Arc<dyn ReauthenticationMailer>,
        secrets: Arc<dyn crate::Secrets>,
    ) -> Self {
        Self {
            repository,
            mail,
            secrets,
        }
    }
    /// Issue and email a session/action/target-bound proof.
    ///
    /// # Errors
    /// Propagates invalid action, stale session, rate-limit, storage and delivery errors.
    pub async fn request(
        &self,
        actor: &Actor,
        action: &str,
        target: Uuid,
        org: OrganizationId,
    ) -> Result<(), Error> {
        let token = self.secrets.token();
        let email = self
            .repository
            .issue_proof(actor, action, target, org, &token)
            .await?;
        self.mail
            .send_proof(&email, action, target, org, &token)
            .await
    }
}

#[async_trait]
pub trait ReauthenticationRepository: Send + Sync {
    async fn issue_proof(
        &self,
        actor: &Actor,
        action: &str,
        target: Uuid,
        organization: OrganizationId,
        token: &str,
    ) -> Result<String, Error>;
}
