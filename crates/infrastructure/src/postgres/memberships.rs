use super::transaction::TenantWork;
use application::memberships::MembershipRepository;
use async_trait::async_trait;
use domain::{Error, UserId};
fn storage(error: sqlx::Error) -> Error {
    super::errors::unavailable(error, "memberships")
}
#[async_trait]
impl MembershipRepository for TenantWork {
    async fn retire_sessions(&mut self, user: UserId) -> Result<(), Error> {
        // lock_users already serialized with login's identity upsert. This
        // fresh statement sees sessions from every login that committed first.
        sqlx::query("UPDATE sessions SET revoked_at=clock_timestamp() WHERE user_id=$1 AND revoked_at IS NULL")
            .bind(user.into_uuid()).execute(&mut *self.tx).await.map_err(storage)?;
        Ok(())
    }
    async fn remove_member(&mut self, user: UserId) -> Result<(), Error> {
        sqlx::query("DELETE FROM memberships WHERE user_id=$1 AND organization_id=$2")
            .bind(user.into_uuid())
            .bind(self.organization)
            .execute(&mut *self.tx)
            .await
            .map_err(storage)?;
        Ok(())
    }
    async fn member_is_admin(&mut self, user: UserId) -> Result<bool, Error> {
        sqlx::query_scalar(
            "SELECT is_admin FROM memberships WHERE organization_id=$1 AND user_id=$2",
        )
        .bind(self.organization)
        .bind(user.into_uuid())
        .fetch_optional(&mut *self.tx)
        .await
        .map_err(storage)?
        .ok_or(Error::NotFound)
    }
    async fn admin_count(&mut self) -> Result<i64, Error> {
        sqlx::query_scalar("SELECT count(*) FROM memberships WHERE organization_id=$1 AND is_admin")
            .bind(self.organization)
            .fetch_one(&mut *self.tx)
            .await
            .map_err(storage)
    }
    async fn set_admin(&mut self, user: UserId, is_admin: bool) -> Result<(), Error> {
        sqlx::query("UPDATE memberships SET is_admin=$1,version=version+1 WHERE organization_id=$2 AND user_id=$3")
            .bind(is_admin)
            .bind(self.organization)
            .bind(user.into_uuid())
            .execute(&mut *self.tx)
            .await
            .map_err(storage)?;
        Ok(())
    }
}
