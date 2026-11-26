//! Deterministic business values, validation and project policy.

pub mod email;
pub mod error;
pub mod ids;
pub mod names;
pub mod projects;

pub use email::EmailAddress;
pub use error::Error;
pub use ids::{InvitationId, OrganizationId, ProjectId, SessionId, UserId};
pub use names::validate_name;
pub use projects::Project;
