use serde::{Deserialize, Serialize};
mod ids;
pub use ids::{InvitationId, OrganizationId, ProjectId, SessionId, UserId};

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum Error {
    #[error("invalid_input")]
    Invalid,
    #[error("unauthenticated")]
    Unauthenticated,
    #[error("forbidden")]
    Forbidden,
    #[error("not_found")]
    NotFound,
    #[error("conflict")]
    Conflict,
    #[error("rate_limited")]
    RateLimited,
    #[error("unavailable")]
    Unavailable,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Project {
    pub id: ProjectId,
    pub organization_id: OrganizationId,
    pub name: String,
    pub version: i64,
    pub archived: bool,
}
impl Project {
    /// Create an active project at version one.
    ///
    /// # Errors
    /// Returns `Invalid` when the name is empty, too long or contains control characters.
    pub fn new(
        id: ProjectId,
        organization_id: OrganizationId,
        name: String,
    ) -> Result<Self, Error> {
        validate_name(&name)?;
        Ok(Self {
            id,
            organization_id,
            name,
            version: 1,
            archived: false,
        })
    }
    /// Rename only at the expected version.
    ///
    /// # Errors
    /// Returns `Conflict` for a stale or exhausted version and `Invalid` for an invalid name.
    pub fn rename(&mut self, expected: i64, name: String) -> Result<(), Error> {
        if self.version != expected {
            return Err(Error::Conflict);
        }
        validate_name(&name)?;
        self.name = name;
        self.version = self.version.checked_add(1).ok_or(Error::Conflict)?;
        Ok(())
    }
}
/// Validate the shared organization/project name contract.
///
/// # Errors
/// Returns `Invalid` for blank names, control characters or more than 120 characters.
pub fn validate_name(name: &str) -> Result<(), Error> {
    if name.trim().is_empty() || name.chars().count() > 120 || name.chars().any(char::is_control) {
        Err(Error::Invalid)
    } else {
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Role {
    Owner,
    Admin,
    Member,
    Viewer,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Permission {
    ReadProject,
    WriteProject,
    ManageMembers,
    ManageOwners,
}
impl Role {
    #[must_use]
    pub fn allows(self, permission: Permission) -> bool {
        match permission {
            Permission::ReadProject => true,
            Permission::WriteProject => self != Self::Viewer,
            Permission::ManageMembers => matches!(self, Self::Owner | Self::Admin),
            Permission::ManageOwners => self == Self::Owner,
        }
    }
}
