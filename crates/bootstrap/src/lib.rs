//! Explicit process composition roots.
mod cli;
mod config;
mod server;
mod services;
mod telemetry;
mod worker;

pub use cli::{Failure, cli, migrate};
pub use config::Config;
pub use server::server;
pub use worker::worker;
