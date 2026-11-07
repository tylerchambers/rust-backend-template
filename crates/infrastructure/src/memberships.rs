use crate::{Database, auth::token_hash, projects::TenantWork};
use application::{
    Actor,
    memberships::{MembershipRepository, ReauthenticationRepository},
};
use async_trait::async_trait;
use domain::{Error, Role};
use domain::{InvitationId, OrganizationId, UserId};
use uuid::Uuid;
fn storage(error: sqlx::Error) -> Error {
    crate::storage::unavailable(error, "memberships")
}
impl Database {
    pub async fn prepare_invitation_delivery(
        &self,
        org: Uuid,
        id: Uuid,
        token: &str,
    ) -> Result<Option<String>, Error> {
        let mut tx = self.begin_tenant(org).await.map_err(storage)?;
        let email=sqlx::query_scalar("UPDATE invitations SET token_hash=$1 WHERE id=$2 AND organization_id=$3 AND accepted_at IS NULL AND revoked_at IS NULL AND expires_at>clock_timestamp() RETURNING email").bind(token_hash(token)).bind(id).bind(org).fetch_optional(&mut *tx).await.map_err(storage)?;
        tx.commit().await.map_err(storage)?;
        Ok(email)
    }
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
fn role_name(role: Role) -> &'static str {
    match role {
        Role::Owner => "owner",
        Role::Admin => "admin",
        Role::Member => "member",
        Role::Viewer => "viewer",
    }
}
#[async_trait]
impl ReauthenticationRepository for Database {
    async fn issue_proof(
        &self,
        actor: &Actor,
        action: &str,
        target: Uuid,
        organization: OrganizationId,
        token: &str,
    ) -> Result<String, Error> {
        if !matches!(
            action,
            "change_role" | "invite" | "revoke_invitation" | "remove_member"
        ) {
            return Err(Error::Invalid);
        }
        let mut tx = self
            .begin_tenant(organization.into_uuid())
            .await
            .map_err(storage)?;
        let email:Option<String>=sqlx::query_scalar("SELECT u.email FROM users u JOIN sessions s ON s.user_id=u.id JOIN memberships m ON m.user_id=u.id WHERE u.id=$1 AND s.id=$2 AND m.organization_id=$3 AND NOT u.disabled AND s.revoked_at IS NULL AND s.expires_at>clock_timestamp() AND s.last_seen_at>clock_timestamp()-interval '30 minutes' FOR UPDATE OF u FOR SHARE OF s,m").bind(actor.user_id.into_uuid()).bind(actor.session_id.into_uuid()).bind(organization.into_uuid()).fetch_optional(&mut *tx).await.map_err(storage)?;
        let email = email.ok_or(Error::Forbidden)?;
        let attempts:i64=sqlx::query_scalar("SELECT count(*) FROM reauthentication_proofs WHERE user_id=$1 AND expires_at>clock_timestamp()").bind(actor.user_id.into_uuid()).fetch_one(&mut *tx).await.map_err(storage)?;
        if attempts >= 5 {
            return Err(Error::RateLimited);
        }
        sqlx::query("INSERT INTO reauthentication_proofs(token_hash,user_id,session_id,organization_id,action,target) VALUES($1,$2,$3,$4,$5,$6)").bind(token_hash(token)).bind(actor.user_id.into_uuid()).bind(actor.session_id.into_uuid()).bind(organization.into_uuid()).bind(action).bind(target).execute(&mut *tx).await.map_err(storage)?;
        tx.commit().await.map_err(storage)?;
        Ok(email)
    }
}
#[async_trait]
impl MembershipRepository for TenantWork {
    async fn retire_sessions(&mut self, user: UserId) -> Result<(), Error> {
        // Login upserts this same user before publishing a session. The second
        // statement gets a fresh snapshot after any concurrent login commits.
        sqlx::query("SELECT id FROM users WHERE id=$1 FOR UPDATE")
            .bind(user.into_uuid())
            .fetch_one(&mut *self.tx)
            .await
            .map_err(storage)?;
        sqlx::query("UPDATE sessions SET revoked_at=clock_timestamp() WHERE user_id=$1 AND revoked_at IS NULL")
            .bind(user.into_uuid()).execute(&mut *self.tx).await.map_err(storage)?;
        Ok(())
    }
    async fn revoke_invitation(&mut self, id: InvitationId) -> Result<(), Error> {
        let n=sqlx::query("UPDATE invitations SET revoked_at=clock_timestamp() WHERE id=$1 AND organization_id=$2 AND accepted_at IS NULL").bind(id.into_uuid()).bind(self.organization).execute(&mut *self.tx).await.map_err(storage)?.rows_affected();
        if n == 1 { Ok(()) } else { Err(Error::NotFound) }
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
    async fn insert_invitation(
        &mut self,
        id: InvitationId,
        email: &str,
        role: Role,
        expires_at: chrono::DateTime<chrono::Utc>,
    ) -> Result<(), Error> {
        use application::Secrets;
        sqlx::query("INSERT INTO invitations(id,organization_id,email,role,token_hash,expires_at) VALUES($1,$2,$3,$4,$5,$6)").bind(id.into_uuid()).bind(self.organization).bind(email).bind(role_name(role)).bind(token_hash(&crate::auth::SystemSecrets.token())).bind(expires_at).execute(&mut *self.tx).await.map_err(storage)?;
        Ok(())
    }
    async fn enqueue_invitation(
        &mut self,
        id: InvitationId,
        correlation_id: Uuid,
    ) -> Result<(), Error> {
        self.db
            .jobs
            .insert(crate::jobs::InvitationEmail {
                correlation_id,
                organization_id: self.organization,
                invitation_id: id.into_uuid(),
            })
            .opts(riverqueue::InsertOpts::default().with_max_attempts(5))
            .tx(&mut self.tx)
            .await
            .map_err(|_| Error::Unavailable)?;
        Ok(())
    }
    async fn accept_invitation(
        &mut self,
        actor: &Actor,
        id: InvitationId,
        token: &str,
    ) -> Result<(), Error> {
        let email:Option<String>=sqlx::query_scalar("SELECT u.email FROM users u JOIN sessions s ON s.user_id=u.id WHERE u.id=$1 AND s.id=$2 AND NOT u.disabled AND s.revoked_at IS NULL AND s.expires_at>clock_timestamp() AND s.last_seen_at>clock_timestamp()-interval '30 minutes' FOR SHARE OF u,s").bind(actor.user_id.into_uuid()).bind(actor.session_id.into_uuid()).fetch_optional(&mut *self.tx).await.map_err(storage)?;
        let role:Option<String>=sqlx::query_scalar("UPDATE invitations SET accepted_at=clock_timestamp() WHERE id=$1 AND organization_id=$2 AND email=$3 AND token_hash=$4 AND accepted_at IS NULL AND revoked_at IS NULL AND expires_at>clock_timestamp() RETURNING role").bind(id.into_uuid()).bind(self.organization).bind(email.ok_or(Error::Unauthenticated)?).bind(token_hash(token)).fetch_optional(&mut *self.tx).await.map_err(storage)?;
        sqlx::query("INSERT INTO memberships(organization_id,user_id,role) VALUES($1,$2,$3) ON CONFLICT(organization_id,user_id) DO NOTHING").bind(self.organization).bind(actor.user_id.into_uuid()).bind(role.ok_or(Error::Forbidden)?).execute(&mut *self.tx).await.map_err(storage)?;
        Ok(())
    }
    async fn lock_organization(&mut self) -> Result<(), Error> {
        sqlx::query("SELECT id FROM organizations WHERE id=$1 FOR UPDATE")
            .bind(self.organization)
            .fetch_optional(&mut *self.tx)
            .await
            .map_err(storage)?
            .ok_or(Error::NotFound)?;
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
        sqlx::query("UPDATE memberships SET role=$1,version=version+1 WHERE organization_id=$2 AND user_id=$3").bind(role_name(role)).bind(self.organization).bind(user.into_uuid()).execute(&mut *self.tx).await.map_err(storage)?;
        Ok(())
    }
    async fn consume_proof(
        &mut self,
        actor: &Actor,
        action: &str,
        target: Uuid,
        token: &str,
    ) -> Result<(), Error> {
        let n=sqlx::query("UPDATE reauthentication_proofs SET consumed_at=clock_timestamp() WHERE token_hash=$1 AND user_id=$2 AND session_id=$3 AND organization_id=$4 AND action=$5 AND target=$6 AND consumed_at IS NULL AND expires_at>clock_timestamp()").bind(token_hash(token)).bind(actor.user_id.into_uuid()).bind(actor.session_id.into_uuid()).bind(self.organization).bind(action).bind(target).execute(&mut *self.tx).await.map_err(storage)?.rows_affected();
        if n == 1 {
            Ok(())
        } else {
            Err(Error::Forbidden)
        }
    }
}
