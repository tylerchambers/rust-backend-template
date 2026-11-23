use crate::{
    Actor,
    authority::{AuthorityMutation, TenantAuthority},
    reauthentication::ProofPurpose,
    transaction::TenantTransaction,
};
use async_trait::async_trait;
use domain::{Error, InvitationId, OrganizationId, Permission, Role};
use std::sync::Arc;
use uuid::Uuid;

#[async_trait]
pub trait InvitationMailer: Send + Sync {
    async fn send_invitation(
        &self,
        email: &str,
        organization: OrganizationId,
        invitation: InvitationId,
        token: &str,
    ) -> Result<(), Error>;
}

/// Transaction-scoped invitation persistence; the workflow supplies authorization.
#[async_trait]
pub trait InvitationRepository: Send {
    async fn insert_invitation(
        &mut self,
        id: InvitationId,
        email: &str,
        role: Role,
        expires_at: chrono::DateTime<chrono::Utc>,
    ) -> Result<(), Error>;
    /// Validate the live actor and matching mailbox, then conditionally consume
    /// an active invitation and insert membership only when absent. Acceptance
    /// must preserve an existing member's role and share this transaction's audit.
    async fn accept_invitation(
        &mut self,
        actor: &Actor,
        id: InvitationId,
        token: &str,
    ) -> Result<(), Error>;
    async fn enqueue_invitation(
        &mut self,
        id: InvitationId,
        correlation_id: Uuid,
    ) -> Result<(), Error>;
    async fn revoke_invitation(&mut self, id: InvitationId) -> Result<(), Error>;
}

/// Invitation changes publish under the same authority locks as membership changes.
/// This capability cannot perform arbitrary member-role changes.
///
/// ```compile_fail,E0599
/// use application::invitations::InvitationTransaction;
/// async fn change(tx: &mut dyn InvitationTransaction, user: domain::UserId) {
///     tx.set_role(user, domain::Role::Owner).await.unwrap();
/// }
/// ```
pub trait InvitationTransaction:
    TenantAuthority + AuthorityMutation + InvitationRepository + TenantTransaction
{
}
impl<T: TenantAuthority + AuthorityMutation + InvitationRepository + TenantTransaction>
    InvitationTransaction for T
{
}

#[async_trait]
pub trait InvitationUnitOfWork: Send + Sync {
    async fn begin_invitations(
        &self,
        organization: OrganizationId,
    ) -> Result<Box<dyn InvitationTransaction>, Error>;
}

pub struct Invitations {
    uow: Arc<dyn InvitationUnitOfWork>,
    ids: Arc<dyn crate::IdGenerator>,
    clock: Arc<dyn crate::Clock>,
}
impl Invitations {
    pub fn new(
        uow: Arc<dyn InvitationUnitOfWork>,
        ids: Arc<dyn crate::IdGenerator>,
        clock: Arc<dyn crate::Clock>,
    ) -> Self {
        Self { uow, ids, clock }
    }
    /// Publish an invitation and its delivery job atomically.
    ///
    /// # Errors
    /// Rejects invalid email, insufficient authority or invalid proof; propagates transaction errors.
    pub async fn invite(
        &self,
        actor: &Actor,
        org: OrganizationId,
        email: &str,
        role: Role,
        proof: &str,
    ) -> Result<InvitationId, Error> {
        let email = domain::EmailAddress::parse(email)?;
        let email = email.as_str();
        let mut tx = self.uow.begin_invitations(org).await?;
        tx.lock_organization().await?;
        tx.lock_users(&[actor.user_id]).await?;
        let caller = tx.current_role(actor, org).await?;
        if !caller.allows(if role == Role::Owner {
            Permission::ManageOwners
        } else {
            Permission::ManageMembers
        }) {
            return Err(Error::Forbidden);
        }
        tx.consume_proof(actor, ProofPurpose::Invite, proof).await?;
        let id = self.ids.invitation();
        tx.insert_invitation(
            id,
            email,
            role,
            self.clock.now() + chrono::Duration::hours(24),
        )
        .await?;
        tx.enqueue_invitation(id, actor.correlation_id).await?;
        tx.audit(actor, "invitation.created", id.into_uuid())
            .await?;
        tx.commit().await?;
        Ok(id)
    }
    /// Consume an invitation with its audit record atomically. Prior membership
    /// is not required; an existing membership keeps its current role.
    ///
    /// # Errors
    /// Rejects invalid, expired or mismatched invitations; propagates session and transaction errors.
    pub async fn accept(
        &self,
        actor: &Actor,
        org: OrganizationId,
        id: InvitationId,
        token: &str,
    ) -> Result<(), Error> {
        let mut tx = self.uow.begin_invitations(org).await?;
        tx.lock_organization().await?;
        tx.lock_users(&[actor.user_id]).await?;
        tx.accept_invitation(actor, id, token).await?;
        tx.audit(actor, "invitation.accepted", id.into_uuid())
            .await?;
        tx.commit().await
    }
    /// Revoke an invitation under the organization lock. Managing members is
    /// sufficient even when the invitation offered the owner role.
    ///
    /// # Errors
    /// Rejects insufficient authority or invalid proof; propagates lookup and transaction errors.
    pub async fn revoke_invitation(
        &self,
        actor: &Actor,
        org: OrganizationId,
        id: InvitationId,
        proof: &str,
    ) -> Result<(), Error> {
        let mut tx = self.uow.begin_invitations(org).await?;
        tx.lock_organization().await?;
        tx.lock_users(&[actor.user_id]).await?;
        if !tx
            .current_role(actor, org)
            .await?
            .allows(Permission::ManageMembers)
        {
            return Err(Error::Forbidden);
        }
        tx.consume_proof(actor, ProofPurpose::RevokeInvitation(id), proof)
            .await?;
        tx.revoke_invitation(id).await?;
        tx.audit(actor, "invitation.revoked", id.into_uuid())
            .await?;
        tx.commit().await
    }
}
