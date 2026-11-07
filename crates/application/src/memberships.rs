use crate::{Actor, projects::UnitOfWork};
use async_trait::async_trait;
use domain::{Error, Permission, Role};
use std::sync::Arc;
use uuid::Uuid;

#[async_trait]
pub trait InvitationMailer: Send + Sync {
    async fn send_invitation(
        &self,
        email: &str,
        organization: Uuid,
        invitation: Uuid,
        token: &str,
    ) -> Result<(), Error>;
}
#[async_trait]
pub trait ReauthenticationMailer: Send + Sync {
    async fn send_proof(
        &self,
        email: &str,
        action: &str,
        target: Uuid,
        organization: Uuid,
        token: &str,
    ) -> Result<(), Error>;
}
pub struct Reauthentication {
    repository: Arc<dyn ReauthenticationRepository>,
    mail: Arc<dyn ReauthenticationMailer>,
    secrets: Arc<dyn crate::Secrets>,
}
impl Reauthentication {
    pub fn new(
        repository: Arc<dyn ReauthenticationRepository>,
        mail: Arc<dyn ReauthenticationMailer>,
        secrets: Arc<dyn crate::Secrets>,
    ) -> Self {
        Self {
            repository,
            mail,
            secrets,
        }
    }
    pub async fn request(
        &self,
        actor: &Actor,
        action: &str,
        target: Uuid,
        org: Uuid,
    ) -> Result<(), Error> {
        let token = self.secrets.token();
        let email = self
            .repository
            .issue_proof(actor, action, target, org, &token)
            .await?;
        self.mail
            .send_proof(&email, action, target, org, &token)
            .await
    }
}

#[async_trait]
pub trait ReauthenticationRepository: Send + Sync {
    async fn issue_proof(
        &self,
        actor: &Actor,
        action: &str,
        target: Uuid,
        organization: Uuid,
        token: &str,
    ) -> Result<String, Error>;
}
#[async_trait]
pub trait MembershipRepository: Send {
    /// Serializes authority changes for an organization, including last-owner checks.
    async fn lock_organization(&mut self) -> Result<(), Error>;
    async fn member_role(&mut self, user: Uuid) -> Result<Role, Error>;
    async fn owner_count(&mut self) -> Result<i64, Error>;
    async fn set_role(&mut self, user: Uuid, role: Role) -> Result<(), Error>;
    async fn consume_proof(
        &mut self,
        actor: &Actor,
        action: &str,
        target: Uuid,
        token: &str,
    ) -> Result<(), Error>;
    async fn insert_invitation(&mut self, id: Uuid, email: &str, role: Role) -> Result<(), Error>;
    async fn accept_invitation(
        &mut self,
        actor: &Actor,
        id: Uuid,
        token: &str,
    ) -> Result<(), Error>;
    async fn enqueue_invitation(&mut self, id: Uuid) -> Result<(), Error>;
}
pub struct Memberships {
    uow: Arc<dyn UnitOfWork>,
}
impl Memberships {
    pub fn new(uow: Arc<dyn UnitOfWork>) -> Self {
        Self { uow }
    }
    pub async fn invite(
        &self,
        actor: &Actor,
        org: Uuid,
        email: &str,
        role: Role,
        proof: &str,
    ) -> Result<Uuid, Error> {
        let email = email.trim().to_lowercase();
        if email.len() > 254
            || !email.contains('@')
            || email.chars().any(char::is_whitespace)
            || email.chars().any(char::is_control)
        {
            return Err(Error::Invalid);
        }
        let mut tx = self.uow.begin(org).await?;
        tx.lock_organization().await?;
        let caller = tx.current_role(actor, org).await?;
        if !caller.allows(if role == Role::Owner {
            Permission::ManageOwners
        } else {
            Permission::ManageMembers
        }) {
            return Err(Error::Forbidden);
        }
        tx.consume_proof(actor, "invite", org, proof).await?;
        let id = Uuid::now_v7();
        tx.insert_invitation(id, &email, role).await?;
        tx.enqueue_invitation(id).await?;
        tx.audit(actor, "invitation.created", id).await?;
        tx.commit().await?;
        Ok(id)
    }
    pub async fn accept(
        &self,
        actor: &Actor,
        org: Uuid,
        id: Uuid,
        token: &str,
    ) -> Result<(), Error> {
        let mut tx = self.uow.begin(org).await?;
        tx.lock_organization().await?;
        tx.accept_invitation(actor, id, token).await?;
        tx.audit(actor, "invitation.accepted", id).await?;
        tx.commit().await
    }
    pub async fn change_role(
        &self,
        actor: &Actor,
        org: Uuid,
        user: Uuid,
        role: Role,
        proof: &str,
    ) -> Result<(), Error> {
        let mut tx = self.uow.begin(org).await?;
        tx.lock_organization().await?;
        let caller = tx.current_role(actor, org).await?;
        let old = tx.member_role(user).await?;
        let permission = if old == Role::Owner || role == Role::Owner {
            Permission::ManageOwners
        } else {
            Permission::ManageMembers
        };
        if !caller.allows(permission) {
            return Err(Error::Forbidden);
        }
        tx.consume_proof(actor, "change_role", user, proof).await?;
        if old == Role::Owner && role != Role::Owner && tx.owner_count().await? <= 1 {
            return Err(Error::Conflict);
        }
        tx.set_role(user, role).await?;
        tx.audit(actor, "membership.role_changed", user).await?;
        tx.commit().await
    }
}
