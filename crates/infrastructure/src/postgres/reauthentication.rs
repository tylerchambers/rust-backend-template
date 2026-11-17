use super::{Database, authentication::token_hash, transaction::TenantWork};
use application::Actor;
use application::reauthentication::ReauthenticationRepository;
use async_trait::async_trait;
use domain::{Error, OrganizationId};
use uuid::Uuid;
fn storage(error: sqlx::Error) -> Error {
    super::errors::unavailable(error, "memberships")
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
        super::authority::lock_actor(&mut tx, actor).await?;
        sqlx::query(
            "SELECT user_id FROM memberships WHERE user_id=$1 AND organization_id=$2 FOR SHARE",
        )
        .bind(actor.user_id.into_uuid())
        .bind(organization.into_uuid())
        .fetch_optional(&mut *tx)
        .await
        .map_err(storage)?
        .ok_or(Error::Forbidden)?;
        let email = super::authority::live_email(&mut tx, actor)
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
impl TenantWork {
    pub(super) async fn consume_proof_record(
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

impl Database {
    pub(super) async fn cleanup_reauthentication_proofs(&self) -> Result<(), Error> {
        sqlx::query("DELETE FROM reauthentication_proofs WHERE expires_at<clock_timestamp()-interval '1 day'").execute(&self.pool).await.map_err(storage)?;
        Ok(())
    }
}
