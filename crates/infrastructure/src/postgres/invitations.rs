use super::{Database, authentication::token_hash, transaction::TenantWork};
use application::Actor;
use domain::{Error, InvitationId};
use uuid::Uuid;
fn storage(error: sqlx::Error) -> Error {
    super::errors::unavailable(error, "memberships")
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
#[async_trait::async_trait]
impl application::invitations::InvitationRepository for TenantWork {
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
    async fn insert_invitation(
        &mut self,
        id: InvitationId,
        email: &str,
        is_admin: bool,
        expires_at: chrono::DateTime<chrono::Utc>,
    ) -> Result<(), Error> {
        use application::Secrets;
        sqlx::query(
            r"
            INSERT INTO invitations(id,organization_id,email,is_admin,token_hash,expires_at)
            VALUES($1,$2,$3,$4,$5,$6)
            ",
        )
        .bind(id.into_uuid())
        .bind(self.organization)
        .bind(email)
        .bind(is_admin)
        .bind(token_hash(&crate::system::SystemSecrets.token()))
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
            .insert(crate::river::jobs::InvitationEmail {
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
        super::authority::lock_actor(&mut self.tx, actor).await?;
        let email = super::authority::live_email(&mut self.tx, actor)
            .await?
            .ok_or(Error::Unauthenticated)?;
        let is_admin: Option<bool> = sqlx::query_scalar(
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
            RETURNING is_admin
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
            INSERT INTO memberships(organization_id,user_id,is_admin)
            VALUES($1,$2,$3)
            ON CONFLICT(organization_id,user_id) DO NOTHING
            ",
        )
        .bind(self.organization)
        .bind(actor.user_id.into_uuid())
        .bind(is_admin.ok_or(Error::Forbidden)?)
        .execute(&mut *self.tx)
        .await
        .map_err(storage)?;
        Ok(())
    }
}

/// Prepare and commit the newest token hash before independent SMTP delivery.
/// Retries may duplicate messages and replace an earlier token; SMTP is not
/// atomic with the database and inactive invitations are silently skipped.
///
/// # Errors
/// Propagates preparation and delivery failures for River retry handling.
pub(crate) async fn deliver_invitation(
    db: &Database,
    secrets: &dyn application::Secrets,
    mail: &dyn application::invitations::InvitationMailer,
    organization: Uuid,
    invitation: Uuid,
) -> Result<(), Error> {
    let token = secrets.token();
    if let Some(email) = db
        .prepare_invitation_delivery(organization, invitation, &token)
        .await?
    {
        mail.send_invitation(&email, organization.into(), invitation.into(), &token)
            .await?;
    }
    Ok(())
}
