use super::transaction::TenantWork;
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
        let rows = sqlx::query_as::<_, (Uuid, String, bool, DateTime<Utc>, bool, bool, bool)>(
            "SELECT id,email,is_admin,expires_at,accepted_at IS NOT NULL,revoked_at IS NOT NULL,
                    expires_at <= statement_timestamp()
             FROM invitations WHERE organization_id=$1 AND ($2::uuid IS NULL OR id>$2)
             ORDER BY id LIMIT 50",
        )
        .bind(self.organization)
        .bind(after.map(InvitationId::into_uuid))
        .fetch_all(&mut *self.tx)
        .await
        .map_err(storage)?;
        Ok(rows
            .into_iter()
            .map(
                |(id, email, is_admin, expires_at, accepted, revoked, expired)| {
                    let status = if accepted {
                        InvitationStatus::Accepted
                    } else if revoked {
                        InvitationStatus::Revoked
                    } else if expired {
                        InvitationStatus::Expired
                    } else {
                        InvitationStatus::Pending
                    };
                    InvitationSummary {
                        id: id.into(),
                        email,
                        is_admin,
                        status,
                        expires_at,
                    }
                },
            )
            .collect())
    }

    async fn audit_events(&mut self, after: Option<Uuid>) -> Result<Vec<AuditEventSummary>, Error> {
        type EventRow = (Uuid, Uuid, String, Uuid, DateTime<Utc>);
        let query = if let Some(id) = after {
            sqlx::query_as::<_, EventRow>(
                "SELECT id,actor_id,action,subject,occurred_at FROM audit_events
                 WHERE organization_id=$1 AND (occurred_at,id) < (
                     SELECT occurred_at,id FROM audit_events WHERE organization_id=$1 AND id=$2
                 )
                 ORDER BY occurred_at DESC,id DESC LIMIT 50",
            )
            .bind(self.organization)
            .bind(id)
        } else {
            sqlx::query_as::<_, EventRow>(
                "SELECT id,actor_id,action,subject,occurred_at FROM audit_events
                 WHERE organization_id=$1 ORDER BY occurred_at DESC,id DESC LIMIT 50",
            )
            .bind(self.organization)
        };
        let rows = query.fetch_all(&mut *self.tx).await.map_err(storage)?;
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
        let rows = sqlx::query_as::<_, (Uuid, String, bool)>(
            "SELECT m.user_id,u.email,m.is_admin FROM memberships m JOIN users u ON u.id=m.user_id
             WHERE m.organization_id=$1 AND ($2::uuid IS NULL OR m.user_id>$2)
             ORDER BY m.user_id LIMIT 50",
        )
        .bind(self.organization)
        .bind(after.map(UserId::into_uuid))
        .fetch_all(&mut *self.tx)
        .await
        .map_err(storage)?;
        Ok(rows
            .into_iter()
            .map(|(user_id, email, is_admin)| DirectoryMember {
                user_id: user_id.into(),
                email,
                is_admin,
            })
            .collect())
    }

    async fn commit(self: Box<Self>) -> Result<(), Error> {
        TenantTransaction::commit(self).await
    }
}
