use crate::{Actor, authority::TenantAuthority, transaction::TenantTransaction};
use async_trait::async_trait;
use domain::{Error, OrganizationId, Role, UserId, validate_name};
use std::sync::Arc;

pub struct OrganizationSummary {
    pub id: OrganizationId,
    pub name: String,
    pub role: Role,
}
pub struct MemberSummary {
    pub user_id: UserId,
    pub role: Role,
}

#[async_trait]
pub trait OrganizationRepository: Send {
    async fn list_organizations(
        &mut self,
        actor: &Actor,
        after: Option<OrganizationId>,
    ) -> Result<Vec<OrganizationSummary>, Error>;
    async fn list_members(&mut self, after: Option<UserId>) -> Result<Vec<MemberSummary>, Error>;
    async fn validate_actor(&mut self, actor: &Actor) -> Result<(), Error>;
    async fn insert_organization(&mut self, id: OrganizationId, name: &str) -> Result<(), Error>;
    async fn insert_owner(
        &mut self,
        organization: OrganizationId,
        user: UserId,
    ) -> Result<(), Error>;
}

/// Organization creation and discovery share the same atomic transaction boundary.
pub trait OrganizationTransaction:
    TenantAuthority + OrganizationRepository + TenantTransaction
{
}
impl<T: TenantAuthority + OrganizationRepository + TenantTransaction> OrganizationTransaction
    for T
{
}

/// Begins organization creation or discovery without project or membership administration.
#[async_trait]
pub trait OrganizationUnitOfWork: Send + Sync {
    /// Begin account-scoped discovery with no selected tenant. The repository
    /// enables SELECT-only self-discovery after locked live-session validation.
    async fn begin_discovery(&self) -> Result<Box<dyn OrganizationTransaction>, Error>;

    async fn begin_organization(
        &self,
        organization: OrganizationId,
    ) -> Result<Box<dyn OrganizationTransaction>, Error>;
}

pub struct Organizations {
    uow: Arc<dyn OrganizationUnitOfWork>,
    ids: Arc<dyn crate::IdGenerator>,
}
impl Organizations {
    /// List one page of the authenticated actor's organizations.
    ///
    /// # Errors
    /// Propagates invalid session and transaction errors.
    pub async fn list(
        &self,
        actor: &Actor,
        after: Option<OrganizationId>,
    ) -> Result<Vec<OrganizationSummary>, Error> {
        // No tenant is selected for account-scoped discovery. The adapter enables
        // only self-membership SELECT visibility after validating the session.
        let mut tx = self.uow.begin_discovery().await?;
        tx.validate_actor(actor).await?;
        let rows = tx.list_organizations(actor, after).await?;
        tx.commit().await?;
        Ok(rows)
    }
    /// List one member page after checking current tenant membership.
    ///
    /// # Errors
    /// Propagates session, membership and transaction errors.
    pub async fn members(
        &self,
        actor: &Actor,
        org: OrganizationId,
        after: Option<UserId>,
    ) -> Result<Vec<MemberSummary>, Error> {
        let mut tx = self.uow.begin_organization(org).await?;
        tx.current_role(actor, org).await?;
        let rows = tx.list_members(after).await?;
        tx.commit().await?;
        Ok(rows)
    }
    pub fn new(uow: Arc<dyn OrganizationUnitOfWork>, ids: Arc<dyn crate::IdGenerator>) -> Self {
        Self { uow, ids }
    }
    /// Create an organization, owner membership and audit record atomically.
    ///
    /// # Errors
    /// Rejects invalid names or sessions; propagates transaction errors.
    pub async fn create(&self, actor: &Actor, name: String) -> Result<OrganizationId, Error> {
        validate_name(&name)?;
        let id = self.ids.organization();
        let mut tx = self.uow.begin_organization(id).await?;
        tx.validate_actor(actor).await?;
        tx.insert_organization(id, &name).await?;
        tx.insert_owner(id, actor.user_id).await?;
        tx.audit(actor, "organization.created", id.into_uuid())
            .await?;
        tx.commit().await?;
        Ok(id)
    }
}
