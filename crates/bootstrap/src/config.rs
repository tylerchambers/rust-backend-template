use infrastructure::InfraResult;
pub struct Config {
    pub(crate) database: String,
    pub(crate) origin: String,
    pub(crate) capture: bool,
}
impl Config {
    /// Validate the canonical public origin and mail mode.
    ///
    /// # Errors
    /// Returns `Invalid` for unsupported or insecure configuration.
    pub fn validate(origin: &str, mail: &str, development: bool) -> Result<(), domain::Error> {
        let url = url::Url::parse(origin).map_err(|_| domain::Error::Invalid)?;
        if !matches!(mail, "smtp" | "capture")
            || !matches!(url.scheme(), "http" | "https")
            || url.origin().ascii_serialization() != origin
            || !url.username().is_empty()
            || url.password().is_some()
            || url.query().is_some()
            || url.fragment().is_some()
            || (!development && (mail == "capture" || url.scheme() != "https"))
            || (url.scheme() == "http"
                && !matches!(url.host_str(), Some("localhost" | "127.0.0.1")))
        {
            return Err(domain::Error::Invalid);
        }
        Ok(())
    }
    pub(crate) fn load() -> InfraResult<Self> {
        let origin = std::env::var("PUBLIC_ORIGIN")?;
        let mail = std::env::var("MAIL_MODE")?;
        Self::validate(
            &origin,
            &mail,
            std::env::var("ENVIRONMENT").as_deref() == Ok("development"),
        )?;
        Ok(Self {
            database: std::env::var("DATABASE_URL")?,
            origin,
            capture: mail == "capture",
        })
    }
}

/// HTTP-only listener settings; workers never load them.
pub(crate) struct ServerConfig {
    pub(crate) runtime: Config,
    pub(crate) bind: String,
}
impl ServerConfig {
    pub(crate) fn load() -> InfraResult<Self> {
        Ok(Self {
            runtime: Config::load()?,
            bind: std::env::var("BIND_ADDRESS").unwrap_or_else(|_| "127.0.0.1:8080".into()),
        })
    }
}
