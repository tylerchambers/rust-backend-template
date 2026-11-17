use crate::{Actor, Secrets};
use async_trait::async_trait;
use domain::{Error, SessionId, UserId};
use std::sync::Arc;

/// Newly issued bearer credentials; never include the token in logs.
pub struct IssuedSession {
    pub token: String,
    pub user_id: UserId,
    pub id: SessionId,
}

/// Supplemental fail-closed login admission, not a replacement for durable budgets.
#[async_trait]
pub trait SecurityLimiter: Send + Sync {
    async fn admit(&self, key: &str) -> Result<(), Error>;
}

#[async_trait]
pub trait LoginMailer: Send + Sync {
    async fn send_login(&self, email: &str, token: &str) -> Result<(), Error>;
}
/// Atomic identity operations. The adapter owns database-time validity, budgets,
/// token hashing and conditional consumption; these are not generic user CRUD.
#[async_trait]
pub trait IdentityRepository: Send + Sync {
    /// Bound untrusted authentication traffic independently of account existence.
    async fn admit_source(&self, source: &str) -> Result<(), Error>;
    async fn issue_login(&self, email: &str, token: &str) -> Result<(), Error>;
    async fn consume_login(&self, token: &str, session: &str) -> Result<Actor, Error>;
    async fn authenticate(&self, token: &str) -> Result<Actor, Error>;
    async fn revoke(&self, token: &str) -> Result<(), Error>;
    async fn list_sessions(
        &self,
        actor: &Actor,
        after: Option<SessionId>,
    ) -> Result<Vec<SessionId>, Error>;
    async fn rotate(&self, actor: &Actor, old: &str, new: &str) -> Result<(), Error>;
    async fn revoke_session(&self, actor: &Actor, session: SessionId) -> Result<(), Error>;
    async fn revoke_all(&self, actor: &Actor) -> Result<(), Error>;
}

pub struct Authentication {
    identities: Arc<dyn IdentityRepository>,
    mail: Arc<dyn LoginMailer>,
    secrets: Arc<dyn Secrets>,
    limiter: Option<Arc<dyn SecurityLimiter>>,
}
impl Authentication {
    pub fn new(
        identities: Arc<dyn IdentityRepository>,
        mail: Arc<dyn LoginMailer>,
        secrets: Arc<dyn Secrets>,
    ) -> Self {
        Self {
            identities,
            mail,
            secrets,
            limiter: None,
        }
    }
    #[must_use]
    pub fn with_limiter(mut self, limiter: Arc<dyn SecurityLimiter>) -> Self {
        self.limiter = Some(limiter);
        self
    }
    /// Admit an authentication request against the source budget.
    ///
    /// # Errors
    /// Returns rate-limit or repository availability errors.
    pub async fn admit_source(&self, source: &str) -> Result<(), Error> {
        self.identities.admit_source(source).await
    }
    /// Issue a login link for a normalized email address.
    ///
    /// # Errors
    /// Rejects invalid email or exhausted budgets; propagates storage and delivery errors.
    pub async fn request(&self, email: &str) -> Result<(), Error> {
        let email = domain::EmailAddress::parse(email)?;
        let email = email.as_str();
        if let Some(limiter) = &self.limiter {
            limiter.admit(email).await?;
        }
        let token = self.secrets.token();
        self.identities.issue_login(email, &token).await?;
        self.mail.send_login(email, &token).await
    }
    /// Consume a login proof and create a session.
    ///
    /// # Errors
    /// Propagates expired, invalid or replayed proof and repository errors.
    pub async fn confirm(&self, token: &str) -> Result<IssuedSession, Error> {
        let session = self.secrets.token();
        let actor = self.identities.consume_login(token, &session).await?;
        Ok(IssuedSession {
            token: session,
            user_id: actor.user_id,
            id: actor.session_id,
        })
    }
    /// Resolve a currently valid session.
    ///
    /// # Errors
    /// Rejects invalid, revoked or expired credentials; propagates repository errors.
    pub async fn authenticate(&self, token: &str) -> Result<Actor, Error> {
        self.identities.authenticate(token).await
    }
    /// List at most 50 live sessions in ascending ID order after an exclusive cursor.
    /// Continue with the last returned ID until an empty page; concurrent creation
    /// or revocation is not a snapshot. Every page remains account-scoped.
    ///
    /// # Errors
    /// Propagates authentication and repository errors.
    pub async fn sessions(
        &self,
        token: &str,
        after: Option<SessionId>,
    ) -> Result<Vec<SessionId>, Error> {
        let actor = self.authenticate(token).await?;
        self.identities.list_sessions(&actor, after).await
    }
    /// Revoke one session belonging to the authenticated account.
    ///
    /// # Errors
    /// Propagates authentication, ownership and repository errors.
    pub async fn revoke_session(&self, token: &str, session: SessionId) -> Result<(), Error> {
        let actor = self.authenticate(token).await?;
        self.identities.revoke_session(&actor, session).await
    }
    /// Revoke all sessions belonging to the authenticated account.
    ///
    /// # Errors
    /// Propagates authentication and repository errors.
    pub async fn revoke_all(&self, token: &str) -> Result<(), Error> {
        let actor = self.authenticate(token).await?;
        self.identities.revoke_all(&actor).await
    }
    /// Rotate credentials while retaining revocation-only lineage.
    ///
    /// # Errors
    /// Propagates authentication and repository errors.
    pub async fn rotate(&self, token: &str) -> Result<IssuedSession, Error> {
        let actor = self.authenticate(token).await?;
        let new = self.secrets.token();
        self.identities.rotate(&actor, token, &new).await?;
        Ok(IssuedSession {
            token: new,
            user_id: actor.user_id,
            id: actor.session_id,
        })
    }
    /// Revoke a session through its current token or revocation lineage.
    ///
    /// # Errors
    /// Propagates repository errors.
    pub async fn logout(&self, token: &str) -> Result<(), Error> {
        self.identities.revoke(token).await
    }
}
