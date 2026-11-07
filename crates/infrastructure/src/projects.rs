use crate::Database;
use application::{
    Actor,
    projects::{OrganizationRepository, ProjectRepository, TenantTransaction, UnitOfWork},
};
use async_trait::async_trait;
use domain::{Error, Project, Role};
use domain::{OrganizationId, ProjectId, UserId};
use sqlx::{Postgres, Row, Transaction};
use uuid::Uuid;

pub(crate) struct TenantWork {
    pub(crate) tx: Transaction<'static, Postgres>,
    pub(crate) organization: Uuid,
    pub(crate) db: Database,
}
fn storage(_: sqlx::Error) -> Error {
    Error::Unavailable
}
#[async_trait]
impl UnitOfWork for Database {
    async fn begin(
        &self,
        organization: OrganizationId,
    ) -> Result<Box<dyn TenantTransaction>, Error> {
        let tx = self
            .begin_tenant(organization.into_uuid())
            .await
            .map_err(storage)?;
        Ok(Box::new(TenantWork {
            tx,
            organization: organization.into_uuid(),
            db: self.clone(),
        }))
    }
}
#[async_trait]
impl OrganizationRepository for TenantWork {
    async fn validate_actor(&mut self, actor: &Actor) -> Result<(), Error> {
        let live:Option<Uuid>=sqlx::query_scalar("SELECT u.id FROM users u JOIN sessions s ON s.user_id=u.id WHERE u.id=$1 AND s.id=$2 AND NOT u.disabled AND s.revoked_at IS NULL AND s.expires_at>clock_timestamp() AND s.last_seen_at>clock_timestamp()-interval '30 minutes' FOR SHARE OF u,s").bind(actor.user_id.into_uuid()).bind(actor.session_id.into_uuid()).fetch_optional(&mut *self.tx).await.map_err(storage)?;
        live.map(|_| ()).ok_or(Error::Unauthenticated)
    }
    async fn insert_organization(&mut self, id: OrganizationId, name: &str) -> Result<(), Error> {
        sqlx::query!(
            "INSERT INTO organizations(id,name) VALUES($1,$2)",
            id.into_uuid(),
            name
        )
        .execute(&mut *self.tx)
        .await
        .map_err(storage)?;
        Ok(())
    }
    async fn insert_owner(
        &mut self,
        organization: OrganizationId,
        user: UserId,
    ) -> Result<(), Error> {
        sqlx::query!(
            "INSERT INTO memberships(organization_id,user_id,role) VALUES($1,$2,'owner')",
            organization.into_uuid(),
            user.into_uuid()
        )
        .execute(&mut *self.tx)
        .await
        .map_err(storage)?;
        Ok(())
    }
    async fn current_role(
        &mut self,
        actor: &Actor,
        organization: OrganizationId,
    ) -> Result<Role, Error> {
        // Lock session and membership until publication. Revocation serializes with use cases.
        let role:Option<String>=sqlx::query_scalar("SELECT m.role FROM memberships m JOIN users u ON u.id=m.user_id JOIN sessions s ON s.user_id=u.id WHERE m.organization_id=$1 AND m.user_id=$2 AND s.id=$3 AND NOT u.disabled AND s.revoked_at IS NULL AND s.expires_at>clock_timestamp() AND s.last_seen_at>clock_timestamp()-interval '30 minutes' FOR SHARE OF m,u,s").bind(organization.into_uuid()).bind(actor.user_id.into_uuid()).bind(actor.session_id.into_uuid()).fetch_optional(&mut *self.tx).await.map_err(storage)?;
        match role.as_deref() {
            Some("owner") => Ok(Role::Owner),
            Some("admin") => Ok(Role::Admin),
            Some("member") => Ok(Role::Member),
            Some("viewer") => Ok(Role::Viewer),
            _ => Err(Error::Forbidden),
        }
    }
}
fn project(row: sqlx::postgres::PgRow) -> Project {
    Project {
        id: row.get::<Uuid, _>("id").into(),
        organization_id: row.get::<Uuid, _>("organization_id").into(),
        name: row.get("name"),
        version: row.get("version"),
        archived: row.get("archived"),
    }
}
#[async_trait]
impl ProjectRepository for TenantWork {
    async fn insert_project(&mut self, p: &Project) -> Result<(), Error> {
        sqlx::query!(
            "INSERT INTO projects(id,organization_id,name,version,archived) VALUES($1,$2,$3,$4,$5)",
            p.id.into_uuid(),
            p.organization_id.into_uuid(),
            p.name,
            p.version,
            p.archived
        )
        .execute(&mut *self.tx)
        .await
        .map_err(storage)?;
        Ok(())
    }
    async fn load_project(&mut self, id: ProjectId) -> Result<Project, Error> {
        sqlx::query("SELECT id,organization_id,name,version,archived FROM projects WHERE id=$1 AND organization_id=$2").bind(id.into_uuid()).bind(self.organization).fetch_optional(&mut *self.tx).await.map_err(storage)?.map(project).ok_or(Error::NotFound)
    }
    async fn save_project(&mut self, p: &Project, expected: i64) -> Result<(), Error> {
        let n=sqlx::query!("UPDATE projects SET name=$1,version=$2,archived=$3 WHERE id=$4 AND organization_id=$5 AND version=$6",p.name,p.version,p.archived,p.id.into_uuid(),self.organization,expected).execute(&mut *self.tx).await.map_err(storage)?.rows_affected();
        if n == 1 { Ok(()) } else { Err(Error::Conflict) }
    }
    async fn list_projects(&mut self, after: Option<ProjectId>) -> Result<Vec<Project>, Error> {
        Ok(sqlx::query("SELECT id,organization_id,name,version,archived FROM projects WHERE organization_id=$1 AND ($2::uuid IS NULL OR id>$2) ORDER BY id LIMIT 50").bind(self.organization).bind(after.map(ProjectId::into_uuid)).fetch_all(&mut *self.tx).await.map_err(storage)?.into_iter().map(project).collect())
    }
}
#[async_trait]
impl TenantTransaction for TenantWork {
    async fn audit(&mut self, actor: &Actor, action: &str, subject: Uuid) -> Result<(), Error> {
        sqlx::query("INSERT INTO audit_events(id,organization_id,actor_id,action,subject,correlation_id) VALUES($1,$2,$3,$4,$5,$6)").bind(Uuid::now_v7()).bind(self.organization).bind(actor.user_id.into_uuid()).bind(action).bind(subject).bind(actor.correlation_id).execute(&mut *self.tx).await.map_err(storage)?;
        Ok(())
    }
    async fn project_changed(&mut self, id: ProjectId, correlation_id: Uuid) -> Result<(), Error> {
        self.db
            .enqueue_project_correlated(
                &mut self.tx,
                self.organization,
                id.into_uuid(),
                correlation_id,
            )
            .await
            .map_err(|_| Error::Unavailable)
    }
    async fn commit(self: Box<Self>) -> Result<(), Error> {
        self.tx.commit().await.map_err(storage)
    }
}
