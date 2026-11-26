use application::authentication::LoginMailer;
use async_trait::async_trait;
use domain::Error;
use lettre::{
    AsyncSmtpTransport, AsyncTransport, Message, Tokio1Executor, message::Mailbox,
    transport::smtp::authentication::Credentials,
};
use std::time::Duration;

pub struct SmtpMailer {
    transport: AsyncSmtpTransport<Tokio1Executor>,
    origin: String,
    from: Mailbox,
}
impl SmtpMailer {
    async fn send_message(&self, email: &str, subject: &str, body: String) -> Result<(), Error> {
        let message = Message::builder()
            .from(self.from.clone())
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
    /// # Errors
    /// Returns `Invalid` for non-loopback capture hosts.
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
            from: "login@example.test".parse().map_err(|_| Error::Invalid)?,
        })
    }
    /// Production SMTP requires STARTTLS with certificate verification.
    /// # Errors
    /// Returns `Invalid` for an invalid sender, TLS configuration or non-HTTPS public origin.
    pub fn production(
        host: &str,
        username: String,
        password: String,
        from: &str,
        origin: String,
    ) -> Result<Self, Error> {
        Self::production_endpoint(host, 587, username, password, from, origin, None)
    }
    /// Configure verified STARTTLS with an optional additional trust root.
    ///
    /// # Errors
    /// Returns `Invalid` for an invalid sender, origin, host, certificate or TLS configuration.
    pub fn production_endpoint(
        host: &str,
        port: u16,
        username: String,
        password: String,
        from: &str,
        origin: String,
        ca: Option<&[u8]>,
    ) -> Result<Self, Error> {
        use lettre::transport::smtp::client::{Certificate, Tls, TlsParameters};
        if !origin.starts_with("https://") {
            return Err(Error::Invalid);
        }
        let from = from.parse().map_err(|_| Error::Invalid)?;
        let mut tls = TlsParameters::builder(host.into());
        if let Some(ca) = ca {
            tls = tls.add_root_certificate(Certificate::from_pem(ca).map_err(|_| Error::Invalid)?);
        }
        let tls = tls.build().map_err(|_| Error::Invalid)?;
        let transport = AsyncSmtpTransport::<Tokio1Executor>::starttls_relay(host)
            .map_err(|_| Error::Invalid)?
            .port(port)
            .tls(Tls::Required(tls))
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
impl application::invitations::InvitationMailer for SmtpMailer {
    async fn send_invitation(
        &self,
        email: &str,
        org: domain::OrganizationId,
        id: domain::InvitationId,
        token: &str,
    ) -> Result<(), Error> {
        self.send_message(email,"Organization invitation",format!("Sign in as this email address, then confirm the invitation.\n{}/v1/invitations/confirmation#{}:{}:{}",self.origin,org,id,token)).await
    }
}
#[async_trait]
impl LoginMailer for SmtpMailer {
    async fn send_login(&self, email: &str, token: &str) -> Result<(), Error> {
        self.send_message(email, "Confirm sign in", format!("Open Project desk and choose Confirm sign in. This link expires in 10 minutes.\n{}/sign-in#{}\nIgnore this email if you did not request it.", self.origin, token)).await
    }
}
