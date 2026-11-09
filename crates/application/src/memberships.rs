use crate::{
    Actor,
    projects::{TenantAuthority, TenantTransaction},
};
use async_trait::async_trait;
use domain::{Error, Permission, Role};
use domain::{InvitationId, OrganizationId, UserId};
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
#[async_trait]
pub trait MembershipRepository: Send {
    /// Serializes authority changes for an organization, including last-owner checks.
    async fn lock_organization(&mut self) -> Result<(), Error>;
    /// Lock all involved identities at retirement strength in deterministic order.
    /// Call once after organization serialization, before any authority/row reads.
    /// Include actor and target; no additional identities may be acquired later.
    async fn lock_users(&mut self, users: &[UserId]) -> Result<(), Error>;
    async fn member_role(&mut self, user: UserId) -> Result<Role, Error>;
    async fn owner_count(&mut self) -> Result<i64, Error>;
    async fn set_role(&mut self, user: UserId, role: Role) -> Result<(), Error>;
    /// Retire existing credentials atomically with an authority change.
    /// The user must be included in the upfront `lock_users` call.
    async fn retire_sessions(&mut self, user: UserId) -> Result<(), Error>;
    async fn consume_proof(
        &mut self,
        actor: &Actor,
        action: &str,
        target: Uuid,
        token: &str,
    ) -> Result<(), Error>;
    async fn insert_invitation(
        &mut self,
        id: InvitationId,
        email: &str,
        role: Role,
        expires_at: chrono::DateTime<chrono::Utc>,
    ) -> Result<(), Error>;
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
    async fn remove_member(&mut self, user: UserId) -> Result<(), Error>;
}
/// Membership mutations share authority, audit and commit, not project persistence.
pub trait MembershipTransaction:
    TenantAuthority + MembershipRepository + TenantTransaction
{
}
impl<T: TenantAuthority + MembershipRepository + TenantTransaction> MembershipTransaction for T {}

#[async_trait]
pub trait MembershipUnitOfWork: Send + Sync {
    async fn begin_memberships(
        &self,
        organization: OrganizationId,
    ) -> Result<Box<dyn MembershipTransaction>, Error>;
}

pub struct Memberships {
    uow: Arc<dyn MembershipUnitOfWork>,
    ids: Arc<dyn crate::IdGenerator>,
    clock: Arc<dyn crate::Clock>,
}
impl Memberships {
    pub fn new(
        uow: Arc<dyn MembershipUnitOfWork>,
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
        let mut tx = self.uow.begin_memberships(org).await?;
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
        tx.consume_proof(actor, "invite", org.into_uuid(), proof)
            .await?;
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
    /// Consume an invitation with its audit record atomically.
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
        let mut tx = self.uow.begin_memberships(org).await?;
        tx.lock_organization().await?;
        tx.lock_users(&[actor.user_id]).await?;
        tx.accept_invitation(actor, id, token).await?;
        tx.audit(actor, "invitation.accepted", id.into_uuid())
            .await?;
        tx.commit().await
    }
    /// Revoke an invitation under the organization lock.
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
        let mut tx = self.uow.begin_memberships(org).await?;
        tx.lock_organization().await?;
        tx.lock_users(&[actor.user_id]).await?;
        if !tx
            .current_role(actor, org)
            .await?
            .allows(Permission::ManageMembers)
        {
            return Err(Error::Forbidden);
        }
        tx.consume_proof(actor, "revoke_invitation", id.into_uuid(), proof)
            .await?;
        tx.revoke_invitation(id).await?;
        tx.audit(actor, "invitation.revoked", id.into_uuid())
            .await?;
        tx.commit().await
    }
    /// Remove a member without removing the last owner.
    ///
    /// # Errors
    /// Rejects insufficient authority, invalid proof or last-owner removal; propagates transaction errors.
    pub async fn remove(
        &self,
        actor: &Actor,
        org: OrganizationId,
        user: UserId,
        proof: &str,
    ) -> Result<(), Error> {
        let mut tx = self.uow.begin_memberships(org).await?;
        tx.lock_organization().await?;
        tx.lock_users(&[actor.user_id, user]).await?;
        let caller = tx.current_role(actor, org).await?;
        let old = tx.member_role(user).await?;
        if !caller.allows(if old == Role::Owner {
            Permission::ManageOwners
        } else {
            Permission::ManageMembers
        }) {
            return Err(Error::Forbidden);
        }
        tx.consume_proof(actor, "remove_member", user.into_uuid(), proof)
            .await?;
        if old == Role::Owner && tx.owner_count().await? <= 1 {
            return Err(Error::Conflict);
        }
        tx.remove_member(user).await?;
        tx.audit(actor, "membership.removed", user.into_uuid())
            .await?;
        tx.commit().await
    }
    /// Change a role and retire affected credentials atomically.
    ///
    /// # Errors
    /// Rejects insufficient authority, invalid proof or last-owner demotion; propagates transaction errors.
    pub async fn change_role(
        &self,
        actor: &Actor,
        org: OrganizationId,
        user: UserId,
        role: Role,
        proof: &str,
    ) -> Result<(), Error> {
        let mut tx = self.uow.begin_memberships(org).await?;
        tx.lock_organization().await?;
        tx.lock_users(&[actor.user_id, user]).await?;
        let caller = tx.current_role(actor, org).await?;
        let old = tx.member_role(user).await?;
        let permission = if old == Role::Owner || role == Role::Owner {
            Permission::ManageOwners
        } else {
            Permission::ManageMembers
        };
        if !caller.allows(permission) {
            return Err(Error::Forbidden);
        }
        tx.consume_proof(actor, "change_role", user.into_uuid(), proof)
            .await?;
        if old == Role::Owner && role != Role::Owner && tx.owner_count().await? <= 1 {
            return Err(Error::Conflict);
        }
        tx.set_role(user, role).await?;
        if old != role {
            tx.retire_sessions(user).await?;
        }
        tx.audit(actor, "membership.role_changed", user.into_uuid())
            .await?;
        tx.commit().await
    }
}
