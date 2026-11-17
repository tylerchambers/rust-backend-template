use crate::{
    Actor,
    authority::{AuthorityMutation, TenantAuthority},
    transaction::TenantTransaction,
};
use async_trait::async_trait;
use domain::{Error, OrganizationId, Permission, Role, UserId};
use std::sync::Arc;

#[async_trait]
pub trait MembershipRepository: Send {
    async fn member_role(&mut self, user: UserId) -> Result<Role, Error>;
    async fn owner_count(&mut self) -> Result<i64, Error>;
    async fn set_role(&mut self, user: UserId, role: Role) -> Result<(), Error>;
    /// Retire existing credentials atomically with an authority change.
    /// The user must be included in the upfront `lock_users` call.
    async fn retire_sessions(&mut self, user: UserId) -> Result<(), Error>;
    async fn remove_member(&mut self, user: UserId) -> Result<(), Error>;
}
/// Membership mutations share authority, audit and commit, not invitation issuance.
///
/// ```compile_fail,E0599
/// use application::memberships::MembershipTransaction;
/// async fn revoke(tx: &mut dyn MembershipTransaction, invitation: domain::InvitationId) {
///     tx.revoke_invitation(invitation).await.unwrap();
/// }
/// ```
pub trait MembershipTransaction:
    TenantAuthority + AuthorityMutation + MembershipRepository + TenantTransaction
{
}
impl<T: TenantAuthority + AuthorityMutation + MembershipRepository + TenantTransaction>
    MembershipTransaction for T
{
}

#[async_trait]
pub trait MembershipUnitOfWork: Send + Sync {
    async fn begin_memberships(
        &self,
        organization: OrganizationId,
    ) -> Result<Box<dyn MembershipTransaction>, Error>;
}

pub struct Memberships {
    uow: Arc<dyn MembershipUnitOfWork>,
}
impl Memberships {
    pub fn new(uow: Arc<dyn MembershipUnitOfWork>) -> Self {
        Self { uow }
    }
    /// Remove a member without removing the last owner. Unlike a role change,
    /// removal does not retire the user's global sessions.
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
    /// Change a role and retire affected credentials atomically. A no-op still
    /// consumes proof and audits, but does not retire sessions. Proof binds the
    /// target user, not the requested new role.
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
