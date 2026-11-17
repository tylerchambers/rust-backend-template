//! Production adapters, grouped by external system.
pub mod postgres;
pub mod redis;
pub mod river;
pub mod smtp;
pub mod system;

pub type InfraResult<T> = Result<T, Box<dyn std::error::Error + Send + Sync>>;
