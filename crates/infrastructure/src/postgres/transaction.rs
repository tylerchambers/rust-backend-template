use super::{Database, errors};
use application::Actor;
use application::transaction::TenantTransaction;
use async_trait::async_trait;
use domain::Error;
use sqlx::{Postgres, Transaction};
use uuid::Uuid;
pub(crate) struct TenantWork {
    pub(crate) tx: Transaction<'static, Postgres>,
    pub(crate) organization: Uuid,
    pub(crate) db: Database,
}
fn storage(error: sqlx::Error) -> Error {
    errors::unavailable(error, "projects")
}
impl Database {
    async fn tenant_work(
        &self,
        organization: domain::OrganizationId,
    ) -> Result<TenantWork, domain::Error> {
        let tx = self
            .begin_tenant(organization.into_uuid())
            .await
            .map_err(|error| errors::unavailable(error, "projects"))?;
        Ok(TenantWork {
            tx,
            organization: organization.into_uuid(),
            db: self.clone(),
        })
    }

    async fn discovery_work(&self) -> Result<TenantWork, Error> {
        let tx = riverqueue::database::begin_postgres(&self.pool)
            .await
            .map_err(storage)?;
        Ok(TenantWork {
            tx,
            organization: Uuid::nil(),
            db: self.clone(),
        })
    }
    /// Begin a cancellation-safe transaction with transaction-local tenant context.
    /// The caller must validate actor authority before performing tenant work.
    ///
    /// # Errors
    /// Propagates database begin/context-setting errors.
    pub async fn begin_tenant(
        &self,
        organization: Uuid,
    ) -> Result<Transaction<'static, Postgres>, sqlx::Error> {
        let mut tx = riverqueue::database::begin_postgres(&self.pool).await?;
        sqlx::query("SELECT set_config('app.organization_id',$1,true)")
            .bind(organization.to_string())
            .execute(&mut *tx)
            .await?;
        Ok(tx)
    }
}
#[async_trait::async_trait]
impl application::projects::ProjectUnitOfWork for Database {
    async fn begin_projects(
        &self,
        organization: domain::OrganizationId,
    ) -> Result<Box<dyn application::projects::ProjectTransaction>, domain::Error> {
        Ok(Box::new(self.tenant_work(organization).await?))
    }
}

#[async_trait::async_trait]
impl application::organizations::OrganizationUnitOfWork for Database {
    async fn begin_discovery(
        &self,
    ) -> Result<Box<dyn application::organizations::OrganizationTransaction>, Error> {
        Ok(Box::new(self.discovery_work().await?))
    }

    async fn begin_organization(
        &self,
        organization: domain::OrganizationId,
    ) -> Result<Box<dyn application::organizations::OrganizationTransaction>, domain::Error> {
        Ok(Box::new(self.tenant_work(organization).await?))
    }
}

#[async_trait::async_trait]
impl application::memberships::MembershipUnitOfWork for Database {
    async fn begin_memberships(
        &self,
        organization: domain::OrganizationId,
    ) -> Result<Box<dyn application::memberships::MembershipTransaction>, domain::Error> {
        Ok(Box::new(self.tenant_work(organization).await?))
    }
}
#[async_trait]
impl TenantTransaction for TenantWork {
    async fn audit(&mut self, actor: &Actor, action: &str, subject: Uuid) -> Result<(), Error> {
        sqlx::query(
            r"
            INSERT INTO audit_events(id,organization_id,actor_id,action,subject,correlation_id)
            VALUES($1,$2,$3,$4,$5,$6)
            ",
        )
        .bind(Uuid::now_v7())
        .bind(self.organization)
        .bind(actor.user_id.into_uuid())
        .bind(action)
        .bind(subject)
        .bind(actor.correlation_id)
        .execute(&mut *self.tx)
        .await
        .map_err(storage)?;
        Ok(())
    }
    async fn commit(self: Box<Self>) -> Result<(), Error> {
        self.tx.commit().await.map_err(storage)
    }
}

#[async_trait]
impl application::invitations::InvitationUnitOfWork for Database {
    async fn begin_invitations(
        &self,
        organization: domain::OrganizationId,
    ) -> Result<Box<dyn application::invitations::InvitationTransaction>, Error> {
        Ok(Box::new(self.tenant_work(organization).await?))
    }
}
