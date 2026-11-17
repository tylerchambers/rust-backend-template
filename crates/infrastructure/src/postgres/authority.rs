//! Explicit identity -> session ordering shared by tenant authority readers.
use application::Actor;
use domain::Error;
use sqlx::{Postgres, Transaction};

fn storage(error: sqlx::Error) -> Error {
    super::errors::unavailable(error, "identity")
}

pub(crate) async fn lock_actor(
    tx: &mut Transaction<'_, Postgres>,
    actor: &Actor,
) -> Result<(), Error> {
    sqlx::query("SELECT id FROM users WHERE id=$1 FOR SHARE")
        .bind(actor.user_id.into_uuid())
        .fetch_optional(&mut **tx)
        .await
        .map_err(storage)?;
    sqlx::query("SELECT id FROM sessions WHERE id=$1 AND user_id=$2 FOR SHARE")
        .bind(actor.session_id.into_uuid())
        .bind(actor.user_id.into_uuid())
        .fetch_optional(&mut **tx)
        .await
        .map_err(storage)?;
    Ok(())
}

/// Recheck time-sensitive authority in a fresh statement after all lock waits.
/// Caller must already hold the identity and session locks until publication.
pub(crate) async fn live_email(
    tx: &mut Transaction<'_, Postgres>,
    actor: &Actor,
) -> Result<Option<String>, Error> {
    sqlx::query_scalar(
        r"
        SELECT u.email
        FROM users u JOIN sessions s ON s.user_id=u.id
        WHERE u.id=$1 AND s.id=$2
        AND NOT u.disabled AND s.revoked_at IS NULL
        AND s.expires_at>clock_timestamp()
        AND s.last_seen_at>clock_timestamp()-interval '30 minutes'
        ",
    )
    .bind(actor.user_id.into_uuid())
    .bind(actor.session_id.into_uuid())
    .fetch_optional(&mut **tx)
    .await
    .map_err(storage)
}

use super::transaction::TenantWork;
use application::authority::AuthorityMutation;
use application::authority::TenantAuthority;
use async_trait::async_trait;
use domain::UserId;
use domain::{OrganizationId, Role};
use uuid::Uuid;
#[async_trait]
impl TenantAuthority for TenantWork {
    async fn current_role(
        &mut self,
        actor: &Actor,
        organization: OrganizationId,
    ) -> Result<Role, Error> {
        super::authority::lock_actor(&mut self.tx, actor).await?;
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
        super::authority::live_email(&mut self.tx, actor)
            .await?
            .ok_or(Error::Forbidden)?;
        super::memberships::role(role.as_deref().ok_or(Error::Forbidden)?)
    }
}

#[async_trait]
impl AuthorityMutation for TenantWork {
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
    async fn consume_proof(
        &mut self,
        actor: &Actor,
        action: &str,
        target: Uuid,
        token: &str,
    ) -> Result<(), Error> {
        self.consume_proof_record(actor, action, target, token)
            .await
    }
}
