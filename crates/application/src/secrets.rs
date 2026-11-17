/// Secret token generation is injected and never enters tracing fields.
pub trait Secrets: Send + Sync {
    fn token(&self) -> String;
}
