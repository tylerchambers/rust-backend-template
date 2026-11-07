pub mod memberships;
pub mod optional;
pub mod projects;
use async_trait::async_trait;
use domain::Error;
pub use projects::{Organizations, Projects};
use std::sync::Arc;
use uuid::Uuid;

pub struct Session {
    pub token: String,
    pub user_id: Uuid,
    pub id: Uuid,
}
#[derive(Clone)]
pub struct Actor {
    pub user_id: Uuid,
    pub session_id: Uuid,
}

/// Tokens deliberately have no Debug implementation and never enter log fields.
pub trait Secrets: Send + Sync {
    fn token(&self) -> String;
}
#[async_trait]
pub trait LoginMailer: Send + Sync {
    async fn send_login(&self, email: &str, token: &str) -> Result<(), Error>;
}
#[async_trait]
pub trait IdentityRepository: Send + Sync {
    async fn issue_login(&self, email: &str, token: &str) -> Result<(), Error>;
    async fn consume_login(&self, token: &str, session: &str) -> Result<Actor, Error>;
    async fn authenticate(&self, token: &str) -> Result<Actor, Error>;
    async fn revoke(&self, token: &str) -> Result<(), Error>;
    async fn list_sessions(&self, actor: &Actor) -> Result<Vec<Uuid>, Error>;
    async fn rotate(&self, actor: &Actor, old: &str, new: &str) -> Result<(), Error>;
    async fn revoke_all(&self, actor: &Actor) -> Result<(), Error>;
}

pub struct Authentication {
    identities: Arc<dyn IdentityRepository>,
    mail: Arc<dyn LoginMailer>,
    secrets: Arc<dyn Secrets>,
    limiter: Option<Arc<dyn optional::SecurityLimiter>>,
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
    pub fn with_limiter(mut self, limiter: Arc<dyn optional::SecurityLimiter>) -> Self {
        self.limiter = Some(limiter);
        self
    }
    pub async fn request(&self, email: &str) -> Result<(), Error> {
        let email = email.trim().to_lowercase();
        if email.len() > 254
            || !email.contains('@')
            || email.chars().any(char::is_whitespace)
            || email.chars().any(char::is_control)
        {
            return Err(Error::Invalid);
        }
        if let Some(limiter) = &self.limiter {
            limiter.admit(&email).await?;
        }
        let token = self.secrets.token();
        self.identities.issue_login(&email, &token).await?;
        self.mail.send_login(&email, &token).await
    }
    pub async fn confirm(&self, token: &str) -> Result<Session, Error> {
        let session = self.secrets.token();
        let actor = self.identities.consume_login(token, &session).await?;
        Ok(Session {
            token: session,
            user_id: actor.user_id,
            id: actor.session_id,
        })
    }
    pub async fn authenticate(&self, token: &str) -> Result<Actor, Error> {
        self.identities.authenticate(token).await
    }
    pub async fn sessions(&self, token: &str) -> Result<Vec<Uuid>, Error> {
        let actor = self.authenticate(token).await?;
        self.identities.list_sessions(&actor).await
    }
    pub async fn revoke_all(&self, token: &str) -> Result<(), Error> {
        let actor = self.authenticate(token).await?;
        self.identities.revoke_all(&actor).await
    }
    pub async fn rotate(&self, token: &str) -> Result<Session, Error> {
        let actor = self.authenticate(token).await?;
        let new = self.secrets.token();
        self.identities.rotate(&actor, token, &new).await?;
        Ok(Session {
            token: new,
            user_id: actor.user_id,
            id: actor.session_id,
        })
    }
    pub async fn logout(&self, token: &str) -> Result<(), Error> {
        self.identities.revoke(token).await
    }
}
