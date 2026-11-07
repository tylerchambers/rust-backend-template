use crate::{Database, auth::token_hash, projects::TenantWork};
use application::{
    Actor,
    memberships::{MembershipRepository, ReauthenticationRepository},
};
use async_trait::async_trait;
use domain::{Error, Role};
use uuid::Uuid;
fn storage(_: sqlx::Error) -> Error {
    Error::Unavailable
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
fn role(value: &str) -> Result<Role, Error> {
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
        organization: Uuid,
        token: &str,
    ) -> Result<String, Error> {
        if !matches!(
            action,
            "change_role" | "invite" | "revoke_invitation" | "remove_member"
        ) {
            return Err(Error::Invalid);
        }
        let mut tx = self.begin_tenant(organization).await.map_err(storage)?;
        let email:Option<String>=sqlx::query_scalar("SELECT u.email FROM users u JOIN sessions s ON s.user_id=u.id JOIN memberships m ON m.user_id=u.id WHERE u.id=$1 AND s.id=$2 AND m.organization_id=$3 AND NOT u.disabled AND s.revoked_at IS NULL AND s.expires_at>clock_timestamp() AND s.last_seen_at>clock_timestamp()-interval '30 minutes' FOR SHARE OF u,s,m").bind(actor.user_id).bind(actor.session_id).bind(organization).fetch_optional(&mut *tx).await.map_err(storage)?;
        let email = email.ok_or(Error::Forbidden)?;
        let attempts:i64=sqlx::query_scalar("SELECT count(*) FROM reauthentication_proofs WHERE user_id=$1 AND expires_at>clock_timestamp()").bind(actor.user_id).fetch_one(&mut *tx).await.map_err(storage)?;
        if attempts >= 5 {
            return Err(Error::RateLimited);
        }
        sqlx::query("INSERT INTO reauthentication_proofs(token_hash,user_id,session_id,organization_id,action,target) VALUES($1,$2,$3,$4,$5,$6)").bind(token_hash(token)).bind(actor.user_id).bind(actor.session_id).bind(organization).bind(action).bind(target).execute(&mut *tx).await.map_err(storage)?;
        tx.commit().await.map_err(storage)?;
        Ok(email)
    }
}
#[async_trait]
impl MembershipRepository for TenantWork {
    async fn insert_invitation(&mut self, id: Uuid, email: &str, role: Role) -> Result<(), Error> {
        use application::Secrets;
        sqlx::query("INSERT INTO invitations(id,organization_id,email,role,token_hash,expires_at) VALUES($1,$2,$3,$4,$5,clock_timestamp()+interval '24 hours')").bind(id).bind(self.organization).bind(email).bind(role_name(role)).bind(token_hash(&crate::auth::SystemSecrets.token())).execute(&mut *self.tx).await.map_err(storage)?;
        Ok(())
    }
    async fn enqueue_invitation(&mut self, id: Uuid) -> Result<(), Error> {
        self.db
            .jobs
            .insert(crate::jobs::InvitationEmail {
                organization_id: self.organization,
                invitation_id: id,
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
        id: Uuid,
        token: &str,
    ) -> Result<(), Error> {
        let email:Option<String>=sqlx::query_scalar("SELECT u.email FROM users u JOIN sessions s ON s.user_id=u.id WHERE u.id=$1 AND s.id=$2 AND NOT u.disabled AND s.revoked_at IS NULL AND s.expires_at>clock_timestamp() AND s.last_seen_at>clock_timestamp()-interval '30 minutes' FOR SHARE OF u,s").bind(actor.user_id).bind(actor.session_id).fetch_optional(&mut *self.tx).await.map_err(storage)?;
        let role:Option<String>=sqlx::query_scalar("UPDATE invitations SET accepted_at=clock_timestamp() WHERE id=$1 AND organization_id=$2 AND email=$3 AND token_hash=$4 AND accepted_at IS NULL AND revoked_at IS NULL AND expires_at>clock_timestamp() RETURNING role").bind(id).bind(self.organization).bind(email.ok_or(Error::Unauthenticated)?).bind(token_hash(token)).fetch_optional(&mut *self.tx).await.map_err(storage)?;
        sqlx::query("INSERT INTO memberships(organization_id,user_id,role) VALUES($1,$2,$3) ON CONFLICT(organization_id,user_id) DO NOTHING").bind(self.organization).bind(actor.user_id).bind(role.ok_or(Error::Forbidden)?).execute(&mut *self.tx).await.map_err(storage)?;
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
    async fn member_role(&mut self, user: Uuid) -> Result<Role, Error> {
        let value: Option<String> = sqlx::query_scalar(
            "SELECT role FROM memberships WHERE organization_id=$1 AND user_id=$2",
        )
        .bind(self.organization)
        .bind(user)
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
    async fn set_role(&mut self, user: Uuid, role: Role) -> Result<(), Error> {
        sqlx::query("UPDATE memberships SET role=$1,version=version+1 WHERE organization_id=$2 AND user_id=$3").bind(role_name(role)).bind(self.organization).bind(user).execute(&mut *self.tx).await.map_err(storage)?;
        Ok(())
    }
    async fn consume_proof(
        &mut self,
        actor: &Actor,
        action: &str,
        target: Uuid,
        token: &str,
    ) -> Result<(), Error> {
        let n=sqlx::query("UPDATE reauthentication_proofs SET consumed_at=clock_timestamp() WHERE token_hash=$1 AND user_id=$2 AND session_id=$3 AND organization_id=$4 AND action=$5 AND target=$6 AND consumed_at IS NULL AND expires_at>clock_timestamp()").bind(token_hash(token)).bind(actor.user_id).bind(actor.session_id).bind(self.organization).bind(action).bind(target).execute(&mut *self.tx).await.map_err(storage)?.rows_affected();
        if n == 1 {
            Ok(())
        } else {
            Err(Error::Forbidden)
        }
    }
}
