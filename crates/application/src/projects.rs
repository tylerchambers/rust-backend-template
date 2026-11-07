use crate::Actor;
use async_trait::async_trait;
use domain::{Error, Permission, Project, Role, validate_name};
use std::sync::Arc;
use uuid::Uuid;

#[async_trait]
pub trait OrganizationRepository: Send {
    async fn validate_actor(&mut self, actor: &Actor) -> Result<(), Error>;
    async fn insert_organization(&mut self, id: Uuid, name: &str) -> Result<(), Error>;
    async fn insert_owner(&mut self, organization: Uuid, user: Uuid) -> Result<(), Error>;
    async fn current_role(&mut self, actor: &Actor, organization: Uuid) -> Result<Role, Error>;
}
#[async_trait]
pub trait ProjectRepository: Send {
    async fn insert_project(&mut self, project: &Project) -> Result<(), Error>;
    async fn load_project(&mut self, id: Uuid) -> Result<Project, Error>;
    async fn save_project(&mut self, project: &Project, expected: i64) -> Result<(), Error>;
    async fn list_projects(&mut self, after: Option<Uuid>) -> Result<Vec<Project>, Error>;
}
#[async_trait]
pub trait TenantTransaction:
    OrganizationRepository + ProjectRepository + crate::memberships::MembershipRepository
{
    async fn audit(&mut self, actor: &Actor, action: &str, subject: Uuid) -> Result<(), Error>;
    async fn project_changed(&mut self, project: Uuid) -> Result<(), Error>;
    async fn commit(self: Box<Self>) -> Result<(), Error>;
}
#[async_trait]
pub trait UnitOfWork: Send + Sync {
    async fn begin(&self, organization: Uuid) -> Result<Box<dyn TenantTransaction>, Error>;
}

pub struct Organizations {
    uow: Arc<dyn UnitOfWork>,
}
impl Organizations {
    pub fn new(uow: Arc<dyn UnitOfWork>) -> Self {
        Self { uow }
    }
    pub async fn create(&self, actor: &Actor, name: String) -> Result<Uuid, Error> {
        validate_name(&name)?;
        let id = Uuid::now_v7();
        let mut tx = self.uow.begin(id).await?;
        tx.validate_actor(actor).await?;
        tx.insert_organization(id, &name).await?;
        tx.insert_owner(id, actor.user_id).await?;
        tx.audit(actor, "organization.created", id).await?;
        tx.commit().await?;
        Ok(id)
    }
}
pub struct Projects {
    uow: Arc<dyn UnitOfWork>,
    cache: Option<Arc<dyn crate::optional::ProjectCache>>,
}
impl Projects {
    pub fn new(uow: Arc<dyn UnitOfWork>) -> Self {
        Self { uow, cache: None }
    }
    pub fn with_cache(mut self, cache: Arc<dyn crate::optional::ProjectCache>) -> Self {
        self.cache = Some(cache);
        self
    }
    async fn authorized(
        &self,
        actor: &Actor,
        org: Uuid,
        permission: Permission,
    ) -> Result<Box<dyn TenantTransaction>, Error> {
        let mut tx = self.uow.begin(org).await?;
        if !tx.current_role(actor, org).await?.allows(permission) {
            return Err(Error::Forbidden);
        }
        Ok(tx)
    }
    pub async fn create(&self, actor: &Actor, org: Uuid, name: String) -> Result<Project, Error> {
        let project = Project::new(Uuid::now_v7(), org, name)?;
        let mut tx = self
            .authorized(actor, org, Permission::WriteProject)
            .await?;
        tx.insert_project(&project).await?;
        tx.project_changed(project.id).await?;
        tx.audit(actor, "project.created", project.id).await?;
        tx.commit().await?;
        if let Some(cache) = &self.cache {
            let _ = cache.invalidate(org).await;
        }
        Ok(project)
    }
    pub async fn rename(
        &self,
        actor: &Actor,
        org: Uuid,
        id: Uuid,
        expected: i64,
        name: String,
    ) -> Result<Project, Error> {
        let mut tx = self
            .authorized(actor, org, Permission::WriteProject)
            .await?;
        let mut project = tx.load_project(id).await?;
        project.rename(expected, name)?;
        tx.save_project(&project, expected).await?;
        tx.project_changed(id).await?;
        tx.audit(actor, "project.updated", id).await?;
        tx.commit().await?;
        if let Some(cache) = &self.cache {
            let _ = cache.invalidate(org).await;
        }
        Ok(project)
    }
    pub async fn list(
        &self,
        actor: &Actor,
        org: Uuid,
        after: Option<Uuid>,
    ) -> Result<Vec<Project>, Error> {
        let mut tx = self.authorized(actor, org, Permission::ReadProject).await?;
        if after.is_none()
            && let Some(cache) = &self.cache
                && let Ok(Some(rows)) = cache.get(org).await
                    && rows.iter().all(|p| p.organization_id == org) {
                        tx.commit().await?;
                        return Ok(rows);
                    }
        let result = tx.list_projects(after).await?;
        tx.commit().await?;
        if after.is_none()
            && let Some(cache) = &self.cache {
                let _ = cache.put(org, &result).await;
            }
        Ok(result)
    }
}
