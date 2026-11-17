use crate::{InfraResult, river::jobs::ProjectChanged};
use sqlx::{Postgres, Transaction};
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
use super::{Database, transaction::TenantWork};
use async_trait::async_trait;
use domain::{Error, Project, ProjectId};
use uuid::Uuid;
fn storage(error: sqlx::Error) -> Error {
    super::errors::unavailable(error, "projects")
}
use application::projects::ProjectRepository;
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
    async fn enqueue_project_change(
        &mut self,
        id: ProjectId,
        correlation_id: Uuid,
    ) -> Result<(), Error> {
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

impl Database {
    pub(crate) async fn refresh_project_index(
        &self,
        organization: Uuid,
        project: Uuid,
    ) -> Result<(), Error> {
        let mut tx = self.begin_tenant(organization).await.map_err(storage)?;
        sqlx::query("INSERT INTO project_index(organization_id,project_id,name,version) SELECT organization_id,id,name,version FROM projects WHERE organization_id=$1 AND id=$2 ON CONFLICT(project_id) DO UPDATE SET name=EXCLUDED.name,version=EXCLUDED.version WHERE project_index.version<=EXCLUDED.version")
            .bind(organization).bind(project).execute(&mut *tx).await.map_err(storage)?;
        tx.commit().await.map_err(storage)
    }
}

impl Database {
    /// Enqueue a project change in the caller-owned transaction.
    ///
    /// # Errors
    /// Propagates River insertion errors; the caller owns commit or rollback.
    pub async fn enqueue_project(
        &self,
        tx: &mut Transaction<'_, Postgres>,
        organization_id: Uuid,
        project_id: Uuid,
    ) -> InfraResult<()> {
        self.enqueue_project_correlated(tx, organization_id, project_id, Uuid::now_v7())
            .await
    }
    /// Enqueue a project change carrying the invocation correlation ID.
    ///
    /// # Errors
    /// Propagates River insertion errors; the caller owns commit or rollback.
    pub async fn enqueue_project_correlated(
        &self,
        tx: &mut Transaction<'_, Postgres>,
        organization_id: Uuid,
        project_id: Uuid,
        correlation_id: Uuid,
    ) -> InfraResult<()> {
        self.jobs
            .insert(ProjectChanged {
                correlation_id,
                organization_id,
                project_id,
            })
            .tx(tx)
            .await?;
        Ok(())
    }
}
