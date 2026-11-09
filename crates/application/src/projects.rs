use crate::Actor;
use async_trait::async_trait;
use domain::{Error, Permission, Project, Role, validate_name};
use domain::{OrganizationId, ProjectId, UserId};
use std::sync::Arc;
use uuid::Uuid;

pub struct OrganizationSummary {
    pub id: OrganizationId,
    pub name: String,
    pub role: Role,
}
pub struct MemberSummary {
    pub user_id: UserId,
    pub role: Role,
}

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
#[async_trait]
pub trait ProjectRepository: Send {
    async fn project_changed(
        &mut self,
        project: ProjectId,
        correlation_id: Uuid,
    ) -> Result<(), Error>;
    async fn insert_project(&mut self, project: &Project) -> Result<(), Error>;
    async fn load_project(&mut self, id: ProjectId) -> Result<Project, Error>;
    async fn save_project(&mut self, project: &Project, expected: i64) -> Result<(), Error>;
    async fn list_projects(&mut self, after: Option<ProjectId>) -> Result<Vec<Project>, Error>;
}
/// Shared atomic publication boundary. Dropping without commit rolls back.
#[async_trait]
pub trait TenantTransaction: Send {
    async fn audit(&mut self, actor: &Actor, action: &str, subject: Uuid) -> Result<(), Error>;
    async fn commit(self: Box<Self>) -> Result<(), Error>;
}

/// A project consumer cannot mutate membership authority.
///
/// ```compile_fail,E0599
/// use application::projects::ProjectTransaction;
/// async fn change(tx: &mut dyn ProjectTransaction, user: domain::UserId) {
///     tx.set_role(user, domain::Role::Owner).await.unwrap();
/// }
/// ```
///
/// ```compile_fail,E0599
/// use application::projects::ProjectTransaction;
/// async fn remove(tx: &mut dyn ProjectTransaction, user: domain::UserId) {
///     tx.remove_member(user).await.unwrap();
/// }
/// ```
pub trait ProjectTransaction: TenantAuthority + ProjectRepository + TenantTransaction {}
impl<T: TenantAuthority + ProjectRepository + TenantTransaction> ProjectTransaction for T {}

/// Organization creation and discovery share the same atomic transaction boundary.
pub trait OrganizationTransaction:
    TenantAuthority + OrganizationRepository + TenantTransaction
{
}
impl<T: TenantAuthority + OrganizationRepository + TenantTransaction> OrganizationTransaction
    for T
{
}

/// Begins project work with no organization or membership mutation capability.
#[async_trait]
pub trait ProjectUnitOfWork: Send + Sync {
    async fn begin_projects(
        &self,
        organization: OrganizationId,
    ) -> Result<Box<dyn ProjectTransaction>, Error>;
}

/// Begins organization creation or discovery without project or membership administration.
#[async_trait]
pub trait OrganizationUnitOfWork: Send + Sync {
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
        let mut tx = self.uow.begin_organization(Uuid::nil().into()).await?;
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
pub struct Projects {
    uow: Arc<dyn ProjectUnitOfWork>,
    ids: Arc<dyn crate::IdGenerator>,
    cache: Option<Arc<dyn crate::optional::ProjectCache>>,
}
impl Projects {
    pub fn new(uow: Arc<dyn ProjectUnitOfWork>, ids: Arc<dyn crate::IdGenerator>) -> Self {
        Self {
            uow,
            ids,
            cache: None,
        }
    }
    #[must_use]
    pub fn with_cache(mut self, cache: Arc<dyn crate::optional::ProjectCache>) -> Self {
        self.cache = Some(cache);
        self
    }
    async fn authorized(
        &self,
        actor: &Actor,
        org: OrganizationId,
        permission: Permission,
    ) -> Result<Box<dyn ProjectTransaction>, Error> {
        let mut tx = self.uow.begin_projects(org).await?;
        if !tx.current_role(actor, org).await?.allows(permission) {
            return Err(Error::Forbidden);
        }
        Ok(tx)
    }
    /// Create a project with an atomic audit record and durable change job.
    ///
    /// # Errors
    /// Rejects invalid names or insufficient authority; propagates transaction errors.
    pub async fn create(
        &self,
        actor: &Actor,
        org: OrganizationId,
        name: String,
    ) -> Result<Project, Error> {
        let project = Project::new(self.ids.project(), org, name)?;
        let mut tx = self
            .authorized(actor, org, Permission::WriteProject)
            .await?;
        tx.insert_project(&project).await?;
        tx.project_changed(project.id, actor.correlation_id).await?;
        tx.audit(actor, "project.created", project.id.into_uuid())
            .await?;
        tx.commit().await?;
        if let Some(cache) = &self.cache {
            let _ = cache.invalidate(org).await;
        }
        Ok(project)
    }
    /// Rename a project with optimistic concurrency and atomic publication.
    ///
    /// # Errors
    /// Rejects invalid names, missing projects, stale versions or insufficient authority;
    /// propagates transaction errors. Cache invalidation is best effort.
    pub async fn rename(
        &self,
        actor: &Actor,
        org: OrganizationId,
        id: ProjectId,
        expected: i64,
        name: String,
    ) -> Result<Project, Error> {
        let mut tx = self
            .authorized(actor, org, Permission::WriteProject)
            .await?;
        let mut project = tx.load_project(id).await?;
        project.rename(expected, name)?;
        tx.save_project(&project, expected).await?;
        tx.project_changed(id, actor.correlation_id).await?;
        tx.audit(actor, "project.updated", id.into_uuid()).await?;
        tx.commit().await?;
        if let Some(cache) = &self.cache {
            let _ = cache.invalidate(org).await;
        }
        Ok(project)
    }
    /// List one project page after checking current tenant membership.
    ///
    /// # Errors
    /// Propagates session, membership and transaction errors. Cache failures fall back to storage.
    pub async fn list(
        &self,
        actor: &Actor,
        org: OrganizationId,
        after: Option<ProjectId>,
    ) -> Result<Vec<Project>, Error> {
        let mut tx = self.authorized(actor, org, Permission::ReadProject).await?;
        if after.is_none()
            && let Some(cache) = &self.cache
            && let Ok(Some(rows)) = cache.get(org).await
            && rows.iter().all(|p| p.organization_id == org)
        {
            tx.commit().await?;
            return Ok(rows);
        }
        let result = tx.list_projects(after).await?;
        tx.commit().await?;
        if after.is_none()
            && let Some(cache) = &self.cache
        {
            let _ = cache.put(org, &result).await;
        }
        Ok(result)
    }
}
