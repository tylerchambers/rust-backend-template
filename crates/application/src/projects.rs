use crate::{Actor, authority::TenantAuthority, transaction::TenantTransaction};
use async_trait::async_trait;
use domain::{Error, OrganizationId, Project, ProjectId};
use std::sync::Arc;
use uuid::Uuid;

/// Best-effort, eventually consistent read projection; never an authority source.
#[async_trait]
pub trait ProjectCache: Send + Sync {
    async fn get(&self, organization: OrganizationId) -> Result<Option<Vec<Project>>, Error>;
    async fn put(&self, organization: OrganizationId, projects: &[Project]) -> Result<(), Error>;
    async fn invalidate(&self, organization: OrganizationId) -> Result<(), Error>;
}

#[async_trait]
pub trait ProjectRepository: Send {
    async fn enqueue_project_change(
        &mut self,
        project: ProjectId,
        correlation_id: Uuid,
    ) -> Result<(), Error>;
    async fn insert_project(&mut self, project: &Project) -> Result<(), Error>;
    async fn load_project(&mut self, id: ProjectId) -> Result<Project, Error>;
    async fn save_project(&mut self, project: &Project, expected: i64) -> Result<(), Error>;
    async fn list_projects(&mut self, after: Option<ProjectId>) -> Result<Vec<Project>, Error>;
}

/// A project consumer cannot mutate membership authority.
///
/// ```compile_fail,E0599
/// use application::projects::ProjectTransaction;
/// async fn change(tx: &mut dyn ProjectTransaction, user: domain::UserId) {
///     tx.set_admin(user, true).await.unwrap();
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

/// Begins project work with no organization or membership mutation capability.
#[async_trait]
pub trait ProjectUnitOfWork: Send + Sync {
    async fn begin_projects(
        &self,
        organization: OrganizationId,
    ) -> Result<Box<dyn ProjectTransaction>, Error>;
}

pub struct Projects {
    uow: Arc<dyn ProjectUnitOfWork>,
    ids: Arc<dyn crate::IdGenerator>,
    cache: Option<Arc<dyn ProjectCache>>,
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
    pub fn with_cache(mut self, cache: Arc<dyn ProjectCache>) -> Self {
        self.cache = Some(cache);
        self
    }
    async fn authorized(
        &self,
        actor: &Actor,
        org: OrganizationId,
    ) -> Result<Box<dyn ProjectTransaction>, Error> {
        let mut tx = self.uow.begin_projects(org).await?;
        tx.is_admin(actor, org).await?;
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
        let mut tx = self.authorized(actor, org).await?;
        tx.insert_project(&project).await?;
        tx.enqueue_project_change(project.id, actor.correlation_id)
            .await?;
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
        let mut tx = self.authorized(actor, org).await?;
        let mut project = tx.load_project(id).await?;
        project.rename(expected, name)?;
        tx.save_project(&project, expected).await?;
        tx.enqueue_project_change(id, actor.correlation_id).await?;
        tx.audit(actor, "project.updated", id.into_uuid()).await?;
        tx.commit().await?;
        if let Some(cache) = &self.cache {
            let _ = cache.invalidate(org).await;
        }
        Ok(project)
    }
    /// Load one project within the caller's organization.
    ///
    /// # Errors
    /// Rejects insufficient authority or a missing project; propagates transaction errors.
    pub async fn get(
        &self,
        actor: &Actor,
        org: OrganizationId,
        id: ProjectId,
    ) -> Result<Project, Error> {
        let mut tx = self.authorized(actor, org).await?;
        let project = tx.load_project(id).await?;
        tx.commit().await?;
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
        let mut tx = self.authorized(actor, org).await?;
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
