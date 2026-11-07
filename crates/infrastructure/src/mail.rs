use application::LoginMailer;
use async_trait::async_trait;
use domain::Error;
use lettre::{
    AsyncSmtpTransport, AsyncTransport, Message, Tokio1Executor,
    transport::smtp::authentication::Credentials,
};
use std::time::Duration;

pub struct SmtpMailer {
    transport: AsyncSmtpTransport<Tokio1Executor>,
    origin: String,
    from: String,
}
impl SmtpMailer {
    async fn send_message(&self, email: &str, subject: &str, body: String) -> Result<(), Error> {
        let message = Message::builder()
            .from(self.from.parse().map_err(|_| Error::Invalid)?)
            .to(email.parse().map_err(|_| Error::Invalid)?)
            .subject(subject)
            .body(body)
            .map_err(|_| Error::Invalid)?;
        self.transport
            .send(message)
            .await
            .map_err(|_| Error::Unavailable)?;
        Ok(())
    }
    /// Plain SMTP is restricted to an explicitly selected loopback capture endpoint.
    pub fn local_capture(host: &str, port: u16, origin: &str) -> Result<Self, Error> {
        if !matches!(host, "127.0.0.1" | "localhost" | "::1") {
            return Err(Error::Invalid);
        }
        Ok(Self {
            transport: AsyncSmtpTransport::<Tokio1Executor>::builder_dangerous(host)
                .port(port)
                .timeout(Some(Duration::from_secs(10)))
                .build(),
            origin: origin.into(),
            from: "login@example.test".into(),
        })
    }
    /// Production SMTP requires STARTTLS with certificate verification.
    pub fn production(
        host: &str,
        username: String,
        password: String,
        from: String,
        origin: String,
    ) -> Result<Self, Error> {
        if !origin.starts_with("https://") {
            return Err(Error::Invalid);
        }
        let transport = AsyncSmtpTransport::<Tokio1Executor>::starttls_relay(host)
            .map_err(|_| Error::Invalid)?
            .credentials(Credentials::new(username, password))
            .timeout(Some(Duration::from_secs(10)))
            .build();
        Ok(Self {
            transport,
            origin,
            from,
        })
    }
}
#[async_trait]
impl application::memberships::InvitationMailer for SmtpMailer {
    async fn send_invitation(
        &self,
        email: &str,
        org: uuid::Uuid,
        id: uuid::Uuid,
        token: &str,
    ) -> Result<(), Error> {
        self.send_message(email,"Organization invitation",format!("Sign in as this email address, then confirm the invitation.\n{}/v1/invitations/confirmation#{}:{}:{}",self.origin,org,id,token)).await
    }
}
#[async_trait]
impl application::memberships::ReauthenticationMailer for SmtpMailer {
    async fn send_proof(
        &self,
        email: &str,
        action: &str,
        target: uuid::Uuid,
        org: uuid::Uuid,
        token: &str,
    ) -> Result<(), Error> {
        self.send_message(email,"Confirm privileged operation",format!("This proof authorizes {action} on {target} in organization {org}. It expires in five minutes and is bound to your requesting session.\n{}/v1/auth/reauth/confirmation#{}",self.origin,token)).await
    }
}
#[async_trait]
impl LoginMailer for SmtpMailer {
    async fn send_login(&self, email: &str, token: &str) -> Result<(), Error> {
        let message=Message::builder().from(self.from.parse().map_err(|_|Error::Invalid)?).to(email.parse().map_err(|_|Error::Invalid)?).subject("Confirm sign in")
   .body(format!("Open this link, then explicitly confirm sign in. It expires in 10 minutes.\n{}/v1/auth/confirmation#{}\nIgnore this email if you did not request it.",self.origin,token)).map_err(|_|Error::Invalid)?;
        // SMTP errors may contain recipient/server data; never log their raw display.
        self.transport
            .send(message)
            .await
            .map_err(|_| Error::Unavailable)?;
        Ok(())
    }
}
