use crate::{
    Actor,
    authority::{AuthorityMutation, TenantAuthority},
    transaction::TenantTransaction,
};
use async_trait::async_trait;
use domain::{Error, OrganizationId, UserId};
use std::sync::Arc;

#[async_trait]
pub trait MembershipRepository: Send {
    async fn member_is_admin(&mut self, user: UserId) -> Result<bool, Error>;
    async fn admin_count(&mut self) -> Result<i64, Error>;
    async fn set_admin(&mut self, user: UserId, is_admin: bool) -> Result<(), Error>;
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
    /// Remove a member without removing the last admin. Unlike a role change,
    /// removal does not retire the user's global sessions.
    ///
    /// # Errors
    /// Rejects insufficient authority or last-admin removal; propagates transaction errors.
    pub async fn remove(
        &self,
        actor: &Actor,
        org: OrganizationId,
        user: UserId,
    ) -> Result<(), Error> {
        let mut tx = self.uow.begin_memberships(org).await?;
        tx.lock_organization().await?;
        tx.lock_users(&[actor.user_id, user]).await?;
        if !tx.is_admin(actor, org).await? {
            return Err(Error::Forbidden);
        }
        let old = tx.member_is_admin(user).await?;
        if old && tx.admin_count().await? <= 1 {
            return Err(Error::Conflict);
        }
        tx.remove_member(user).await?;
        tx.audit(actor, "membership.removed", user.into_uuid())
            .await?;
        tx.commit().await
    }
    /// Change a role and retire affected credentials atomically. A no-op still
    /// records an audit event but does not retire sessions.
    ///
    /// # Errors
    /// Rejects insufficient authority or last-admin demotion; propagates transaction errors.
    pub async fn change_role(
        &self,
        actor: &Actor,
        org: OrganizationId,
        user: UserId,
        is_admin: bool,
    ) -> Result<(), Error> {
        let mut tx = self.uow.begin_memberships(org).await?;
        tx.lock_organization().await?;
        tx.lock_users(&[actor.user_id, user]).await?;
        if !tx.is_admin(actor, org).await? {
            return Err(Error::Forbidden);
        }
        let old = tx.member_is_admin(user).await?;
        if old && !is_admin && tx.admin_count().await? <= 1 {
            return Err(Error::Conflict);
        }
        tx.set_admin(user, is_admin).await?;
        if old != is_admin {
            tx.retire_sessions(user).await?;
        }
        tx.audit(actor, "membership.role_changed", user.into_uuid())
            .await?;
        tx.commit().await
    }
}
