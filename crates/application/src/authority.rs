use crate::Actor;
use async_trait::async_trait;
use domain::{Error, OrganizationId, UserId};

/// Current session and tenant membership authority, held until transaction completion.
/// Membership writers must first lock every involved identity via `lock_users`;
/// authority reads cannot safely be followed by acquiring another identity.
#[async_trait]
pub trait TenantAuthority: Send {
    /// Returns an error for a stale session or missing membership, never `false`.
    async fn is_admin(
        &mut self,
        actor: &Actor,
        organization: OrganizationId,
    ) -> Result<bool, Error>;
}

/// Shared locking for authority-changing transactions.
#[async_trait]
pub trait AuthorityMutation: Send {
    /// Serializes authority changes for an organization, including last-admin checks.
    async fn lock_organization(&mut self) -> Result<(), Error>;
    /// Lock all involved identities at retirement strength in deterministic order.
    /// Call once after organization serialization, before any authority/row reads.
    /// Include actor and target; no additional identities may be acquired later.
    async fn lock_users(&mut self, users: &[UserId]) -> Result<(), Error>;
}
