use super::transaction::TenantWork;
use application::Actor;
use async_trait::async_trait;
use domain::{Error, OrganizationId, UserId};
use uuid::Uuid;
fn storage(error: sqlx::Error) -> Error {
    super::errors::unavailable(error, "projects")
}
use application::organizations::OrganizationRepository;
#[async_trait]
impl OrganizationRepository for TenantWork {
    async fn list_organizations(
        &mut self,
        actor: &Actor,
        after: Option<OrganizationId>,
    ) -> Result<Vec<application::organizations::OrganizationSummary>, Error> {
        self.validate_actor(actor).await?;
        sqlx::query("SELECT set_config('app.discovery_user_id',$1,true)")
            .bind(actor.user_id.to_string())
            .execute(&mut *self.tx)
            .await
            .map_err(storage)?;
        let rows: Vec<(Uuid, String, bool)> = sqlx::query_as(
            r"
            SELECT o.id,o.name,m.is_admin
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
        Ok(rows
            .into_iter()
            .map(
                |(id, name, is_admin)| application::organizations::OrganizationSummary {
                    id: id.into(),
                    name,
                    is_admin,
                },
            )
            .collect())
    }
    async fn list_members(
        &mut self,
        after: Option<UserId>,
    ) -> Result<Vec<application::organizations::MemberSummary>, Error> {
        let rows: Vec<(Uuid, bool)> = sqlx::query_as(
            r"
            SELECT user_id,is_admin
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
        Ok(rows
            .into_iter()
            .map(
                |(user_id, is_admin)| application::organizations::MemberSummary {
                    user_id: user_id.into(),
                    is_admin,
                },
            )
            .collect())
    }
    async fn validate_actor(&mut self, actor: &Actor) -> Result<(), Error> {
        super::authority::lock_actor(&mut self.tx, actor).await?;
        super::authority::live_email(&mut self.tx, actor)
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
    async fn insert_admin(
        &mut self,
        organization: OrganizationId,
        user: UserId,
    ) -> Result<(), Error> {
        sqlx::query("INSERT INTO memberships(organization_id,user_id,is_admin) VALUES($1,$2,true)")
            .bind(organization.into_uuid())
            .bind(user.into_uuid())
            .execute(&mut *self.tx)
            .await
            .map_err(storage)?;
        Ok(())
    }
}
