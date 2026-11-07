use crate::Actor;
use async_trait::async_trait;
use domain::{Error, Permission, Project, Role, validate_name};
use domain::{OrganizationId, ProjectId, UserId};
use std::sync::Arc;
use uuid::Uuid;

#[async_trait]
pub trait OrganizationRepository: Send {
    async fn validate_actor(&mut self, actor: &Actor) -> Result<(), Error>;
    async fn insert_organization(&mut self, id: OrganizationId, name: &str) -> Result<(), Error>;
    async fn insert_owner(
        &mut self,
        organization: OrganizationId,
        user: UserId,
    ) -> Result<(), Error>;
    async fn current_role(
        &mut self,
        actor: &Actor,
        organization: OrganizationId,
    ) -> Result<Role, Error>;
}
#[async_trait]
pub trait ProjectRepository: Send {
    async fn insert_project(&mut self, project: &Project) -> Result<(), Error>;
    async fn load_project(&mut self, id: ProjectId) -> Result<Project, Error>;
    async fn save_project(&mut self, project: &Project, expected: i64) -> Result<(), Error>;
    async fn list_projects(&mut self, after: Option<ProjectId>) -> Result<Vec<Project>, Error>;
}
#[async_trait]
pub trait TenantTransaction:
    OrganizationRepository + ProjectRepository + crate::memberships::MembershipRepository
{
    async fn audit(&mut self, actor: &Actor, action: &str, subject: Uuid) -> Result<(), Error>;
    async fn project_changed(
        &mut self,
        project: ProjectId,
        correlation_id: Uuid,
    ) -> Result<(), Error>;
    async fn commit(self: Box<Self>) -> Result<(), Error>;
}
#[async_trait]
pub trait UnitOfWork: Send + Sync {
    async fn begin(
        &self,
        organization: OrganizationId,
    ) -> Result<Box<dyn TenantTransaction>, Error>;
}

pub struct Organizations {
    uow: Arc<dyn UnitOfWork>,
    ids: Arc<dyn crate::IdGenerator>,
}
impl Organizations {
    pub fn new(uow: Arc<dyn UnitOfWork>, ids: Arc<dyn crate::IdGenerator>) -> Self {
        Self { uow, ids }
    }
    pub async fn create(&self, actor: &Actor, name: String) -> Result<OrganizationId, Error> {
        validate_name(&name)?;
        let id = self.ids.organization();
        let mut tx = self.uow.begin(id).await?;
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
    uow: Arc<dyn UnitOfWork>,
    ids: Arc<dyn crate::IdGenerator>,
    cache: Option<Arc<dyn crate::optional::ProjectCache>>,
}
impl Projects {
    pub fn new(uow: Arc<dyn UnitOfWork>, ids: Arc<dyn crate::IdGenerator>) -> Self {
        Self {
            uow,
            ids,
            cache: None,
        }
    }
    pub fn with_cache(mut self, cache: Arc<dyn crate::optional::ProjectCache>) -> Self {
        self.cache = Some(cache);
        self
    }
    async fn authorized(
        &self,
        actor: &Actor,
        org: OrganizationId,
        permission: Permission,
    ) -> Result<Box<dyn TenantTransaction>, Error> {
        let mut tx = self.uow.begin(org).await?;
        if !tx.current_role(actor, org).await?.allows(permission) {
            return Err(Error::Forbidden);
        }
        Ok(tx)
    }
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
