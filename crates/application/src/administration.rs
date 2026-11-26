use crate::{Actor, authority::TenantAuthority};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use domain::{Error, InvitationId, OrganizationId, UserId};
use std::sync::Arc;
use uuid::Uuid;

pub struct DirectoryMember {
    pub user_id: UserId,
    pub email: String,
    pub is_admin: bool,
}

pub struct AuditEventSummary {
    pub id: Uuid,
    pub actor_id: UserId,
    pub action: String,
    pub subject: Uuid,
    pub occurred_at: DateTime<Utc>,
}

pub struct InvitationSummary {
    pub id: InvitationId,
    pub email: String,
    pub is_admin: bool,
    pub status: InvitationStatus,
    pub expires_at: DateTime<Utc>,
}

#[derive(Debug, PartialEq, Eq)]
pub enum InvitationStatus {
    Pending,
    Accepted,
    Revoked,
    Expired,
}

#[async_trait]
pub trait AdminReadUnitOfWork: Send + Sync {
    async fn begin_admin_read(
        &self,
        org: OrganizationId,
    ) -> Result<Box<dyn AdminReadTransaction>, Error>;
}

/// Privileged tenant projections with no mutation or audit-insertion capability.
#[async_trait]
pub trait AdminReadTransaction: TenantAuthority {
    async fn invitations(
        &mut self,
        after: Option<InvitationId>,
    ) -> Result<Vec<InvitationSummary>, Error>;
    async fn audit_events(&mut self, after: Option<Uuid>) -> Result<Vec<AuditEventSummary>, Error>;
    async fn member_directory(
        &mut self,
        after: Option<UserId>,
    ) -> Result<Vec<DirectoryMember>, Error>;
    async fn commit(self: Box<Self>) -> Result<(), Error>;
}

pub struct Administration {
    uow: Arc<dyn AdminReadUnitOfWork>,
}

impl Administration {
    #[must_use]
    pub fn new(uow: Arc<dyn AdminReadUnitOfWork>) -> Self {
        Self { uow }
    }

    async fn authorized(
        &self,
        actor: &Actor,
        org: OrganizationId,
    ) -> Result<Box<dyn AdminReadTransaction>, Error> {
        let mut tx = self.uow.begin_admin_read(org).await?;
        if !tx.is_admin(actor, org).await? {
            return Err(Error::Forbidden);
        }
        Ok(tx)
    }

    /// Read the current state of each invitation, including terminal invitations.
    ///
    /// # Errors
    /// Requires current tenant administration authority; propagates storage failures.
    pub async fn invitations(
        &self,
        actor: &Actor,
        org: OrganizationId,
        after: Option<InvitationId>,
    ) -> Result<Vec<InvitationSummary>, Error> {
        let mut tx = self.authorized(actor, org).await?;
        let rows = tx.invitations(after).await?;
        tx.commit().await?;
        Ok(rows)
    }

    /// Read recorded organization events newest first, breaking timestamp ties by descending ID.
    ///
    /// # Errors
    /// Requires current tenant administration authority; propagates storage failures.
    pub async fn audit_events(
        &self,
        actor: &Actor,
        org: OrganizationId,
        after: Option<Uuid>,
    ) -> Result<Vec<AuditEventSummary>, Error> {
        let mut tx = self.authorized(actor, org).await?;
        let rows = tx.audit_events(after).await?;
        tx.commit().await?;
        Ok(rows)
    }

    /// List identities of current tenant members, including disabled accounts.
    ///
    /// # Errors
    /// Requires current tenant administration authority; propagates storage failures.
    pub async fn member_directory(
        &self,
        actor: &Actor,
        org: OrganizationId,
        after: Option<UserId>,
    ) -> Result<Vec<DirectoryMember>, Error> {
        let mut tx = self.authorized(actor, org).await?;
        let rows = tx.member_directory(after).await?;
        tx.commit().await?;
        Ok(rows)
    }
}
