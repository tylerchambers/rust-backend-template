use super::transaction::TenantWork;
use application::memberships::MembershipRepository;
use async_trait::async_trait;
use domain::{Error, Role, UserId};
fn storage(error: sqlx::Error) -> Error {
    super::errors::unavailable(error, "memberships")
}
pub(crate) fn role(value: &str) -> Result<Role, Error> {
    match value {
        "owner" => Ok(Role::Owner),
        "admin" => Ok(Role::Admin),
        "member" => Ok(Role::Member),
        "viewer" => Ok(Role::Viewer),
        _ => Err(Error::Unavailable),
    }
}
pub(super) fn role_name(role: Role) -> &'static str {
    match role {
        Role::Owner => "owner",
        Role::Admin => "admin",
        Role::Member => "member",
        Role::Viewer => "viewer",
    }
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
    async fn member_role(&mut self, user: UserId) -> Result<Role, Error> {
        let value: Option<String> = sqlx::query_scalar(
            "SELECT role FROM memberships WHERE organization_id=$1 AND user_id=$2",
        )
        .bind(self.organization)
        .bind(user.into_uuid())
        .fetch_optional(&mut *self.tx)
        .await
        .map_err(storage)?;
        role(&value.ok_or(Error::NotFound)?)
    }
    async fn owner_count(&mut self) -> Result<i64, Error> {
        sqlx::query_scalar(
            "SELECT count(*) FROM memberships WHERE organization_id=$1 AND role='owner'",
        )
        .bind(self.organization)
        .fetch_one(&mut *self.tx)
        .await
        .map_err(storage)
    }
    async fn set_role(&mut self, user: UserId, role: Role) -> Result<(), Error> {
        sqlx::query("UPDATE memberships SET role=$1,version=version+1 WHERE organization_id=$2 AND user_id=$3")
            .bind(role_name(role))
            .bind(self.organization)
            .bind(user.into_uuid())
            .execute(&mut *self.tx)
            .await
            .map_err(storage)?;
        Ok(())
    }
}
