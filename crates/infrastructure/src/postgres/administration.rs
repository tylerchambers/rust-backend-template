use super::{memberships::role, transaction::TenantWork};
use application::{
    administration::{
        AdminReadTransaction, AuditEventSummary, DirectoryMember, InvitationStatus,
        InvitationSummary,
    },
    transaction::TenantTransaction,
};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use domain::{Error, InvitationId, UserId};
use uuid::Uuid;

fn storage(error: sqlx::Error) -> Error {
    super::errors::unavailable(error, "administration")
}

#[async_trait]
impl AdminReadTransaction for TenantWork {
    async fn invitations(
        &mut self,
        after: Option<InvitationId>,
    ) -> Result<Vec<InvitationSummary>, Error> {
        sqlx::query_as::<_, (Uuid, String, String, DateTime<Utc>, bool, bool, bool)>(
            "SELECT id,email,role,expires_at,accepted_at IS NOT NULL,revoked_at IS NOT NULL,
                    expires_at <= statement_timestamp()
             FROM invitations WHERE organization_id=$1 AND ($2::uuid IS NULL OR id>$2)
             ORDER BY id LIMIT 50",
        )
        .bind(self.organization)
        .bind(after.map(InvitationId::into_uuid))
        .fetch_all(&mut *self.tx)
        .await
        .map_err(storage)?
        .into_iter()
        .map(
            |(id, email, stored_role, expires_at, accepted, revoked, expired)| {
                let status = if accepted {
                    InvitationStatus::Accepted
                } else if revoked {
                    InvitationStatus::Revoked
                } else if expired {
                    InvitationStatus::Expired
                } else {
                    InvitationStatus::Pending
                };
                Ok(InvitationSummary {
                    id: id.into(),
                    email,
                    role: role(&stored_role)?,
                    status,
                    expires_at,
                })
            },
        )
        .collect()
    }

    async fn audit_events(&mut self, after: Option<Uuid>) -> Result<Vec<AuditEventSummary>, Error> {
        let rows = sqlx::query_as::<_, (Uuid, Uuid, String, Uuid, DateTime<Utc>)>(
            "SELECT id,actor_id,action,subject,occurred_at FROM audit_events
             WHERE organization_id=$1 AND ($2::uuid IS NULL OR id>$2)
             ORDER BY id LIMIT 50",
        )
        .bind(self.organization)
        .bind(after)
        .fetch_all(&mut *self.tx)
        .await
        .map_err(storage)?;
        Ok(rows
            .into_iter()
            .map(
                |(id, actor_id, action, subject, occurred_at)| AuditEventSummary {
                    id,
                    actor_id: actor_id.into(),
                    action,
                    subject,
                    occurred_at,
                },
            )
            .collect())
    }

    async fn member_directory(
        &mut self,
        after: Option<UserId>,
    ) -> Result<Vec<DirectoryMember>, Error> {
        sqlx::query_as::<_, (Uuid, String, String)>(
            "SELECT m.user_id,u.email,m.role FROM memberships m JOIN users u ON u.id=m.user_id
             WHERE m.organization_id=$1 AND ($2::uuid IS NULL OR m.user_id>$2)
             ORDER BY m.user_id LIMIT 50",
        )
        .bind(self.organization)
        .bind(after.map(UserId::into_uuid))
        .fetch_all(&mut *self.tx)
        .await
        .map_err(storage)?
        .into_iter()
        .map(|(user_id, email, stored_role)| {
            Ok(DirectoryMember {
                user_id: user_id.into(),
                email,
                role: role(&stored_role)?,
            })
        })
        .collect()
    }

    async fn commit(self: Box<Self>) -> Result<(), Error> {
        TenantTransaction::commit(self).await
    }
}
