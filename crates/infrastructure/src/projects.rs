#[cfg(test)]
mod row_tests {
    #[tokio::test]
    #[allow(clippy::unwrap_used, reason = "Test setup must fail immediately")]
    async fn malformed_project_rows_return_decode_errors() {
        let pool = sqlx::PgPool::connect(&std::env::var("TEST_DATABASE_URL").unwrap())
            .await
            .unwrap();
        let result = sqlx::query_as::<_, super::ProjectRow>(r"
            SELECT 'not-a-uuid'::text AS id, gen_random_uuid() AS organization_id, 'name'::text AS name, 1::bigint AS version, false AS archived
            ").fetch_one(&pool).await;
        assert!(matches!(result, Err(sqlx::Error::ColumnDecode { .. })));
    }
}

use crate::Database;
use application::{
    Actor,
    projects::{OrganizationRepository, ProjectRepository, TenantAuthority, TenantTransaction},
};
use async_trait::async_trait;
use domain::{Error, Project, Role};
use domain::{OrganizationId, ProjectId, UserId};
use sqlx::{Postgres, Transaction};
use uuid::Uuid;

pub(crate) struct TenantWork {
    pub(crate) tx: Transaction<'static, Postgres>,
    pub(crate) organization: Uuid,
    pub(crate) db: Database,
}
fn storage(error: sqlx::Error) -> Error {
    crate::storage::unavailable(error, "projects")
}
#[async_trait]
impl OrganizationRepository for TenantWork {
    async fn list_organizations(
        &mut self,
        actor: &Actor,
        after: Option<OrganizationId>,
    ) -> Result<Vec<application::projects::OrganizationSummary>, Error> {
        self.validate_actor(actor).await?;
        sqlx::query("SELECT set_config('app.discovery_user_id',$1,true)")
            .bind(actor.user_id.to_string())
            .execute(&mut *self.tx)
            .await
            .map_err(storage)?;
        let rows: Vec<(Uuid, String, String)> = sqlx::query_as(
            r"
            SELECT o.id,o.name,m.role
            FROM organizations o
            JOIN memberships m ON m.organization_id=o.id
            WHERE m.user_id=$1
            AND ($2::uuid IS NULL OR o.id>$2)
            ORDER BY o.id
            LIMIT 50
            ",
        )
        .bind(actor.user_id.into_uuid())
        .bind(after.map(OrganizationId::into_uuid))
        .fetch_all(&mut *self.tx)
        .await
        .map_err(storage)?;
        rows.into_iter()
            .map(|(id, name, value)| {
                Ok(application::projects::OrganizationSummary {
                    id: id.into(),
                    name,
                    role: crate::memberships::role(&value)?,
                })
            })
            .collect()
    }
    async fn list_members(
        &mut self,
        after: Option<UserId>,
    ) -> Result<Vec<application::projects::MemberSummary>, Error> {
        let rows: Vec<(Uuid, String)> = sqlx::query_as(
            r"
            SELECT user_id,role
            FROM memberships
            WHERE organization_id=$1
            AND ($2::uuid IS NULL OR user_id>$2)
            ORDER BY user_id
            LIMIT 50
            ",
        )
        .bind(self.organization)
        .bind(after.map(UserId::into_uuid))
        .fetch_all(&mut *self.tx)
        .await
        .map_err(storage)?;
        rows.into_iter()
            .map(|(id, value)| {
                Ok(application::projects::MemberSummary {
                    user_id: id.into(),
                    role: crate::memberships::role(&value)?,
                })
            })
            .collect()
    }
    async fn validate_actor(&mut self, actor: &Actor) -> Result<(), Error> {
        crate::identity::lock_actor(&mut self.tx, actor).await?;
        crate::identity::live_email(&mut self.tx, actor)
            .await?
            .ok_or(Error::Unauthenticated)?;
        Ok(())
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
}
#[async_trait]
impl TenantAuthority for TenantWork {
    async fn current_role(
        &mut self,
        actor: &Actor,
        organization: OrganizationId,
    ) -> Result<Role, Error> {
        crate::identity::lock_actor(&mut self.tx, actor).await?;
        let role: Option<String> = sqlx::query_scalar(
            r"
            SELECT role FROM memberships
            WHERE organization_id=$1 AND user_id=$2
            FOR SHARE
            ",
        )
        .bind(organization.into_uuid())
        .bind(actor.user_id.into_uuid())
        .fetch_optional(&mut *self.tx)
        .await
        .map_err(storage)?;
        crate::identity::live_email(&mut self.tx, actor)
            .await?
            .ok_or(Error::Forbidden)?;
        crate::memberships::role(role.as_deref().ok_or(Error::Forbidden)?)
    }
}
#[derive(sqlx::FromRow)]
struct ProjectRow {
    id: Uuid,
    organization_id: Uuid,
    name: String,
    version: i64,
    archived: bool,
}
impl From<ProjectRow> for Project {
    fn from(row: ProjectRow) -> Self {
        Self {
            id: row.id.into(),
            organization_id: row.organization_id.into(),
            name: row.name,
            version: row.version,
            archived: row.archived,
        }
    }
}
#[async_trait]
impl ProjectRepository for TenantWork {
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
        sqlx::query_as::<_, ProjectRow>(
            r"
            SELECT id,organization_id,name,version,archived
            FROM projects
            WHERE id=$1
            AND organization_id=$2
            ",
        )
        .bind(id.into_uuid())
        .bind(self.organization)
        .fetch_optional(&mut *self.tx)
        .await
        .map_err(storage)?
        .map(Into::into)
        .ok_or(Error::NotFound)
    }
    async fn save_project(&mut self, p: &Project, expected: i64) -> Result<(), Error> {
        let n = sqlx::query!(
            "UPDATE projects SET name=$1,version=$2,archived=$3 WHERE id=$4 AND organization_id=$5 AND version=$6",
            p.name,
            p.version,
            p.archived,
            p.id.into_uuid(),
            self.organization,
            expected
        )
            .execute(&mut *self.tx)
            .await
            .map_err(storage)?
            .rows_affected();
        if n == 1 { Ok(()) } else { Err(Error::Conflict) }
    }
    async fn list_projects(&mut self, after: Option<ProjectId>) -> Result<Vec<Project>, Error> {
        Ok(sqlx::query_as::<_, ProjectRow>(
            r"
            SELECT id,organization_id,name,version,archived
            FROM projects
            WHERE organization_id=$1
            AND ($2::uuid IS NULL OR id>$2)
            ORDER BY id
            LIMIT 50
            ",
        )
        .bind(self.organization)
        .bind(after.map(ProjectId::into_uuid))
        .fetch_all(&mut *self.tx)
        .await
        .map_err(storage)?
        .into_iter()
        .map(Into::into)
        .collect())
    }
}
#[async_trait]
impl TenantTransaction for TenantWork {
    async fn audit(&mut self, actor: &Actor, action: &str, subject: Uuid) -> Result<(), Error> {
        sqlx::query(
            r"
            INSERT INTO audit_events(id,organization_id,actor_id,action,subject,correlation_id)
            VALUES($1,$2,$3,$4,$5,$6)
            ",
        )
        .bind(Uuid::now_v7())
        .bind(self.organization)
        .bind(actor.user_id.into_uuid())
        .bind(action)
        .bind(subject)
        .bind(actor.correlation_id)
        .execute(&mut *self.tx)
        .await
        .map_err(storage)?;
        Ok(())
    }
    async fn commit(self: Box<Self>) -> Result<(), Error> {
        self.tx.commit().await.map_err(storage)
    }
}
