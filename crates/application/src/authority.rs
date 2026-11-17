use crate::Actor;
use async_trait::async_trait;
use domain::{Error, OrganizationId, Role, UserId};
use uuid::Uuid;

/// Current session and tenant membership authority, held until transaction completion.
/// Membership writers must first lock every involved identity via `lock_users`;
/// authority reads cannot safely be followed by acquiring another identity.
#[async_trait]
pub trait TenantAuthority: Send {
    async fn current_role(
        &mut self,
        actor: &Actor,
        organization: OrganizationId,
    ) -> Result<Role, Error>;
}

/// Shared locking and proof consumption for authority-changing transactions.
/// Proof consumption rolls back with the protected mutation, never independently.
#[async_trait]
pub trait AuthorityMutation: Send {
    /// Serializes authority changes for an organization, including last-owner checks.
    async fn lock_organization(&mut self) -> Result<(), Error>;
    /// Lock all involved identities at retirement strength in deterministic order.
    /// Call once after organization serialization, before any authority/row reads.
    /// Include actor and target; no additional identities may be acquired later.
    async fn lock_users(&mut self, users: &[UserId]) -> Result<(), Error>;
    async fn consume_proof(
        &mut self,
        actor: &Actor,
        action: &str,
        target: Uuid,
        token: &str,
    ) -> Result<(), Error>;
}
