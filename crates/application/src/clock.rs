/// Invitation issue time; adapters retain authoritative security-validity time.
pub trait Clock: Send + Sync {
    fn now(&self) -> chrono::DateTime<chrono::Utc>;
}
