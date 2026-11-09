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
    /// Replace the token hash only for a still-deliverable invitation.
    ///
    /// # Errors
    /// Returns `Unavailable` on database failure. Missing or inactive invitations return `None`.
    pub async fn prepare_invitation_delivery(
        &self,
        org: Uuid,
        id: Uuid,
        token: &str,
    ) -> Result<Option<String>, Error> {
        let mut tx = self.begin_tenant(org).await.map_err(storage)?;
        let email = sqlx::query_scalar(
            r"
            UPDATE invitations
            SET token_hash=$1
            WHERE id=$2
            AND organization_id=$3
            AND accepted_at IS NULL
            AND revoked_at IS NULL
            AND expires_at>clock_timestamp()
            RETURNING email
            ",
        )
        .bind(token_hash(token))
        .bind(id)
        .bind(org)
        .fetch_optional(&mut *tx)
        .await
        .map_err(storage)?;
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
        // Proof admission and login/retirement serialize on this identity.
        sqlx::query("SELECT id FROM users WHERE id=$1 FOR UPDATE")
            .bind(actor.user_id.into_uuid())
            .fetch_optional(&mut *tx)
            .await
            .map_err(storage)?
            .ok_or(Error::Forbidden)?;
        crate::identity::lock_actor(&mut tx, actor).await?;
        sqlx::query(
            "SELECT user_id FROM memberships WHERE user_id=$1 AND organization_id=$2 FOR SHARE",
        )
        .bind(actor.user_id.into_uuid())
        .bind(organization.into_uuid())
        .fetch_optional(&mut *tx)
        .await
        .map_err(storage)?
        .ok_or(Error::Forbidden)?;
        let email = crate::identity::live_email(&mut tx, actor)
            .await?
            .ok_or(Error::Forbidden)?;
        let attempts: i64 = sqlx::query_scalar(
            r"
            SELECT count(*)
            FROM reauthentication_proofs
            WHERE user_id=$1
            AND expires_at>clock_timestamp()
            ",
        )
        .bind(actor.user_id.into_uuid())
        .fetch_one(&mut *tx)
        .await
        .map_err(storage)?;
        if attempts >= 5 {
            return Err(Error::RateLimited);
        }
        sqlx::query(r"
            INSERT INTO reauthentication_proofs(token_hash,user_id,session_id,organization_id,action,target)
            VALUES($1,$2,$3,$4,$5,$6)
            ")
            .bind(token_hash(token))
            .bind(actor.user_id.into_uuid())
            .bind(actor.session_id.into_uuid())
            .bind(organization.into_uuid())
            .bind(action)
            .bind(target)
            .execute(&mut *tx)
            .await
            .map_err(storage)?;
        tx.commit().await.map_err(storage)?;
        Ok(email)
    }
}
#[async_trait]
impl MembershipRepository for TenantWork {
    async fn lock_users(&mut self, users: &[UserId]) -> Result<(), Error> {
        let ids: Vec<Uuid> = users.iter().map(|user| user.into_uuid()).collect();
        // One row per identity, UUID order, final strength: no later lock upgrades.
        sqlx::query("SELECT id FROM users WHERE id=ANY($1) ORDER BY id FOR UPDATE")
            .bind(ids)
            .fetch_all(&mut *self.tx)
            .await
            .map_err(storage)?;
        Ok(())
    }
    async fn retire_sessions(&mut self, user: UserId) -> Result<(), Error> {
        // lock_users already serialized with login's identity upsert. This
        // fresh statement sees sessions from every login that committed first.
        sqlx::query("UPDATE sessions SET revoked_at=clock_timestamp() WHERE user_id=$1 AND revoked_at IS NULL")
            .bind(user.into_uuid()).execute(&mut *self.tx).await.map_err(storage)?;
        Ok(())
    }
    async fn revoke_invitation(&mut self, id: InvitationId) -> Result<(), Error> {
        let n = sqlx::query(
            r"
            UPDATE invitations
            SET revoked_at=clock_timestamp()
            WHERE id=$1
            AND organization_id=$2
            AND accepted_at IS NULL
            ",
        )
        .bind(id.into_uuid())
        .bind(self.organization)
        .execute(&mut *self.tx)
        .await
        .map_err(storage)?
        .rows_affected();
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
        sqlx::query(
            r"
            INSERT INTO invitations(id,organization_id,email,role,token_hash,expires_at)
            VALUES($1,$2,$3,$4,$5,$6)
            ",
        )
        .bind(id.into_uuid())
        .bind(self.organization)
        .bind(email)
        .bind(role_name(role))
        .bind(token_hash(&crate::auth::SystemSecrets.token()))
        .bind(expires_at)
        .execute(&mut *self.tx)
        .await
        .map_err(storage)?;
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
        crate::identity::lock_actor(&mut self.tx, actor).await?;
        let email = crate::identity::live_email(&mut self.tx, actor)
            .await?
            .ok_or(Error::Unauthenticated)?;
        let role: Option<String> = sqlx::query_scalar(
            r"
            UPDATE invitations
            SET accepted_at=clock_timestamp()
            WHERE id=$1
            AND organization_id=$2
            AND email=$3
            AND token_hash=$4
            AND accepted_at IS NULL
            AND revoked_at IS NULL
            AND expires_at>clock_timestamp()
            RETURNING role
            ",
        )
        .bind(id.into_uuid())
        .bind(self.organization)
        .bind(email)
        .bind(token_hash(token))
        .fetch_optional(&mut *self.tx)
        .await
        .map_err(storage)?;
        sqlx::query(
            r"
            INSERT INTO memberships(organization_id,user_id,role)
            VALUES($1,$2,$3)
            ON CONFLICT(organization_id,user_id) DO NOTHING
            ",
        )
        .bind(self.organization)
        .bind(actor.user_id.into_uuid())
        .bind(role.ok_or(Error::Forbidden)?)
        .execute(&mut *self.tx)
        .await
        .map_err(storage)?;
        Ok(())
    }
    async fn lock_organization(&mut self) -> Result<(), Error> {
        // Serialize mutations without blocking FK KEY SHARE checks from proof
        // insertion (which already owns an identity lock). IDs never change.
        sqlx::query("SELECT id FROM organizations WHERE id=$1 FOR NO KEY UPDATE")
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
        sqlx::query("UPDATE memberships SET role=$1,version=version+1 WHERE organization_id=$2 AND user_id=$3")
            .bind(role_name(role))
            .bind(self.organization)
            .bind(user.into_uuid())
            .execute(&mut *self.tx)
            .await
            .map_err(storage)?;
        Ok(())
    }
    async fn consume_proof(
        &mut self,
        actor: &Actor,
        action: &str,
        target: Uuid,
        token: &str,
    ) -> Result<(), Error> {
        let n = sqlx::query(
            r"
            UPDATE reauthentication_proofs
            SET consumed_at=clock_timestamp()
            WHERE token_hash=$1
            AND user_id=$2
            AND session_id=$3
            AND organization_id=$4
            AND action=$5
            AND target=$6
            AND consumed_at IS NULL
            AND expires_at>clock_timestamp()
            ",
        )
        .bind(token_hash(token))
        .bind(actor.user_id.into_uuid())
        .bind(actor.session_id.into_uuid())
        .bind(self.organization)
        .bind(action)
        .bind(target)
        .execute(&mut *self.tx)
        .await
        .map_err(storage)?
        .rows_affected();
        if n == 1 {
            Ok(())
        } else {
            Err(Error::Forbidden)
        }
    }
}
