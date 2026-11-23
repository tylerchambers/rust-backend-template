use crate::Actor;
use async_trait::async_trait;
use domain::{Error, InvitationId, OrganizationId, UserId};
use std::sync::Arc;
use uuid::Uuid;

/// The target of an invitation to join is the scoped organization itself.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProofPurpose {
    ChangeRole(UserId),
    Invite,
    RevokeInvitation(InvitationId),
    RemoveMember(UserId),
}

impl ProofPurpose {
    #[must_use]
    pub fn binding(self, organization: OrganizationId) -> (&'static str, Uuid) {
        match self {
            Self::ChangeRole(user) => ("change_role", user.into_uuid()),
            Self::Invite => ("invite", organization.into_uuid()),
            Self::RevokeInvitation(invitation) => ("revoke_invitation", invitation.into_uuid()),
            Self::RemoveMember(user) => ("remove_member", user.into_uuid()),
        }
    }
}

#[async_trait]
pub trait ReauthenticationMailer: Send + Sync {
    async fn send_proof(
        &self,
        email: &str,
        purpose: ProofPurpose,
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
    /// Propagates stale session, rate-limit, storage and delivery errors.
    pub async fn request(
        &self,
        actor: &Actor,
        purpose: ProofPurpose,
        org: OrganizationId,
    ) -> Result<(), Error> {
        let token = self.secrets.token();
        let email = self
            .repository
            .issue_proof(actor, purpose, org, &token)
            .await?;
        self.mail.send_proof(&email, purpose, org, &token).await
    }
}

#[async_trait]
pub trait ReauthenticationRepository: Send + Sync {
    async fn issue_proof(
        &self,
        actor: &Actor,
        purpose: ProofPurpose,
        organization: OrganizationId,
        token: &str,
    ) -> Result<String, Error>;
}
